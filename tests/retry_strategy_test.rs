use execute::{RetryPolicy, RetryStrategy};
use std::time::Duration;

#[test]
fn test_fixed_interval_strategy() {
    let strategy = RetryStrategy::FixedInterval(Duration::from_secs(1));

    // 固定间隔策略应该对所有重试次数返回相同的延迟
    assert_eq!(strategy.delay_for_attempt(1), Duration::from_secs(1));
    assert_eq!(strategy.delay_for_attempt(2), Duration::from_secs(1));
    assert_eq!(strategy.delay_for_attempt(3), Duration::from_secs(1));
    assert_eq!(strategy.delay_for_attempt(10), Duration::from_secs(1));
}

#[test]
fn test_exponential_backoff_strategy() {
    let strategy = RetryStrategy::ExponentialBackoff {
        initial: Duration::from_millis(100),
        max: Duration::from_secs(10),
        multiplier: 2.0,
    };

    // 指数退避：100ms, 200ms, 400ms, 800ms, ...
    assert_eq!(strategy.delay_for_attempt(1), Duration::from_millis(100));
    assert_eq!(strategy.delay_for_attempt(2), Duration::from_millis(200));
    assert_eq!(strategy.delay_for_attempt(3), Duration::from_millis(400));
    assert_eq!(strategy.delay_for_attempt(4), Duration::from_millis(800));
    assert_eq!(strategy.delay_for_attempt(5), Duration::from_millis(1600));
}

#[test]
fn test_exponential_backoff_max_limit() {
    let strategy = RetryStrategy::ExponentialBackoff {
        initial: Duration::from_millis(100),
        max: Duration::from_secs(1), // 最大 1 秒
        multiplier: 2.0,
    };

    // 应该在达到最大值后保持不变
    assert_eq!(strategy.delay_for_attempt(1), Duration::from_millis(100));
    assert_eq!(strategy.delay_for_attempt(2), Duration::from_millis(200));
    assert_eq!(strategy.delay_for_attempt(3), Duration::from_millis(400));
    assert_eq!(strategy.delay_for_attempt(4), Duration::from_millis(800));
    assert_eq!(strategy.delay_for_attempt(5), Duration::from_secs(1)); // 达到最大值
    assert_eq!(strategy.delay_for_attempt(6), Duration::from_secs(1)); // 保持最大值
    assert_eq!(strategy.delay_for_attempt(10), Duration::from_secs(1)); // 保持最大值
}

#[test]
fn test_exponential_backoff_different_multiplier() {
    let strategy = RetryStrategy::ExponentialBackoff {
        initial: Duration::from_millis(50),
        max: Duration::from_secs(10),
        multiplier: 3.0, // 使用 3 倍增长
    };

    // 指数退避：50ms, 150ms, 450ms, 1350ms, ...
    assert_eq!(strategy.delay_for_attempt(1), Duration::from_millis(50));
    assert_eq!(strategy.delay_for_attempt(2), Duration::from_millis(150));
    assert_eq!(strategy.delay_for_attempt(3), Duration::from_millis(450));
    assert_eq!(strategy.delay_for_attempt(4), Duration::from_millis(1350));
}

#[test]
fn test_retry_policy_fixed_interval() {
    let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_secs(2)));

    assert_eq!(policy.max_attempts, 3);
    assert_eq!(policy.delay_for_attempt(1), Duration::from_secs(2));
    assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(2));
    assert_eq!(policy.delay_for_attempt(3), Duration::from_secs(2));
}

#[test]
fn test_retry_policy_exponential_backoff() {
    let policy = RetryPolicy::new(
        5,
        RetryStrategy::ExponentialBackoff {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(10),
            multiplier: 2.0,
        },
    );

    assert_eq!(policy.max_attempts, 5);
    assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(100));
    assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
    assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
}

#[test]
fn test_retry_policy_zero_attempts() {
    // 即使 max_attempts 为 0，策略仍然有效（表示不重试）
    let policy = RetryPolicy::new(0, RetryStrategy::FixedInterval(Duration::from_secs(1)));

    assert_eq!(policy.max_attempts, 0);
}

#[test]
fn test_exponential_backoff_edge_case_attempt_zero() {
    let strategy = RetryStrategy::ExponentialBackoff {
        initial: Duration::from_millis(100),
        max: Duration::from_secs(10),
        multiplier: 2.0,
    };

    // attempt = 0 应该返回 initial（边界情况处理）
    let delay = strategy.delay_for_attempt(0);
    assert_eq!(delay, Duration::from_millis(100));
}

#[test]
fn test_exponential_backoff_large_attempt() {
    let strategy = RetryStrategy::ExponentialBackoff {
        initial: Duration::from_millis(1),
        max: Duration::from_secs(60),
        multiplier: 2.0,
    };

    // 测试大的重试次数不会溢出
    let delay = strategy.delay_for_attempt(100);
    assert_eq!(delay, Duration::from_secs(60)); // 应该被限制在最大值
}

#[test]
fn test_command_config_with_retry() {
    use execute::CommandConfig;

    let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_secs(1)));
    let cmd = CommandConfig::new("curl", vec!["https://example.com".to_string()])
        .with_retry(policy.clone());

    // 验证重试策略已设置
    assert!(cmd.retry_policy().is_some());
    let retrieved_policy = cmd.retry_policy().unwrap();
    assert_eq!(retrieved_policy.max_attempts, 3);
}

#[test]
fn test_command_config_without_retry() {
    use execute::CommandConfig;

    let cmd = CommandConfig::new("echo", vec!["hello".to_string()]);

    // 默认情况下没有重试策略
    assert!(cmd.retry_policy().is_none());
}

#[test]
fn test_command_config_retry_chaining() {
    use execute::CommandConfig;

    // 测试链式调用
    let policy = RetryPolicy::new(
        5,
        RetryStrategy::ExponentialBackoff {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(10),
            multiplier: 2.0,
        },
    );

    let cmd = CommandConfig::new("test", vec![])
        .with_timeout(Duration::from_secs(30))
        .with_working_dir("/tmp")
        .with_retry(policy);

    assert!(cmd.retry_policy().is_some());
    assert_eq!(cmd.timeout(), Some(Duration::from_secs(30)));
    assert_eq!(cmd.working_dir(), Some("/tmp"));
}
