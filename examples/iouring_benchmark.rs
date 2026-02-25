//! io_uring 性能测试
//!
//! 对比标准执行器与 io_uring 优化执行器的性能差异。
//! 需要 Linux 5.1+ 内核支持。

#[cfg(feature = "iouring")]
use execute::{CommandConfig, IoUringExecutor, execute_batch_iouring};
#[cfg(not(feature = "iouring"))]
use execute::CommandConfig;

use std::time::Instant;

fn main() {
    println!("=== io_uring 性能测试 ===\n");

    #[cfg(not(feature = "iouring"))]
    {
        println!("警告：未启用 iouring feature");
        println!("请使用：cargo run --example iouring_benchmark --features iouring\n");
        run_standard_benchmark();
        return;
    }

    #[cfg(feature = "iouring")]
    {
        run_iouring_benchmark();
    }
}

#[cfg(feature = "iouring")]
fn run_iouring_benchmark() {
    // 测试 1: 单命令执行对比
    println!("【测试 1】单命令执行对比");
    compare_single_execution();

    // 测试 2: 批量执行对比
    println!("\n【测试 2】批量执行对比");
    compare_batch_execution();

    // 测试 3: io_uring 执行器吞吐量
    println!("\n【测试 3】io_uring 执行器吞吐量");
    test_iouring_throughput();
}

#[cfg(feature = "iouring")]
fn compare_single_execution() {
    use execute::CommandPool;

    let config = CommandConfig::new("echo", vec!["hello".to_string()]);
    let count = 100;

    // 标准执行器
    let pool = CommandPool::new();
    let start = Instant::now();
    for _ in 0..count {
        let _ = pool.execute_task(&config);
    }
    let standard_time = start.elapsed();

    // io_uring 执行器
    let mut executor = IoUringExecutor::new(32).expect("Failed to create io_uring executor");
    let start = Instant::now();
    for _ in 0..count {
        let _ = executor.execute(&config);
    }
    let iouring_time = start.elapsed();

    println!("  标准执行器: {:?} ({:.0} 次/秒)", 
             standard_time, count as f64 / standard_time.as_secs_f64());
    println!("  io_uring:   {:?} ({:.0} 次/秒)", 
             iouring_time, count as f64 / iouring_time.as_secs_f64());
    
    let speedup = standard_time.as_secs_f64() / iouring_time.as_secs_f64();
    println!("  加速比: {:.2}x", speedup);
}

#[cfg(feature = "iouring")]
fn compare_batch_execution() {
    use execute::CommandPool;

    let configs: Vec<_> = (0..50)
        .map(|i| CommandConfig::new("echo", vec![format!("task{}", i)]))
        .collect();

    // 标准顺序执行
    let pool = CommandPool::new();
    let start = Instant::now();
    for config in &configs {
        let _ = pool.execute_task(config);
    }
    let standard_time = start.elapsed();

    // io_uring 批量执行
    let start = Instant::now();
    let _ = execute_batch_iouring(&configs);
    let iouring_time = start.elapsed();

    println!("  标准顺序执行: {:?}", standard_time);
    println!("  io_uring批量: {:?}", iouring_time);
    
    let speedup = standard_time.as_secs_f64() / iouring_time.as_secs_f64();
    println!("  加速比: {:.2}x", speedup);
}

#[cfg(feature = "iouring")]
fn test_iouring_throughput() {
    let mut executor = IoUringExecutor::new(256).expect("Failed to create io_uring executor");
    let config = CommandConfig::new("true", vec![]);
    let count = 500;

    let start = Instant::now();
    for _ in 0..count {
        let _ = executor.execute(&config);
    }
    let elapsed = start.elapsed();

    println!("  执行 {} 个 true 命令: {:?}", count, elapsed);
    println!("  吞吐量: {:.0} 命令/秒", count as f64 / elapsed.as_secs_f64());
}

#[cfg(not(feature = "iouring"))]
fn run_standard_benchmark() {
    println!("运行标准基准测试...\n");
    println!("  请启用 iouring feature 进行完整测试");
    println!("  cargo run --example iouring_benchmark --features iouring");
}
