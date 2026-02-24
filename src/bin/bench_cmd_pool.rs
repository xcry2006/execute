use execute::{CommandConfig, CommandPool};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn bench_push_pop_single_thread() {
    let iterations = 100;
    let n = 10_000;
    for i in 0..iterations {
        let pool = CommandPool::new();
        let start = Instant::now();
        for j in 0..n {
            let _ = pool.push_task(CommandConfig::new("echo", vec![j.to_string()]));
        }
        // Clear the queue instead of popping
        let _ = pool.clear();
        let dur = start.elapsed();
        println!("single_thread run {:3} took {:?}", i + 1, dur);
    }
}

fn bench_push_multi_thread() {
    let iterations = 50;
    let per_thread = 2_000;
    for i in 0..iterations {
        let pool = Arc::new(CommandPool::new());
        let start = Instant::now();
        let mut handles = Vec::new();
        for _ in 0..8 {
            let pool_clone = pool.clone();
            handles.push(thread::spawn(move || {
                for k in 0..per_thread {
                    let _ = pool_clone.push_task(CommandConfig::new("echo", vec![k.to_string()]));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        // Clear the queue instead of popping
        let _ = pool.clear();
        let dur = start.elapsed();
        println!("multi_thread run {:3} took {:?}", i + 1, dur);
    }
}

fn bench_execute_true() {
    let iterations = 20;
    for i in 0..iterations {
        let pool = CommandPool::new();
        let cfg = CommandConfig::new("true", vec![]);
        let start = Instant::now();
        for _ in 0..100 {
            let _ = pool.execute_task(&cfg);
        }
        let dur = start.elapsed();
        println!("execute_true run {:3} took {:?}", i + 1, dur);
    }
}

fn bench_executor() {
    let iterations = 10;
    for i in 0..iterations {
        let pool = CommandPool::new();
        for _ in 0..100 {
            let _ = pool.push_task(CommandConfig::new("true", vec![]));
        }
        let start = Instant::now();
        pool.start_executor(Duration::from_millis(1));
        // wait until pool is empty
        while !pool.is_empty() {
            thread::sleep(Duration::from_millis(10));
        }
        let dur = start.elapsed();
        println!("executor run {:3} took {:?}", i + 1, dur);
    }
}

fn main() {
    println!("Starting simple benchmarks...");
    bench_push_pop_single_thread();
    bench_push_multi_thread();
    bench_execute_true();
    bench_executor();
}
