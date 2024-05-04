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


#[derive(Clone, Copy, Default, Debug)]
struct BenchmarkResult {
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

        
        thread::spawn(move || unsafe {
            let mut results = BenchmarkResult::default();

            let rw = rw;
            let g = rw.rlu_global;
            let obj = rw.obj; 
            let mut _rnd = SmallRng::from_seed([0; 16]);
            let start = Instant::now();

            let id = rlu_thread_init(g);
            // initialize thread
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
                              break 'inner;
                            }
                          }
                    } 
                    rlu_reader_unlock(g, id);
                    results.writes += 1;
                    results.write_times += curr.elapsed().as_nanos();
                } else {
                    // read operation
                    let curr = Instant::now();
                    rlu_reader_lock(g, id);
                    let read_obj = rlu_dereference(g, id, obj).unwrap();
                    rlu_reader_unlock(g, id);


                    results.reads += 1;
                    results.read_times += curr.elapsed().as_nanos();
                }

                results.ops += 1;
                results.op_times += i.elapsed().as_nanos();
            }

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


    

    println!("Write_Ratio,Thread_Count,Throughput");
    for wr in &[0.02, 0.2, 0.4] {
        for i in 1..=8 {
            let config = BenchmarkConfig {
                write_ratio: *wr,
                n_threads: i,
                timeout: 10000,
            };


            let rlu_global : *mut RluGlobal<u64> = RluGlobal::init();
            let rlu_global_obj = unsafe { & *rlu_global };

            let int_object = RluInt64Wrapper {
                obj : Box::into_raw(Box::new(rlu_global_obj.alloc(0))),
                rlu_global: rlu_global,
            };

            let ops: Vec<_> = (0..3).map(|_| {
                read_write(int_object, config)
            }).collect();
            
            let avg: f64 = (ops.iter().map(|res| res.reads).sum::<usize>() as f64)/ (ops.len() as f64);
            let throughput = avg / ((config.timeout * 1000) as f64);

            println!("{},{},{}", wr, i, throughput);
        }
    }
}

fn main() {
    benchmark();
}
