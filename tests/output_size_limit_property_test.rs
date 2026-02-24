// Feature: production-ready-improvements, Property 12: 输出大小限制
// **Validates: Requirement 8.2**
//
// 属性 12: 输出大小限制
// 对于任意输出超过限制的命令，输出应该被截断并记录警告
//
// 验证需求：
// - 需求 8.2: WHEN 命令输出超过限制时，THE System SHALL 截断输出并记录警告

use execute::{execute_command_with_context, CommandConfig, ResourceLimits};
use proptest::prelude::*;
use std::time::Duration;

/// 生成输出大小限制策略（10-1000字节）
fn output_limit_strategy() -> impl Strategy<Value = usize> {
    10usize..=1000
}

// Note: Helper strategies for generating commands are defined inline in the tests
// to avoid unused function warnings and keep the code focused

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意输出大小限制，超过限制的输出应该被截断
    ///
    /// 验证需求：
    /// - 需求 8.2: WHEN 命令输出超过限制时，THE System SHALL 截断输出并记录警告
    #[test]
    fn prop_output_size_limit_truncates_large_output(
        limit in output_limit_strategy(),
    ) {
        // 初始化 tracing 以捕获警告日志
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建资源限制配置
        let limits = ResourceLimits::new().with_max_output_size(limit);

        // 使用会产生大量输出的命令
        let config = CommandConfig::new(
            "echo",
            vec!["This is a very long output that will definitely exceed the limit and should be truncated by the system".to_string()]
        )
        .with_resource_limits(limits)
        .with_timeout(Duration::from_secs(5));

        // 执行命令
        let result = execute_command_with_context(&config, 1);

        // 验证命令成功执行
        prop_assert!(
            result.is_ok(),
            "Command should execute successfully even with output limit"
        );

        let output = result.unwrap();

        // 验证输出被截断到限制大小或更小
        // 注意：由于读取是按块进行的，实际输出可能略小于限制
        prop_assert!(
            output.stdout.len() <= limit,
            "Output size ({}) should not exceed limit ({})",
            output.stdout.len(),
            limit
        );

        // 如果原始输出确实超过了限制，验证输出被截断
        // 原始 echo 输出大约是 100+ 字节
        if limit < 100 {
            prop_assert!(
                output.stdout.len() <= limit,
                "Output should be truncated when exceeding limit"
            );
        }
    }

    /// 属性测试：对于任意输出大小限制，小于限制的输出不应该被截断
    ///
    /// 验证需求：
    /// - 需求 8.2: 只有超过限制的输出才被截断
    #[test]
    fn prop_output_size_limit_preserves_small_output(
        limit in 100usize..=1000,
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建资源限制配置（限制足够大）
        let limits = ResourceLimits::new().with_max_output_size(limit);

        // 使用产生小输出的命令
        let config = CommandConfig::new("echo", vec!["test".to_string()])
            .with_resource_limits(limits)
            .with_timeout(Duration::from_secs(5));

        // 执行命令
        let result = execute_command_with_context(&config, 1);

        // 验证命令成功执行
        prop_assert!(result.is_ok(), "Command should execute successfully");

        let output = result.unwrap();

        // 验证小输出没有被截断（echo "test" 输出约 5 字节）
        prop_assert!(
            output.stdout.len() < limit,
            "Small output should not be truncated"
        );

        // 验证输出内容正确
        let output_str = String::from_utf8_lossy(&output.stdout);
        prop_assert!(
            output_str.contains("test"),
            "Output should contain the expected content"
        );
    }

    /// 属性测试：对于不同的输出大小限制，截断行为应该一致
    ///
    /// 验证需求：
    /// - 需求 8.2: 截断行为应该对所有限制值一致
    #[test]
    fn prop_output_size_limit_consistent_truncation(
        limit1 in 50usize..=200,
        limit2 in 50usize..=200,
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 使用相同的命令但不同的限制
        let limits1 = ResourceLimits::new().with_max_output_size(limit1);
        let limits2 = ResourceLimits::new().with_max_output_size(limit2);

        let config1 = CommandConfig::new(
            "echo",
            vec!["This is a long output for testing truncation behavior".to_string()]
        )
        .with_resource_limits(limits1)
        .with_timeout(Duration::from_secs(5));

        let config2 = CommandConfig::new(
            "echo",
            vec!["This is a long output for testing truncation behavior".to_string()]
        )
        .with_resource_limits(limits2)
        .with_timeout(Duration::from_secs(5));

        // 执行两个命令
        let result1 = execute_command_with_context(&config1, 1);
        let result2 = execute_command_with_context(&config2, 2);

        // 验证都成功执行
        prop_assert!(result1.is_ok(), "First command should succeed");
        prop_assert!(result2.is_ok(), "Second command should succeed");

        let output1 = result1.unwrap();
        let output2 = result2.unwrap();

        // 验证输出大小符合各自的限制
        prop_assert!(
            output1.stdout.len() <= limit1,
            "First output should respect its limit"
        );
        prop_assert!(
            output2.stdout.len() <= limit2,
            "Second output should respect its limit"
        );

        // 如果 limit1 < limit2，则 output1 应该 <= output2
        if limit1 < limit2 {
            prop_assert!(
                output1.stdout.len() <= output2.stdout.len(),
                "Smaller limit should produce smaller or equal output"
            );
        }
    }
}

#[test]
fn test_output_size_limit_basic_truncation() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建一个小的输出限制
    let limits = ResourceLimits::new().with_max_output_size(20);

    let config = CommandConfig::new(
        "echo",
        vec!["This is a very long output that should be truncated".to_string()],
    )
    .with_resource_limits(limits)
    .with_timeout(Duration::from_secs(5));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证成功执行
    assert!(result.is_ok(), "Command should execute successfully");

    let output = result.unwrap();

    // 验证输出被截断
    assert!(
        output.stdout.len() <= 20,
        "Output should be truncated to 20 bytes or less, got {}",
        output.stdout.len()
    );
}

#[test]
fn test_output_size_limit_no_truncation_for_small_output() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建一个大的输出限制
    let limits = ResourceLimits::new().with_max_output_size(1000);

    let config = CommandConfig::new("echo", vec!["small".to_string()])
        .with_resource_limits(limits)
        .with_timeout(Duration::from_secs(5));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证成功执行
    assert!(result.is_ok(), "Command should execute successfully");

    let output = result.unwrap();

    // 验证输出没有被截断
    let output_str = String::from_utf8_lossy(&output.stdout);
    assert!(
        output_str.contains("small"),
        "Small output should not be truncated"
    );
}

#[test]
fn test_output_size_limit_zero_limit() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建零字节限制（边界情况）
    let limits = ResourceLimits::new().with_max_output_size(0);

    let config = CommandConfig::new("echo", vec!["test".to_string()])
        .with_resource_limits(limits)
        .with_timeout(Duration::from_secs(5));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证成功执行
    assert!(result.is_ok(), "Command should execute successfully");

    let output = result.unwrap();

    // 验证输出为空（被完全截断）
    assert_eq!(
        output.stdout.len(),
        0,
        "Output should be completely truncated with zero limit"
    );
}

#[test]
#[cfg(unix)]
fn test_output_size_limit_with_large_output_command() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建输出限制
    let limits = ResourceLimits::new().with_max_output_size(100);

    // 使用 seq 命令生成大量输出
    let config = CommandConfig::new("seq", vec!["1".to_string(), "1000".to_string()])
        .with_resource_limits(limits)
        .with_timeout(Duration::from_secs(5));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证成功执行
    assert!(result.is_ok(), "Command should execute successfully");

    let output = result.unwrap();

    // 验证输出被截断
    assert!(
        output.stdout.len() <= 100,
        "Output should be truncated to 100 bytes or less, got {}",
        output.stdout.len()
    );
}

#[test]
fn test_output_size_limit_stderr_also_limited() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建输出限制
    let limits = ResourceLimits::new().with_max_output_size(50);

    // 使用一个会输出到 stderr 的命令
    #[cfg(unix)]
    let config = CommandConfig::new(
        "sh",
        vec![
            "-c".to_string(),
            "echo 'This is a long error message that should be truncated' >&2".to_string(),
        ],
    )
    .with_resource_limits(limits)
    .with_timeout(Duration::from_secs(5));

    #[cfg(not(unix))]
    let config = CommandConfig::new(
        "cmd",
        vec![
            "/c".to_string(),
            "echo This is a long error message that should be truncated 1>&2".to_string(),
        ],
    )
    .with_resource_limits(limits)
    .with_timeout(Duration::from_secs(5));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证成功执行
    assert!(result.is_ok(), "Command should execute successfully");

    let output = result.unwrap();

    // 验证 stderr 也被限制
    assert!(
        output.stderr.len() <= 50,
        "Stderr should also be limited to 50 bytes or less, got {}",
        output.stderr.len()
    );
}
