//! 进程池预热性能测试
//!
//! 对比预热与非预热的执行性能。

use execute::{CommandConfig, WarmExecutor, WarmProcessPool};
use std::time::Instant;

fn main() {
    println!("=== 进程池预热性能测试 ===\n");

    // 测试 1: echo 命令预热效果
    test_echo_warmup();

    // 测试 2: true 命令预热效果
    test_true_warmup();

    // 测试 3: 预热执行器对比
    test_warm_executor();

    // 测试 4: 不同预热数量对比
    test_warmup_counts();

    // 测试 5: 长时间运行稳定性
    test_long_running();

    println!("\n=== 测试完成 ===");
}

fn test_echo_warmup() {
    println!("【测试 1】echo 命令预热效果");
    let config = CommandConfig::new("echo", vec!["hello".to_string()]);
    let count = 10;

    // 非预热执行
    let start = Instant::now();
    for _ in 0..count {
        let output = std::process::Command::new("echo")
            .arg("hello")
            .output()
            .unwrap();
        let _ = String::from_utf8_lossy(&output.stdout);
    }
    let no_warm_time = start.elapsed();

    // 预热执行
    let pool = WarmProcessPool::new(4, std::time::Duration::from_secs(60));
    pool.warm_up(&config, 2).unwrap();

    let start = Instant::now();
    for _ in 0..count {
        let mut child = pool.execute_with_warm(&config).unwrap();
        let output = child.wait_with_output().unwrap();
        let _ = String::from_utf8_lossy(&output.stdout);
    }
    let warm_time = start.elapsed();

    println!("  非预热执行: {:?}", no_warm_time);
    println!("  预热执行:   {:?}", warm_time);
    println!("  加速比: {:.2}x\n", 
             no_warm_time.as_secs_f64() / warm_time.as_secs_f64());
}

fn test_true_warmup() {
    println!("【测试 2】true 命令预热效果");
    let config = CommandConfig::new("true", vec![]);
    let count = 50;

    // 非预热执行
    let start = Instant::now();
    for _ in 0..count {
        let _ = std::process::Command::new("true").status().unwrap();
    }
    let no_warm_time = start.elapsed();

    // 预热执行
    let pool = WarmProcessPool::new(8, std::time::Duration::from_secs(60));
    pool.warm_up(&config, 4).unwrap();

    let start = Instant::now();
    for _ in 0..count {
        let mut child = pool.execute_with_warm(&config).unwrap();
        let _ = child.wait().unwrap();
    }
    let warm_time = start.elapsed();

    println!("  非预热执行: {:?}", no_warm_time);
    println!("  预热执行:   {:?}", warm_time);
    println!("  加速比: {:.2}x\n", 
             no_warm_time.as_secs_f64() / warm_time.as_secs_f64());
}

fn test_warm_executor() {
    println!("【测试 3】预热执行器对比");
    let config = CommandConfig::new("echo", vec!["test".to_string()]);
    let count = 20;

    // 标准执行器
    let start = Instant::now();
    for _ in 0..count {
        let output = std::process::Command::new("echo")
            .arg("test")
            .output()
            .unwrap();
        let _ = String::from_utf8_lossy(&output.stdout);
    }
    let standard_time = start.elapsed();

    // 预热执行器
    let executor = WarmExecutor::new();
    executor.warm_up(&config, 4).unwrap();

    let start = Instant::now();
    for _ in 0..count {
        let output = executor.execute(&config).unwrap();
        let _ = String::from_utf8_lossy(&output.stdout);
    }
    let warm_time = start.elapsed();

    println!("  标准执行器: {:?}", standard_time);
    println!("  预热执行器: {:?}", warm_time);
    println!("  加速比: {:.2}x\n", 
             standard_time.as_secs_f64() / warm_time.as_secs_f64());
}

fn test_warmup_counts() {
    println!("【测试 4】不同预热数量对比");
    let config = CommandConfig::new("true", vec![]);
    let execute_count = 10;

    let warm_counts = [0, 1, 2, 4, 8];
    for &warm_count in &warm_counts {
        let pool = WarmProcessPool::new(8, std::time::Duration::from_secs(60));
        
        // 预热
        if warm_count > 0 {
            pool.warm_up(&config, warm_count).unwrap();
        }

        let start = Instant::now();
        for _ in 0..execute_count {
            let mut child = pool.execute_with_warm(&config).unwrap();
            let _ = child.wait().unwrap();
        }
        let elapsed = start.elapsed();

        println!("  预热 {} 个进程: {:?}", warm_count, elapsed);
    }
    println!();
}

fn test_long_running() {
    println!("【测试 5】长时间运行稳定性");
    let config = CommandConfig::new("echo", vec!["stability".to_string()]);
    let iterations = 100;

    let executor = WarmExecutor::new();
    executor.warm_up(&config, 2).unwrap();

    let start = Instant::now();
    let mut success_count = 0;

    for i in 0..iterations {
        match executor.execute(&config) {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("stability") {
                    success_count += 1;
                }
            }
            Err(e) => {
                println!("  第 {} 次执行失败: {:?}", i, e);
            }
        }
    }

    let elapsed = start.elapsed();
    let success_rate = success_count as f64 / iterations as f64 * 100.0;

    println!("  执行 {} 次: {:?}", iterations, elapsed);
    println!("  成功率: {:.1}%", success_rate);
    println!("  平均每次: {:?}", elapsed / iterations);
    println!("  吞吐量: {:.0} 次/秒\n", 
             iterations as f64 / elapsed.as_secs_f64());
}
