use execute::{CommandConfig, RetryPolicy, RetryStrategy};
use std::time::Duration;

fn main() {
    println!("=== Retry Strategy Demo ===\n");

    // 示例 1: 固定间隔重试
    println!("1. Fixed Interval Retry Strategy:");
    let fixed_policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_secs(1)));
    println!("   Max attempts: {}", fixed_policy.max_attempts);
    for attempt in 1..=3 {
        let delay = fixed_policy.delay_for_attempt(attempt);
        println!("   Attempt {}: delay = {:?}", attempt, delay);
    }

    // 示例 2: 指数退避重试
    println!("\n2. Exponential Backoff Retry Strategy:");
    let backoff_policy = RetryPolicy::new(
        5,
        RetryStrategy::ExponentialBackoff {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(10),
            multiplier: 2.0,
        },
    );
    println!("   Max attempts: {}", backoff_policy.max_attempts);
    for attempt in 1..=5 {
        let delay = backoff_policy.delay_for_attempt(attempt);
        println!("   Attempt {}: delay = {:?}", attempt, delay);
    }

    // 示例 3: 在 CommandConfig 中使用重试策略
    println!("\n3. Using Retry Policy with CommandConfig:");
    let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_secs(2)));
    let cmd = CommandConfig::new("curl", vec!["https://example.com".to_string()])
        .with_timeout(Duration::from_secs(30))
        .with_retry(policy);

    println!("   Command: {} {:?}", cmd.program(), cmd.args());
    println!("   Timeout: {:?}", cmd.timeout());
    if let Some(retry) = cmd.retry_policy() {
        println!("   Retry enabled: max {} attempts", retry.max_attempts);
    }

    // 示例 4: 指数退避达到最大值
    println!("\n4. Exponential Backoff with Max Limit:");
    let limited_backoff = RetryPolicy::new(
        10,
        RetryStrategy::ExponentialBackoff {
            initial: Duration::from_millis(50),
            max: Duration::from_secs(1),
            multiplier: 2.0,
        },
    );
    for attempt in 1..=10 {
        let delay = limited_backoff.delay_for_attempt(attempt);
        println!("   Attempt {}: delay = {:?}", attempt, delay);
    }

    println!("\n=== Demo Complete ===");
}
