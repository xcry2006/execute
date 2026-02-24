// Feature: production-ready-improvements, Property 17: 重试行为
// **Validates: Requirements 11.4, 11.6**
//
// 属性 17: 重试行为
// 对于任意失败的任务，如果配置了重试且未达到最大次数，应该自动重试；
// 达到最大次数后应该返回最终错误
//
// 验证需求：
// - 需求 11.4: WHEN 任务失败且未达到最大重试次数时，THE System SHALL 自动重试任务
// - 需求 11.6: WHEN 达到最大重试次数后仍失败时，THE System SHALL 返回最终错误

use execute::{CommandConfig, RetryPolicy, RetryStrategy, execute_with_retry};
use proptest::prelude::*;
use std::time::Duration;

/// 生成重试次数策略（0-5次）
fn retry_attempts_strategy() -> impl Strategy<Value = usize> {
    0usize..=5
}

/// 生成重试延迟策略（10-100ms）
fn retry_delay_strategy() -> impl Strategy<Value = Duration> {
    (10u64..=100).prop_map(Duration::from_millis)
}

/// 生成重试策略
fn retry_policy_strategy() -> impl Strategy<Value = RetryPolicy> {
    (retry_attempts_strategy(), retry_delay_strategy()).prop_map(|(attempts, delay)| {
        RetryPolicy::new(attempts, RetryStrategy::FixedInterval(delay))
    })
}

/// 生成超时时间策略（10-50ms）
fn timeout_strategy() -> impl Strategy<Value = Duration> {
    (10u64..=50).prop_map(Duration::from_millis)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意配置了重试的超时任务，应该重试指定次数后失败
    ///
    /// 验证需求：
    /// - 需求 11.4: 任务失败且未达到最大重试次数时，系统应自动重试
    /// - 需求 11.6: 达到最大重试次数后仍失败时，应返回最终错误
    #[test]
    fn prop_retry_behavior_on_timeout(
        retry_policy in retry_policy_strategy(),
        timeout in timeout_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建一个会超时的命令
        let config = CommandConfig::new("sleep", vec!["10".to_string()])
            .with_timeout(timeout)
            .with_retry(retry_policy.clone());

        // 记录开始时间
        let start = std::time::Instant::now();

        // 执行命令（应该失败）
        let result = execute_with_retry(&config, 1);

        // 记录执行时间
        let elapsed = start.elapsed();

        // 验证结果应该是错误（超时）
        prop_assert!(result.is_err(), "Command should fail due to timeout");

        // 验证执行时间
        // 总尝试次数 = 初始尝试 + 重试次数
        let total_attempts = retry_policy.max_attempts + 1;
        
        // 最小执行时间 = 总尝试次数 * 超时时间 + 重试次数 * 重试延迟
        // 允许一些误差（-5ms）因为系统调度可能导致轻微的时间差异
        let min_expected_time = timeout.saturating_sub(Duration::from_millis(5)) * total_attempts as u32
            + retry_policy.delay_for_attempt(1).saturating_sub(Duration::from_millis(5)) * retry_policy.max_attempts as u32;

        prop_assert!(
            elapsed >= min_expected_time,
            "Execution time ({:?}) should be at least {:?} (attempts: {}, timeout: {:?}, delay: {:?})",
            elapsed,
            min_expected_time,
            total_attempts,
            timeout,
            retry_policy.delay_for_attempt(1)
        );

        // 验证执行时间不会过长（允许一些误差）
        let max_expected_time = min_expected_time + Duration::from_millis(500);
        prop_assert!(
            elapsed <= max_expected_time,
            "Execution time ({:?}) should not exceed {:?}",
            elapsed,
            max_expected_time
        );
    }

    /// 属性测试：对于任意配置了重试的spawn失败任务，应该重试指定次数后失败
    ///
    /// 验证需求：
    /// - 需求 11.4: 任务失败且未达到最大重试次数时，系统应自动重试
    /// - 需求 11.6: 达到最大重试次数后仍失败时，应返回最终错误
    #[test]
    fn prop_retry_behavior_on_spawn_failure(
        retry_policy in retry_policy_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建一个会spawn失败的命令（不存在的命令）
        let config = CommandConfig::new("nonexistent_command_xyz_12345", vec![])
            .with_retry(retry_policy.clone());

        // 记录开始时间
        let start = std::time::Instant::now();

        // 执行命令（应该失败）
        let result = execute_with_retry(&config, 2);

        // 记录执行时间
        let elapsed = start.elapsed();

        // 验证结果应该是错误（spawn失败）
        prop_assert!(result.is_err(), "Command should fail due to spawn failure");

        // 验证执行时间
        // spawn失败很快，主要是重试延迟
        // 允许一些误差（-5ms）因为系统调度可能导致轻微的时间差异
        let min_expected_time = retry_policy.delay_for_attempt(1).saturating_sub(Duration::from_millis(5)) * retry_policy.max_attempts as u32;

        prop_assert!(
            elapsed >= min_expected_time,
            "Execution time ({:?}) should be at least {:?} (retry delays)",
            elapsed,
            min_expected_time
        );

        // 验证执行时间不会过长（spawn失败应该很快）
        let max_expected_time = min_expected_time + Duration::from_millis(500);
        prop_assert!(
            elapsed <= max_expected_time,
            "Execution time ({:?}) should not exceed {:?}",
            elapsed,
            max_expected_time
        );
    }

    /// 属性测试：对于任意成功的命令，即使配置了重试也不应该重试
    ///
    /// 验证需求：
    /// - 需求 11.4: 只有失败的任务才会重试，成功的任务不重试
    #[test]
    fn prop_no_retry_on_success(
        retry_policy in retry_policy_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建一个会成功的命令
        let config = CommandConfig::new("echo", vec!["success".to_string()])
            .with_retry(retry_policy.clone());

        // 记录开始时间
        let start = std::time::Instant::now();

        // 执行命令（应该成功）
        let result = execute_with_retry(&config, 3);

        // 记录执行时间
        let elapsed = start.elapsed();

        // 验证结果应该成功
        prop_assert!(result.is_ok(), "Command should succeed");

        // 验证输出
        let output = result.unwrap();
        prop_assert!(output.status.success(), "Exit status should be success");
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        prop_assert_eq!(
            stdout_str.trim(),
            "success",
            "Output should match expected"
        );

        // 验证执行时间应该很短（没有重试）
        // 成功的命令应该只执行一次，不应该有重试延迟
        let max_expected_time = Duration::from_millis(500);
        prop_assert!(
            elapsed <= max_expected_time,
            "Execution time ({:?}) should be quick (no retries for success)",
            elapsed
        );
    }
}

#[test]
#[cfg(unix)]
fn test_retry_behavior_zero_retries() {
    // 测试重试次数为0的情况（只尝试一次）
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let policy = RetryPolicy::new(0, RetryStrategy::FixedInterval(Duration::from_millis(50)));
    let config = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_timeout(Duration::from_millis(50))
        .with_retry(policy);

    let start = std::time::Instant::now();
    let result = execute_with_retry(&config, 1);
    let elapsed = start.elapsed();

    // 应该失败
    assert!(result.is_err());

    // 应该只尝试一次，没有重试延迟
    // 最小时间：1次超时 = 50ms
    assert!(elapsed >= Duration::from_millis(50));
    // 最大时间：允许一些系统开销
    assert!(elapsed < Duration::from_millis(300));
}

#[test]
#[cfg(unix)]
fn test_retry_behavior_max_retries() {
    // 测试达到最大重试次数
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_millis(50)));
    let config = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_timeout(Duration::from_millis(50))
        .with_retry(policy);

    let start = std::time::Instant::now();
    let result = execute_with_retry(&config, 2);
    let elapsed = start.elapsed();

    // 应该失败
    assert!(result.is_err());

    // 应该尝试4次（初始 + 3次重试），加上3次重试延迟
    // 4 * 50ms + 3 * 50ms = 350ms
    assert!(elapsed >= Duration::from_millis(350));
}

#[test]
#[cfg(unix)]
fn test_retry_behavior_exponential_backoff() {
    // 测试指数退避策略
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let policy = RetryPolicy::new(
        3,
        RetryStrategy::ExponentialBackoff {
            initial: Duration::from_millis(10),
            max: Duration::from_secs(1),
            multiplier: 2.0,
        },
    );
    let config = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_timeout(Duration::from_millis(50))
        .with_retry(policy);

    let start = std::time::Instant::now();
    let result = execute_with_retry(&config, 3);
    let elapsed = start.elapsed();

    // 应该失败
    assert!(result.is_err());

    // 应该尝试4次（初始 + 3次重试），加上指数退避延迟
    // 4 * 50ms + (10ms + 20ms + 40ms) = 270ms
    assert!(elapsed >= Duration::from_millis(270));
}

#[test]
#[cfg(unix)]
fn test_retry_behavior_success_no_retry() {
    // 测试成功的命令不会重试
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let policy = RetryPolicy::new(5, RetryStrategy::FixedInterval(Duration::from_millis(100)));
    let config = CommandConfig::new("echo", vec!["test".to_string()]).with_retry(policy);

    let start = std::time::Instant::now();
    let result = execute_with_retry(&config, 4);
    let elapsed = start.elapsed();

    // 应该成功
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "test");

    // 应该很快完成（没有重试延迟）
    assert!(elapsed < Duration::from_millis(500));
}

#[test]
#[cfg(unix)]
fn test_retry_behavior_spawn_failure() {
    // 测试spawn失败的重试
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let policy = RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(50)));
    let config = CommandConfig::new("nonexistent_command_xyz_12345", vec![]).with_retry(policy);

    let start = std::time::Instant::now();
    let result = execute_with_retry(&config, 5);
    let elapsed = start.elapsed();

    // 应该失败
    assert!(result.is_err());

    // 应该尝试3次（初始 + 2次重试），加上2次重试延迟
    // spawn失败很快，主要是延迟时间：2 * 50ms = 100ms
    assert!(elapsed >= Duration::from_millis(100));
}

#[test]
#[cfg(unix)]
fn test_retry_behavior_final_error_returned() {
    // 测试返回最终错误
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let policy = RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(50)));
    let config = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_timeout(Duration::from_millis(50))
        .with_retry(policy);

    let result = execute_with_retry(&config, 6);

    // 应该返回错误
    assert!(result.is_err());

    // 验证错误类型是超时错误
    let err = result.unwrap_err();
    let err_str = format!("{}", err);
    assert!(
        err_str.contains("timeout") || err_str.contains("Timeout"),
        "Error should indicate timeout: {}",
        err_str
    );
}

#[test]
#[cfg(unix)]
fn test_retry_behavior_without_retry_policy() {
    // 测试没有配置重试策略的情况
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    let config =
        CommandConfig::new("sleep", vec!["10".to_string()]).with_timeout(Duration::from_millis(50));

    let start = std::time::Instant::now();
    let result = execute_with_retry(&config, 7);
    let elapsed = start.elapsed();

    // 应该失败
    assert!(result.is_err());

    // 应该只尝试一次，没有重试
    // 最小时间：1次超时 = 50ms
    assert!(elapsed >= Duration::from_millis(50));
    // 最大时间：允许一些系统开销
    assert!(elapsed < Duration::from_millis(300));
}
