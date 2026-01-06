use criterion::{criterion_group, criterion_main, Criterion};
use execute::{CommandConfig, CommandPool};
use std::sync::Arc;
use std::thread;

fn bench_push_pop_single_thread(c: &mut Criterion) {
    c.bench_function("push_pop_single_thread_1k", |b| b.iter(|| {
        let pool = CommandPool::new();
        for i in 0..1000 {
            pool.push_task(CommandConfig::new("echo", vec![i.to_string()]));
        }
        while pool.pop_task().is_some() {}
    }));
}

fn bench_push_multi_thread(c: &mut Criterion) {
    c.bench_function("push_multi_thread_8x1k", |b| b.iter(|| {
        let pool = Arc::new(CommandPool::new());
        let mut handles = Vec::new();
        for _ in 0..8 {
            let pool_clone = pool.clone();
            handles.push(thread::spawn(move || {
                for i in 0..1000 {
                    pool_clone.push_task(CommandConfig::new("echo", vec![i.to_string()]));
                }
            }));
        }
        for h in handles { h.join().unwrap(); }
        while pool.pop_task().is_some() {}
    }));
}

fn bench_execute_true(c: &mut Criterion) {
    c.bench_function("execute_true_100", |b| b.iter(|| {
        let pool = CommandPool::new();
        let cfg = CommandConfig::new("true", vec![]);
        for _ in 0..100 {
            let _ = pool.execute_task(&cfg);
        }
    }));
}

criterion_group!(benches, bench_push_pop_single_thread, bench_push_multi_thread, bench_execute_true);
criterion_main!(benches);
