//! 环境变量应用优化性能测试
//!
//! 对比标准 apply_env_config 与优化版本的性能差异。

use execute::{CommandConfig, EnvCache, EnvConfig, EnvOptimizer, apply_env_config_optimized};
use std::process::Command;
use std::time::Instant;

fn main() {
    println!("=== 环境变量应用优化性能测试 ===\n");

    // 测试 1: 少量环境变量
    test_small_env();

    // 测试 2: 大量环境变量
    test_large_env();

    // 测试 3: 缓存效果
    test_cache_performance();

    // 测试 4: 批量应用对比
    test_batch_apply();

    // 测试 5: 实际命令执行对比
    test_real_command_execution();

    println!("\n=== 测试完成 ===");
}

/// 标准实现（原始版本）
fn apply_env_config_standard(cmd: &mut Command, env_config: &EnvConfig) {
    if !env_config.inherit_parent() {
        cmd.env_clear();
    }

    for (key, value) in env_config.vars() {
        match value {
            Some(v) => {
                cmd.env(key, v);
            }
            None => {
                cmd.env_remove(key);
            }
        }
    }
}

fn test_small_env() {
    println!("【测试 1】少量环境变量（5个）");
    let config = EnvConfig::new()
        .set("VAR1", "value1")
        .set("VAR2", "value2")
        .set("VAR3", "value3")
        .set("VAR4", "value4")
        .set("VAR5", "value5");

    let iterations = 10000;

    // 标准实现
    let start = Instant::now();
    for _ in 0..iterations {
        let mut cmd = Command::new("echo");
        apply_env_config_standard(&mut cmd, &config);
    }
    let standard_time = start.elapsed();

    // 优化实现
    let start = Instant::now();
    for _ in 0..iterations {
        let mut cmd = Command::new("echo");
        apply_env_config_optimized(&mut cmd, &config);
    }
    let optimized_time = start.elapsed();

    println!(
        "  标准实现: {:?} ({:.0} 次/秒)",
        standard_time,
        iterations as f64 / standard_time.as_secs_f64()
    );
    println!(
        "  优化实现: {:?} ({:.0} 次/秒)",
        optimized_time,
        iterations as f64 / optimized_time.as_secs_f64()
    );
    println!(
        "  加速比: {:.2}x\n",
        standard_time.as_secs_f64() / optimized_time.as_secs_f64()
    );
}

fn test_large_env() {
    println!("【测试 2】大量环境变量（50个）");
    let mut config = EnvConfig::new();
    for i in 0..50 {
        config = config.set(format!("VAR{}", i), format!("value{}", i));
    }

    let iterations = 5000;

    // 标准实现
    let start = Instant::now();
    for _ in 0..iterations {
        let mut cmd = Command::new("echo");
        apply_env_config_standard(&mut cmd, &config);
    }
    let standard_time = start.elapsed();

    // 优化实现
    let start = Instant::now();
    for _ in 0..iterations {
        let mut cmd = Command::new("echo");
        apply_env_config_optimized(&mut cmd, &config);
    }
    let optimized_time = start.elapsed();

    println!(
        "  标准实现: {:?} ({:.0} 次/秒)",
        standard_time,
        iterations as f64 / standard_time.as_secs_f64()
    );
    println!(
        "  优化实现: {:?} ({:.0} 次/秒)",
        optimized_time,
        iterations as f64 / optimized_time.as_secs_f64()
    );
    println!(
        "  加速比: {:.2}x\n",
        standard_time.as_secs_f64() / optimized_time.as_secs_f64()
    );
}

fn test_cache_performance() {
    println!("【测试 3】缓存性能（相同配置重复使用）");
    let config = EnvConfig::new()
        .set("PATH", "/usr/bin:/bin")
        .set("HOME", "/home/user")
        .set("USER", "test");

    let iterations = 10000;

    // 无缓存：每次都创建优化器
    let start = Instant::now();
    for _ in 0..iterations {
        let mut cmd = Command::new("echo");
        let optimizer = EnvOptimizer::from_config(&config);
        optimizer.apply(&mut cmd);
    }
    let no_cache_time = start.elapsed();

    // 有缓存：使用 EnvCache
    let mut cache = EnvCache::new();
    let start = Instant::now();
    for _ in 0..iterations {
        let mut cmd = Command::new("echo");
        let optimizer = cache.get_or_create(&config);
        optimizer.apply(&mut cmd);
    }
    let cached_time = start.elapsed();

    println!(
        "  无缓存: {:?} ({:.0} 次/秒)",
        no_cache_time,
        iterations as f64 / no_cache_time.as_secs_f64()
    );
    println!(
        "  有缓存: {:?} ({:.0} 次/秒)",
        cached_time,
        iterations as f64 / cached_time.as_secs_f64()
    );
    println!(
        "  缓存加速比: {:.2}x",
        no_cache_time.as_secs_f64() / cached_time.as_secs_f64()
    );
    println!("  缓存大小: {}\n", cache.len());
}

fn test_batch_apply() {
    println!("【测试 4】批量应用对比（10个变量）");
    let mut config = EnvConfig::new().no_inherit();
    for i in 0..10 {
        config = config.set(format!("KEY{}", i), format!("value{}", i));
    }

    let iterations = 5000;

    // 逐个设置
    let start = Instant::now();
    for _ in 0..iterations {
        let mut cmd = Command::new("echo");
        cmd.env_clear();
        for i in 0..10 {
            cmd.env(format!("KEY{}", i), format!("value{}", i));
        }
    }
    let individual_time = start.elapsed();

    // 批量设置（优化版本）
    let start = Instant::now();
    for _ in 0..iterations {
        let mut cmd = Command::new("echo");
        apply_env_config_optimized(&mut cmd, &config);
    }
    let batch_time = start.elapsed();

    println!(
        "  逐个设置: {:?} ({:.0} 次/秒)",
        individual_time,
        iterations as f64 / individual_time.as_secs_f64()
    );
    println!(
        "  批量设置: {:?} ({:.0} 次/秒)",
        batch_time,
        iterations as f64 / batch_time.as_secs_f64()
    );
    println!(
        "  批量加速比: {:.2}x\n",
        individual_time.as_secs_f64() / batch_time.as_secs_f64()
    );
}

fn test_real_command_execution() {
    println!("【测试 5】实际命令执行（带环境变量）");
    let config = EnvConfig::new()
        .set("TEST_VAR1", "hello")
        .set("TEST_VAR2", "world");

    let cmd_config = CommandConfig::new("env", vec![]).with_env(config);
    let iterations = 100;

    // 使用标准方式
    let start = Instant::now();
    for _ in 0..iterations {
        let mut cmd = Command::new(cmd_config.program());
        cmd.args(cmd_config.args());
        if let Some(env_config) = cmd_config.env_config() {
            apply_env_config_standard(&mut cmd, env_config);
        }
        let _ = cmd.output();
    }
    let standard_time = start.elapsed();

    // 使用优化方式
    let start = Instant::now();
    for _ in 0..iterations {
        let mut cmd = Command::new(cmd_config.program());
        cmd.args(cmd_config.args());
        if let Some(env_config) = cmd_config.env_config() {
            apply_env_config_optimized(&mut cmd, env_config);
        }
        let _ = cmd.output();
    }
    let optimized_time = start.elapsed();

    println!(
        "  标准方式: {:?} ({:.0} 次/秒)",
        standard_time,
        iterations as f64 / standard_time.as_secs_f64()
    );
    println!(
        "  优化方式: {:?} ({:.0} 次/秒)",
        optimized_time,
        iterations as f64 / optimized_time.as_secs_f64()
    );
    println!(
        "  加速比: {:.2}x\n",
        standard_time.as_secs_f64() / optimized_time.as_secs_f64()
    );
}
