use execute::{CommandConfig, RetryPolicy, RetryStrategy, execute_with_retry};
use std::time::Duration;

fn main() {
    // 初始化日志系统
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Retry Execution Demo ===\n");

    // 示例 1: 固定间隔重试
    println!("1. Fixed Interval Retry Strategy");
    println!("   Retrying a command that might fail with 1 second intervals\n");

    let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_secs(1)));
    let config =
        CommandConfig::new("echo", vec!["Hello from retry!".to_string()]).with_retry(policy);

    match execute_with_retry(&config, 1) {
        Ok(output) => {
            println!("   ✓ Command succeeded!");
            println!("   Output: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("   ✗ Command failed: {}", e);
        }
    }

    println!("\n2. Exponential Backoff Retry Strategy");
    println!("   Retrying with exponential backoff (100ms, 200ms, 400ms, ...)\n");

    let policy = RetryPolicy::new(
        5,
        RetryStrategy::ExponentialBackoff {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(10),
            multiplier: 2.0,
        },
    );
    let config =
        CommandConfig::new("echo", vec!["Exponential backoff!".to_string()]).with_retry(policy);

    match execute_with_retry(&config, 2) {
        Ok(output) => {
            println!("   ✓ Command succeeded!");
            println!("   Output: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("   ✗ Command failed: {}", e);
        }
    }

    // 示例 3: 超时与重试结合
    println!("\n3. Retry with Timeout");
    println!("   Command will timeout and retry 2 times\n");

    let policy = RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(500)));
    let config = CommandConfig::new("sleep", vec!["5".to_string()])
        .with_timeout(Duration::from_millis(200))
        .with_retry(policy);

    let start = std::time::Instant::now();
    match execute_with_retry(&config, 3) {
        Ok(output) => {
            println!("   ✓ Command succeeded!");
            println!("   Output: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("   ✗ Command failed after retries: {}", e);
            println!("   Total time: {:?}", start.elapsed());
        }
    }

    // 示例 4: 命令不存在的情况
    println!("\n4. Retry with Non-existent Command");
    println!("   Command doesn't exist, will retry 3 times\n");

    let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_millis(200)));
    let config = CommandConfig::new("nonexistent_command_xyz", vec![]).with_retry(policy);

    let start = std::time::Instant::now();
    match execute_with_retry(&config, 4) {
        Ok(output) => {
            println!("   ✓ Command succeeded!");
            println!("   Output: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("   ✗ Command failed after retries: {}", e);
            println!("   Total time: {:?}", start.elapsed());
        }
    }

    println!("\n=== Demo Complete ===");
}
