#![allow(dead_code, unused_variables)]

extern crate rand;

use std::{thread, time::Instant};
use rlu::{RluSet, ConcurrentSet};

use rand::{rngs::SmallRng, Rng, SeedableRng};

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
    insert_ratio: f64,
    n_threads: u8,
    timeout: u128,
    initial_size: usize,
    range: usize
}

fn read_write(set: RluSet<usize>, config : BenchmarkConfig) -> BenchmarkResult {
    let worker = || {
        let mut results: BenchmarkResult = BenchmarkResult::default();
        let set = set.clone_ref();

        thread::spawn(move || {
            let start = Instant::now();
            // initialize thread
            let mut ops = 0;
            loop {
                if start.elapsed().as_millis() > config.timeout {
                    break;
                }

                let i = Instant::now();

                let mut _rnd = SmallRng::from_seed([0;16]);
                let num = _rnd.gen_range(0, config.range);
                if _rnd.gen::<f64>() < config.write_ratio {
                    //println!("write op: {}, thread {}", ops, n_threads);
                    let curr = Instant::now();

                    if _rnd.gen::<f64>()  < config.insert_ratio {
                        set.delete(num);
                    } else {
                        set.insert(num);
                    }
                    
                } else {
                    // read operation
                    set.contains(num);
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
    println!("Write_Ratio,Thread_Count,Throughput");
    for wr in &[0.02, 0.2, 0.4] {
        for i in 1..=8 {
            let config = BenchmarkConfig {
                write_ratio: *wr,
                insert_ratio: 0.5,
                n_threads: i,
                timeout: 10000,
                initial_size: 256,
                range: 512,
            };

            let ops: Vec<_> = (0..3).map(|_| {
                let set = RluSet::new();
                let mut _rnd = SmallRng::from_seed([0; 16]);
                while set.len() < config.initial_size {
                    let i = _rnd.gen_range(0, config.range);
                    set.insert(i);
                }
                read_write(set, config)
            }).collect();
            
            let avg: f64 = (ops.iter().map(|res| res.ops).sum::<usize>() as f64)/ (ops.len() as f64);
            let throughput = avg / ((config.timeout * 1000) as f64);

            println!("{},{},{}", wr, i, throughput);
        }
    }
}

fn main() {
    benchmark();
}
