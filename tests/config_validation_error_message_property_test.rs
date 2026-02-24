// Feature: production-ready-improvements, Property 8: 配置验证错误消息
// **Validates: Requirements 5.6**
//
// 属性 8: 配置验证错误消息
// 对于任意无效配置参数，系统应该返回清晰描述问题的错误消息
//
// 验证需求：
// - 需求 5.6: 系统在配置验证失败时提供清晰的错误消息

use execute::{ConfigError, PoolConfigBuilder};
use proptest::prelude::*;
use std::time::Duration;

/// 生成无效线程数策略（包括 0 和非常大的值）
fn invalid_thread_count_strategy() -> impl Strategy<Value = usize> {
    prop_oneof![
        Just(0), // 零线程数
        500_000usize..=2_000_000usize, // 超过系统限制的大值
    ]
}

/// 生成无效队列容量策略
fn invalid_queue_capacity_strategy() -> impl Strategy<Value = usize> {
    Just(0)
}

/// 生成无效超时策略
fn invalid_timeout_strategy() -> impl Strategy<Value = Duration> {
    Just(Duration::ZERO)
}

/// 生成无效轮询间隔策略
fn invalid_poll_interval_strategy() -> impl Strategy<Value = Duration> {
    Just(Duration::ZERO)
}

/// 验证错误消息的清晰度
///
/// 清晰的错误消息应该包含：
/// 1. 错误类型的描述
/// 2. 无效的值
/// 3. 期望的值或约束条件
fn verify_error_message_clarity(error: &ConfigError, invalid_value_str: &str) {
    let error_msg = error.to_string();

    // 错误消息应该是非空的
    assert!(
        !error_msg.is_empty(),
        "Error message should not be empty"
    );

    // 错误消息应该包含无效的值（对于数值类型）
    // 注意：对于非常大的数字，可能会以不同格式显示
    let contains_value = error_msg.contains(invalid_value_str)
        || error_msg.contains(&invalid_value_str.replace(",", ""))
        || error_msg.contains(&invalid_value_str.replace("_", ""));

    assert!(
        contains_value,
        "Error message should contain the invalid value. Expected '{}' in '{}'",
        invalid_value_str,
        error_msg
    );

    // 错误消息应该包含约束条件的描述
    match error {
        ConfigError::InvalidThreadCount(_) => {
            assert!(
                error_msg.contains("must be") || error_msg.contains(">="),
                "Error message should describe the constraint: '{}'",
                error_msg
            );
        }
        ConfigError::InvalidQueueCapacity(_) => {
            assert!(
                error_msg.contains("must be") || error_msg.contains(">="),
                "Error message should describe the constraint: '{}'",
                error_msg
            );
        }
        ConfigError::InvalidTimeout(_) => {
            assert!(
                error_msg.contains("must be") || error_msg.contains("positive"),
                "Error message should describe the constraint: '{}'",
                error_msg
            );
        }
        ConfigError::InvalidPollInterval(_) => {
            assert!(
                error_msg.contains("must be") || error_msg.contains("positive"),
                "Error message should describe the constraint: '{}'",
                error_msg
            );
        }
        ConfigError::ThreadCountExceedsLimit(requested, limit) => {
            assert!(
                error_msg.contains(&requested.to_string()),
                "Error message should contain requested thread count: '{}'",
                error_msg
            );
            assert!(
                error_msg.contains(&limit.to_string()),
                "Error message should contain system limit: '{}'",
                error_msg
            );
            assert!(
                error_msg.contains("exceeds") || error_msg.contains("limit"),
                "Error message should describe the limit: '{}'",
                error_msg
            );
        }
    }

    // 错误消息应该以大写字母开头或包含错误类型关键词
    assert!(
        error_msg.chars().next().unwrap().is_uppercase()
            || error_msg.contains("Invalid")
            || error_msg.contains("exceeds"),
        "Error message should be properly formatted: '{}'",
        error_msg
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意无效的线程数，错误消息应该清晰描述问题
    ///
    /// 此测试验证：
    /// - 需求 5.6: 配置验证失败时提供清晰的错误消息
    #[test]
    fn prop_invalid_thread_count_error_message(
        thread_count in invalid_thread_count_strategy()
    ) {
        let result = PoolConfigBuilder::new()
            .thread_count(thread_count)
            .build();

        // 验证配置失败
        prop_assert!(result.is_err(), "Invalid thread count should fail validation");

        if let Err(error) = result {
            // 验证错误类型
            prop_assert!(
                matches!(
                    error,
                    ConfigError::InvalidThreadCount(_) | ConfigError::ThreadCountExceedsLimit(_, _)
                ),
                "Should be InvalidThreadCount or ThreadCountExceedsLimit error"
            );

            // 验证错误消息清晰度
            verify_error_message_clarity(&error, &thread_count.to_string());
        }
    }

    /// 属性测试：对于任意无效的队列容量，错误消息应该清晰描述问题
    ///
    /// 此测试验证：
    /// - 需求 5.6: 配置验证失败时提供清晰的错误消息
    #[test]
    fn prop_invalid_queue_capacity_error_message(
        capacity in invalid_queue_capacity_strategy()
    ) {
        let result = PoolConfigBuilder::new()
            .queue_capacity(capacity)
            .build();

        // 验证配置失败
        prop_assert!(result.is_err(), "Invalid queue capacity should fail validation");

        if let Err(error) = result {
            // 验证错误类型
            prop_assert!(
                matches!(error, ConfigError::InvalidQueueCapacity(_)),
                "Should be InvalidQueueCapacity error"
            );

            // 验证错误消息清晰度
            verify_error_message_clarity(&error, &capacity.to_string());
        }
    }

    /// 属性测试：对于任意无效的超时时间，错误消息应该清晰描述问题
    ///
    /// 此测试验证：
    /// - 需求 5.6: 配置验证失败时提供清晰的错误消息
    #[test]
    fn prop_invalid_timeout_error_message(
        timeout in invalid_timeout_strategy()
    ) {
        let result = PoolConfigBuilder::new()
            .default_timeout(timeout)
            .build();

        // 验证配置失败
        prop_assert!(result.is_err(), "Invalid timeout should fail validation");

        if let Err(error) = result {
            // 验证错误类型
            prop_assert!(
                matches!(error, ConfigError::InvalidTimeout(_)),
                "Should be InvalidTimeout error"
            );

            // 验证错误消息清晰度
            verify_error_message_clarity(&error, &format!("{:?}", timeout));
        }
    }

    /// 属性测试：对于任意无效的轮询间隔，错误消息应该清晰描述问题
    ///
    /// 此测试验证：
    /// - 需求 5.6: 配置验证失败时提供清晰的错误消息
    #[test]
    fn prop_invalid_poll_interval_error_message(
        interval in invalid_poll_interval_strategy()
    ) {
        let result = PoolConfigBuilder::new()
            .poll_interval(interval)
            .build();

        // 验证配置失败
        prop_assert!(result.is_err(), "Invalid poll interval should fail validation");

        if let Err(error) = result {
            // 验证错误类型
            prop_assert!(
                matches!(error, ConfigError::InvalidPollInterval(_)),
                "Should be InvalidPollInterval error"
            );

            // 验证错误消息清晰度
            verify_error_message_clarity(&error, &format!("{:?}", interval));
        }
    }
}

// 单元测试：验证特定错误场景的消息清晰度

#[test]
fn test_invalid_thread_count_zero_message() {
    let result = PoolConfigBuilder::new().thread_count(0).build();

    assert!(result.is_err(), "Thread count 0 should fail");

    if let Err(error) = result {
        let msg = error.to_string();

        // 验证消息包含关键信息
        assert!(msg.contains("Invalid thread count"));
        assert!(msg.contains("0"));
        assert!(msg.contains("must be >= 1"));

        // 验证错误类型
        assert!(matches!(error, ConfigError::InvalidThreadCount(0)));
    }
}

#[test]
fn test_thread_count_exceeds_limit_message() {
    let result = PoolConfigBuilder::new().thread_count(1_000_000).build();

    assert!(result.is_err(), "Very large thread count should fail");

    if let Err(error) = result {
        let msg = error.to_string();

        // 验证消息包含关键信息
        assert!(msg.contains("1000000") || msg.contains("1,000,000"));
        assert!(msg.contains("exceeds"));
        assert!(msg.contains("limit"));

        // 验证错误类型
        if let ConfigError::ThreadCountExceedsLimit(requested, limit) = error {
            assert_eq!(requested, 1_000_000);
            assert!(limit > 0);
            assert!(limit < 1_000_000);
        } else {
            panic!("Expected ThreadCountExceedsLimit error");
        }
    }
}

#[test]
fn test_invalid_queue_capacity_zero_message() {
    let result = PoolConfigBuilder::new().queue_capacity(0).build();

    assert!(result.is_err(), "Queue capacity 0 should fail");

    if let Err(error) = result {
        let msg = error.to_string();

        // 验证消息包含关键信息
        assert!(msg.contains("Invalid queue capacity"));
        assert!(msg.contains("0"));
        assert!(msg.contains("must be >= 1"));

        // 验证错误类型
        assert!(matches!(error, ConfigError::InvalidQueueCapacity(0)));
    }
}

#[test]
fn test_invalid_timeout_zero_message() {
    let result = PoolConfigBuilder::new()
        .default_timeout(Duration::ZERO)
        .build();

    assert!(result.is_err(), "Zero timeout should fail");

    if let Err(error) = result {
        let msg = error.to_string();

        // 验证消息包含关键信息
        assert!(msg.contains("Invalid timeout"));
        assert!(msg.contains("0s") || msg.contains("0ns"));
        assert!(msg.contains("must be positive"));

        // 验证错误类型
        assert!(matches!(error, ConfigError::InvalidTimeout(_)));
    }
}

#[test]
fn test_invalid_poll_interval_zero_message() {
    let result = PoolConfigBuilder::new()
        .poll_interval(Duration::ZERO)
        .build();

    assert!(result.is_err(), "Zero poll interval should fail");

    if let Err(error) = result {
        let msg = error.to_string();

        // 验证消息包含关键信息
        assert!(msg.contains("Invalid poll interval"));
        assert!(msg.contains("0s") || msg.contains("0ns"));
        assert!(msg.contains("must be positive"));

        // 验证错误类型
        assert!(matches!(error, ConfigError::InvalidPollInterval(_)));
    }
}

#[test]
fn test_error_message_format_consistency() {
    // 测试所有错误消息格式的一致性

    let errors = vec![
        PoolConfigBuilder::new().thread_count(0).build().unwrap_err(),
        PoolConfigBuilder::new()
            .queue_capacity(0)
            .build()
            .unwrap_err(),
        PoolConfigBuilder::new()
            .default_timeout(Duration::ZERO)
            .build()
            .unwrap_err(),
        PoolConfigBuilder::new()
            .poll_interval(Duration::ZERO)
            .build()
            .unwrap_err(),
    ];

    for error in errors {
        let msg = error.to_string();

        // 所有错误消息都应该：
        // 1. 非空
        assert!(!msg.is_empty(), "Error message should not be empty");

        // 2. 包含 "Invalid" 或其他错误类型关键词
        assert!(
            msg.contains("Invalid") || msg.contains("exceeds"),
            "Error message should contain error type: '{}'",
            msg
        );

        // 3. 包含约束条件
        assert!(
            msg.contains("must be") || msg.contains("positive") || msg.contains(">="),
            "Error message should contain constraint: '{}'",
            msg
        );

        // 4. 格式良好（以大写字母开头）
        assert!(
            msg.chars().next().unwrap().is_uppercase(),
            "Error message should start with uppercase: '{}'",
            msg
        );
    }
}

#[test]
fn test_error_debug_format() {
    // 测试 Debug 格式也包含有用信息
    let error = PoolConfigBuilder::new().thread_count(0).build().unwrap_err();

    let debug_msg = format!("{:?}", error);

    // Debug 格式应该包含错误类型和值
    assert!(debug_msg.contains("InvalidThreadCount"));
    assert!(debug_msg.contains("0"));
}

#[test]
fn test_multiple_invalid_configs() {
    // 测试多个无效配置时，第一个错误被正确报告
    let result = PoolConfigBuilder::new()
        .thread_count(0) // 第一个无效配置
        .queue_capacity(0) // 第二个无效配置
        .build();

    assert!(result.is_err(), "Multiple invalid configs should fail");

    if let Err(error) = result {
        // 应该报告第一个遇到的错误（线程数）
        assert!(
            matches!(error, ConfigError::InvalidThreadCount(_)),
            "Should report the first validation error"
        );

        verify_error_message_clarity(&error, "0");
    }
}
