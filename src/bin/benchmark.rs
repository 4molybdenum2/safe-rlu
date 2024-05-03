#![allow(dead_code, unused_variables)]

extern crate rand;

use std::{thread, time::Instant};

use rand::{rngs::SmallRng, Rng, SeedableRng};
use rlu::{
  rlu_dereference, rlu_reader_lock, rlu_reader_unlock,
  rlu_try_lock, rlu_thread_init, rlu_abort, RluGlobal, Rlu
};

#[derive(Copy, Clone, Debug)]
pub struct RluInt64Wrapper {
  pub obj : *mut Rlu<u64>,
  pub rlu_global : *mut RluGlobal<u64>
}

unsafe impl Send for RluInt64Wrapper {}
unsafe impl Sync for RluInt64Wrapper {}

// Constants
const N_THREADS: u8 = 4;
const TIMEOUT: u128 = 10000;

#[derive(Clone, Copy, Default, Debug)]
struct BenchmarkResult {
    n_threads: u8,
    reads: usize,
    read_times: u128,
    writes: usize,
    write_times: u128,
    ops: usize,
    op_times: u128,
}


#[derive(Clone, Copy)]
struct BenchmarkConfig {
    write_ratio: f64,
    n_threads: u8,
    timeout: u128,
}

fn read_write(rw : RluInt64Wrapper, config : BenchmarkConfig) -> BenchmarkResult {
    let worker = || {
        let mut results = BenchmarkResult::default();

        thread::spawn(move || unsafe {
            let rw = rw;
            let g = rw.rlu_global;
            let obj = rw.obj; 
            let mut _rnd = SmallRng::from_seed([0; 16]);
            let start = Instant::now();

            // initialize thread
            let id = rlu_thread_init(g);
            let mut ops = 0;
            loop {
                if start.elapsed().as_millis() > config.timeout {
                    break;
                }

                let i = Instant::now();
                if _rnd.gen::<f64>() < config.write_ratio {
                    //println!("write op: {}, thread {}", ops, n_threads);
                    let curr = Instant::now();

                    // write operation
                    'inner: loop {
                        rlu_reader_lock(g, id);
                        let locked_obj = rlu_try_lock(g, id, obj);

                        match locked_obj {
                            None => {
                              rlu_abort(g, id);
                              continue;
                            }
                
                            Some(locked_obj) => {
                              *locked_obj += 1;
                              results.writes += 1;
                              results.write_times += start.elapsed().as_nanos();
                              break 'inner;
                            }
                          }
                    } 
                    rlu_reader_unlock(g, id);
                    
                } else {
                    // read operation
                    let curr = Instant::now();
                    rlu_reader_lock(g, id);
                    let read_obj = rlu_dereference(g, id, obj).unwrap();
                    results.reads += 1;
                    results.read_times += start.elapsed().as_nanos();
                    rlu_reader_unlock(g, id);
                }

                results.ops += 1;
                results.op_times += i.elapsed().as_nanos();
                ops += 1;
            }

            results.n_threads = config.n_threads;
            //println!("Results for {} threads: {:?}", config.n_threads, results);
            results
        })
    };


    let threads: Vec<_> = (0..config.n_threads).map(|_| worker()).collect();
    threads.into_iter().map(|t| t.join().unwrap()).fold(
        BenchmarkResult::default(), 
    
        |mut x, res| {
            x.ops += res.ops;
            x.op_times += res.op_times;

            x.reads += res.reads;
            x.read_times += res.read_times;

            x.writes += res.writes;
            x.write_times += res.write_times;

            x
        }
    )
}

fn benchmark() {
    let rlu_global : *mut RluGlobal<u64> = RluGlobal::init();
    let rlu_global_obj = unsafe { & *rlu_global };

    let int_object = RluInt64Wrapper {
        obj : Box::into_raw(Box::new(rlu_global_obj.alloc(0))),
        rlu_global: rlu_global,
    };

    

    println!(" Write Fraction, Thread Count, Throughput");
    for wr in &[0.02, 0.2, 0.4] {
        
        for i in 1..=N_THREADS {
            let config = BenchmarkConfig {
                write_ratio: *wr,
                n_threads: i,
                timeout: 10000,
            };

            let ops = read_write(int_object, config).ops as f64;
            
            let throughput = ops / ((config.timeout * 1000) as f64);
            println!(" {} {} {}", wr, i, throughput);
        }
    }
}

fn main() {
    benchmark();
}
