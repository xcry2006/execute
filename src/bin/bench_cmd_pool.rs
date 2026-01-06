use execute::{CommandConfig, CommandPool, CommandPoolSeg};
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
            pool.push_task(CommandConfig::new("echo", vec![j.to_string()]));
        }
        while pool.pop_task().is_some() {}
        let dur = start.elapsed();
        println!("single_thread run {:3} took {:?}", i + 1, dur);
    }
}

fn bench_push_pop_single_thread_seg() {
    let iterations = 100;
    let n = 10_000;
    for i in 0..iterations {
        let pool = CommandPoolSeg::new();
        let start = Instant::now();
        for j in 0..n {
            pool.push_task(CommandConfig::new("echo", vec![j.to_string()]));
        }
        while pool.pop_task().is_some() {}
        let dur = start.elapsed();
        println!("single_thread_seg run {:3} took {:?}", i + 1, dur);
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
                    pool_clone.push_task(CommandConfig::new("echo", vec![k.to_string()]));
                }
            }));
        }
        for h in handles { h.join().unwrap(); }
        while pool.pop_task().is_some() {}
        let dur = start.elapsed();
        println!("multi_thread run {:3} took {:?}", i + 1, dur);
    }
}

fn bench_push_multi_thread_seg() {
    let iterations = 50;
    let per_thread = 2_000;
    for i in 0..iterations {
        let pool = Arc::new(CommandPoolSeg::new());
        let start = Instant::now();
        let mut handles = Vec::new();
        for _ in 0..8 {
            let pool_clone = pool.clone();
            handles.push(thread::spawn(move || {
                for k in 0..per_thread {
                    pool_clone.push_task(CommandConfig::new("echo", vec![k.to_string()]));
                }
            }));
        }
        for h in handles { h.join().unwrap(); }
        while pool.pop_task().is_some() {}
        let dur = start.elapsed();
        println!("multi_thread_seg run {:3} took {:?}", i + 1, dur);
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

fn bench_executor_with_limit() {
    let iterations = 10;
    for i in 0..iterations {
        let pool = CommandPool::new();
        for _ in 0..100 {
            pool.push_task(CommandConfig::new("true", vec![]));
        }
        let start = Instant::now();
        // workers=4, limit=4
        pool.start_executor_with_workers_and_limit(Duration::from_millis(1), 4, 4);
        // wait until pool is empty
        while !pool.is_empty() {
            thread::sleep(Duration::from_millis(10));
        }
        let dur = start.elapsed();
        println!("executor_limit run {:3} took {:?}", i + 1, dur);
    }
}

fn main() {
    println!("Starting simple benchmarks...");
    bench_push_pop_single_thread();
    bench_push_pop_single_thread_seg();
    bench_push_multi_thread();
    bench_push_multi_thread_seg();
    bench_execute_true();
    bench_executor_with_limit();
}
