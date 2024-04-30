
#![allow(dead_code, unused_variables)]

extern crate rand;

use std::{result, thread, time::{self, Instant}};

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
struct Result {
    reads: usize,
    read_times: u128,
    writes: usize,
    write_times: u128,
    ops: usize,
    op_times: u128,
}



fn read_write(intObject : RluInt64Wrapper, write_ratio : f64) {
    let mut rw = intObject.clone();
    let worker = || unsafe {
        let g = rw.rlu_global;
        let obj = rw.obj;

        let mut results = Result::default();
        let mut _rnd = SmallRng::from_seed([0; 16]);
        let start = Instant::now();

        // initialize thread
        let id = rlu_thread_init(g);

        loop {
            if start.elapsed().as_millis() > TIMEOUT {
                break;
            }

            let i = Instant::now();
            let _rnd_write_val = _rnd.gen_range(0, 10);
            if _rnd.gen::<f64>() < write_ratio {
                let curr = Instant::now();

                'inner : loop {
                    rlu_reader_lock(g, id);
                    let locked_obj = rlu_try_lock(g, id, obj);

                    match locked_obj {
                        None => {
                          rlu_abort(g, id);
                          continue;
                        }
            
                        Some(obj) => {
                          *obj = _rnd_write_val;
                          results.writes += 1;
                          results.write_times += start.elapsed().as_nanos();
                          break 'inner;
                        }
                      }

                    
                } 
                rlu_reader_unlock(g, id);
            } else {
                let curr = Instant::now();
                rlu_reader_lock(g, id);
                let read_obj = rlu_dereference(g, id, obj).unwrap();
                results.reads += 1;
                results.read_times += start.elapsed().as_nanos();
                rlu_reader_unlock(g, id);
            }

            results.ops += 1;
            results.op_times += start.elapsed().as_nanos();

        }

        results
    };


    let threads: Vec<_> = (0..N_THREADS).map(|_| worker()).collect();
    for t in threads {
        println!("{:?}", t);
    }

    //worker.join().unwrap();
}

fn benchmark() {

    let rlu_global : *mut RluGlobal<u64> = RluGlobal::init();
    let rlu_global_obj = unsafe { & *rlu_global };

    let mut intObject = RluInt64Wrapper { // need wrapper for unsafe send and sync
        obj : Box::into_raw(Box::new(rlu_global_obj.alloc(0))),
        rlu_global: rlu_global,
    };

    println!("Start benchmarking...");
    for write_ratio in &[0.01] {
        for i in 0..N_THREADS {
            read_write(intObject, *write_ratio);
        }

    }
}

fn main() {
    benchmark();
}
