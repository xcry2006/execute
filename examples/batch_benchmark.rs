//! 批量执行性能测试
//!
//! 对比标准执行与批量执行的性能差异。

use execute::{
    BatchConfig, CommandConfig, CommandPool, execute_batch_detailed, execute_parallel_batch,
    execute_sequential_batch,
};
use std::time::Instant;

fn main() {
    println!("=== 批量执行性能测试 ===\n");

    // 测试 1: 小批量命令（10个）
    test_small_batch();

    // 测试 2: 中批量命令（50个）
    test_medium_batch();

    // 测试 3: 大批量命令（100个）
    test_large_batch();

    // 测试 4: 顺序 vs 并行批量
    test_sequential_vs_parallel();

    // 测试 5: 详细批量执行
    test_detailed_batch();

    println!("\n=== 测试完成 ===");
}

fn test_small_batch() {
    println!("【测试 1】小批量命令（10个 echo）");
    let configs: Vec<_> = (0..10)
        .map(|i| CommandConfig::new("echo", vec![format!("task{}", i)]))
        .collect();

    // 标准逐个执行
    let pool = CommandPool::new();
    let start = Instant::now();
    for config in &configs {
        let _ = pool.execute_task(config);
    }
    let standard_time = start.elapsed();

    // 并行批量执行
    let start = Instant::now();
    let _ = execute_parallel_batch(&configs, &BatchConfig::default());
    let parallel_time = start.elapsed();

    println!("  标准逐个执行: {:?}", standard_time);
    println!("  并行批量执行: {:?}", parallel_time);
    println!(
        "  加速比: {:.2}x\n",
        standard_time.as_secs_f64() / parallel_time.as_secs_f64()
    );
}

fn test_medium_batch() {
    println!("【测试 2】中批量命令（50个 true）");
    let configs: Vec<_> = (0..50)
        .map(|_| CommandConfig::new("true", vec![]))
        .collect();

    // 标准逐个执行
    let pool = CommandPool::new();
    let start = Instant::now();
    for config in &configs {
        let _ = pool.execute_task(config);
    }
    let standard_time = start.elapsed();

    // 并行批量执行
    let start = Instant::now();
    let _ = execute_parallel_batch(&configs, &BatchConfig::default());
    let parallel_time = start.elapsed();

    // 顺序批量执行
    let start = Instant::now();
    let _ = execute_sequential_batch(&configs, true);
    let sequential_time = start.elapsed();

    println!("  标准逐个执行: {:?}", standard_time);
    println!("  并行批量执行: {:?}", parallel_time);
    println!("  顺序批量执行: {:?}", sequential_time);
    println!(
        "  并行加速比: {:.2}x",
        standard_time.as_secs_f64() / parallel_time.as_secs_f64()
    );
    println!(
        "  顺序加速比: {:.2}x\n",
        standard_time.as_secs_f64() / sequential_time.as_secs_f64()
    );
}

fn test_large_batch() {
    println!("【测试 3】大批量命令（100个 echo）");
    let configs: Vec<_> = (0..100)
        .map(|i| CommandConfig::new("echo", vec![format!("item{}", i)]))
        .collect();

    // 标准逐个执行（只执行10个作为基准）
    let pool = CommandPool::new();
    let start = Instant::now();
    for config in configs.iter().take(10) {
        let _ = pool.execute_task(config);
    }
    let standard_time_10 = start.elapsed();
    let estimated_standard = standard_time_10 * 10; // 估算100个的时间

    // 并行批量执行
    let start = Instant::now();
    let result = execute_parallel_batch(&configs, &BatchConfig::default());
    let parallel_time = start.elapsed();

    println!("  标准逐个执行(10个): {:?}", standard_time_10);
    println!("  估算标准执行(100个): {:?}", estimated_standard);
    println!("  并行批量执行(100个): {:?}", parallel_time);

    if let Ok(output) = result {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        println!("  成功执行: {} 个命令", lines.len());
    }

    println!(
        "  预估加速比: {:.1}x\n",
        estimated_standard.as_secs_f64() / parallel_time.as_secs_f64()
    );
}

fn test_sequential_vs_parallel() {
    println!("【测试 4】顺序 vs 并行批量（20个 sleep 0.01）");
    let configs: Vec<_> = (0..20)
        .map(|_| CommandConfig::new("sleep", vec!["0.01".to_string()]))
        .collect();

    // 顺序执行（预计 20 * 0.01 = 0.2s）
    let start = Instant::now();
    let _ = execute_sequential_batch(&configs, true);
    let sequential_time = start.elapsed();

    // 并行执行（预计 0.01s + 开销）
    let start = Instant::now();
    let _ = execute_parallel_batch(
        &configs,
        &BatchConfig {
            max_parallel: 20,
            ..Default::default()
        },
    );
    let parallel_time = start.elapsed();

    println!("  顺序批量: {:?}", sequential_time);
    println!("  并行批量: {:?}", parallel_time);
    println!(
        "  并行加速比: {:.2}x\n",
        sequential_time.as_secs_f64() / parallel_time.as_secs_f64()
    );
}

fn test_detailed_batch() {
    println!("【测试 5】详细批量执行（5个命令）");
    let configs = vec![
        CommandConfig::new("echo", vec!["success1".to_string()]),
        CommandConfig::new("echo", vec!["success2".to_string()]),
        CommandConfig::new("false", vec![]), // 会失败
        CommandConfig::new("echo", vec!["success3".to_string()]),
        CommandConfig::new("/nonexistent", vec![]), // 会失败
    ];

    let start = Instant::now();
    let results = execute_batch_detailed(&configs);
    let elapsed = start.elapsed();

    println!("  执行 {} 个命令: {:?}", configs.len(), elapsed);

    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(output) => {
                println!(
                    "  [{}] {}: stdout={}",
                    i,
                    if output.success { "✓" } else { "✗" },
                    output.stdout.trim()
                );
            }
            Err(e) => {
                println!("  [{}] ✗ Error: {:?}", i, e);
            }
        }
    }
    println!();
}
