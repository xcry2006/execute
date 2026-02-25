// Feature: production-ready-improvements, Property 6: 超时错误详情
// **Validates: Requirements 3.5**
//
// 属性 6: 超时错误详情
// 对于任意超时的命令，错误信息应该包含配置的超时值和实际执行时长
//
// 验证需求：
// - 需求 3.5: 超时发生时，错误信息中包含配置的超时值和实际执行时长

use execute::{CommandConfig, CommandError, execute_command_with_context};
use proptest::prelude::*;
use std::time::Duration;

/// 生成超时命令策略
///
/// 生成会超时的命令配置，包括命令、参数和超时时间。
/// 使用 sleep 命令确保命令执行时间超过配置的超时时间。
#[cfg(unix)]
fn timeout_command_strategy() -> impl Strategy<Value = (String, Vec<String>, Duration)> {
    prop_oneof![
        // sleep 命令，执行时间远超超时时间
        Just(("sleep".to_string(), vec!["10".to_string()], Duration::from_millis(50))),
        Just(("sleep".to_string(), vec!["5".to_string()], Duration::from_millis(100))),
        Just(("sleep".to_string(), vec!["3".to_string()], Duration::from_millis(80))),
        Just(("sleep".to_string(), vec!["2".to_string()], Duration::from_millis(60))),
        // 不同的超时值
        Just(("sleep".to_string(), vec!["10".to_string()], Duration::from_millis(30))),
        Just(("sleep".to_string(), vec!["10".to_string()], Duration::from_millis(70))),
        Just(("sleep".to_string(), vec!["10".to_string()], Duration::from_millis(120))),
    ]
}

/// 生成任务 ID 策略
fn task_id_strategy() -> impl Strategy<Value = u64> {
    1u64..10000u64
}

/// 验证超时错误包含所有必需的详情
///
/// 此函数验证超时错误包含：
/// 1. 配置的超时值（与设置的超时时间一致）
/// 2. 实际执行时长（应该接近配置的超时值）
/// 3. 完整的错误上下文（task_id、command、working_dir、timestamp）
fn verify_timeout_error_details(
    error: &CommandError,
    expected_task_id: u64,
    expected_command: &str,
    expected_timeout: Duration,
) {
    match error {
        CommandError::Timeout {
            context,
            configured_timeout,
            actual_duration,
        } => {
            // 验证需求 3.5: 包含配置的超时值
            assert_eq!(
                *configured_timeout, expected_timeout,
                "Timeout error should contain the configured timeout value. Expected {:?}, got {:?}",
                expected_timeout, configured_timeout
            );

            // 验证需求 3.5: 包含实际执行时长
            // 实际执行时长应该接近配置的超时值（允许一些时间误差）
            // 由于系统调度和计时精度，实际时长可能略小于或略大于超时值
            let tolerance = expected_timeout / 10; // 10% 容差
            let min_duration = expected_timeout.saturating_sub(tolerance);
            let max_duration = expected_timeout + tolerance + Duration::from_millis(50); // 额外 50ms 用于进程终止

            assert!(
                *actual_duration >= min_duration,
                "Actual duration should be close to configured timeout. \
                 Expected >= {:?}, got {:?}",
                min_duration,
                actual_duration
            );

            assert!(
                *actual_duration <= max_duration,
                "Actual duration should not exceed configured timeout by too much. \
                 Expected <= {:?}, got {:?}",
                max_duration,
                actual_duration
            );

            // 验证错误上下文完整性（需求 3.1-3.4）
            assert_eq!(
                context.task_id, expected_task_id,
                "Error context should contain correct task_id"
            );

            assert!(
                context.command.contains(expected_command),
                "Error context should contain command string. Expected '{}' in '{}'",
                expected_command,
                context.command
            );

            // 验证时间戳是最近的
            let now = std::time::SystemTime::now();
            let elapsed = now
                .duration_since(context.timestamp)
                .expect("Timestamp should be in the past");
            assert!(
                elapsed < Duration::from_secs(10),
                "Error context timestamp should be recent (within 10 seconds). Elapsed: {:?}",
                elapsed
            );
        }
        _ => panic!(
            "Expected Timeout error, got {:?}",
            match error {
                CommandError::SpawnFailed { .. } => "SpawnFailed",
                CommandError::ExecutionFailed { .. } => "ExecutionFailed",
                _ => "Unknown",
            }
        ),
    }
}

#[cfg(unix)]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意超时的命令，错误应该包含配置的超时值和实际执行时长
    ///
    /// 此测试验证：
    /// - 需求 3.5: 超时错误包含配置的超时值
    /// - 需求 3.5: 超时错误包含实际执行时长
    /// - 实际执行时长应该接近配置的超时值
    #[test]
    fn prop_timeout_error_contains_timeout_details(
        (cmd, args, timeout) in timeout_command_strategy(),
        task_id in task_id_strategy(),
    ) {
        // 创建会超时的命令配置
        let config = CommandConfig::new(&cmd, args.clone())
            .with_timeout(timeout);

        // 执行命令（应该超时）
        let result = execute_command_with_context(&config, task_id);

        // 验证命令超时
        prop_assert!(result.is_err(), "Command should timeout");

        // 获取错误并验证超时详情
        if let Err(error) = result {
            verify_timeout_error_details(&error, task_id, &cmd, timeout);
        }
    }
}

// 单元测试：验证特定超时场景

#[test]
#[cfg(unix)]
fn test_timeout_error_with_50ms_timeout() {
    let timeout = Duration::from_millis(50);
    let config = CommandConfig::new("sleep", vec!["10".to_string()]).with_timeout(timeout);
    let task_id = 42;

    let result = execute_command_with_context(&config, task_id);

    assert!(result.is_err(), "Command should timeout");

    if let Err(error) = result {
        verify_timeout_error_details(&error, task_id, "sleep", timeout);
    }
}

#[test]
#[cfg(unix)]
fn test_timeout_error_with_100ms_timeout() {
    let timeout = Duration::from_millis(100);
    let config = CommandConfig::new("sleep", vec!["5".to_string()]).with_timeout(timeout);
    let task_id = 123;

    let result = execute_command_with_context(&config, task_id);

    assert!(result.is_err(), "Command should timeout");

    if let Err(error) = result {
        verify_timeout_error_details(&error, task_id, "sleep", timeout);
    }
}

#[test]
#[cfg(unix)]
fn test_timeout_error_display_format() {
    let timeout = Duration::from_millis(50);
    let config = CommandConfig::new("sleep", vec!["10".to_string()]).with_timeout(timeout);
    let task_id = 999;

    let result = execute_command_with_context(&config, task_id);

    assert!(result.is_err(), "Command should timeout");

    if let Err(error) = result {
        let error_string = format!("{}", error);

        // 验证错误消息包含所有关键信息
        assert!(
            error_string.contains("timeout") || error_string.contains("Timeout"),
            "Error message should mention timeout"
        );
        assert!(
            error_string.contains("task_id=999"),
            "Error message should contain task_id"
        );
        assert!(
            error_string.contains("sleep"),
            "Error message should contain command"
        );
        assert!(
            error_string.contains("configured_timeout"),
            "Error message should contain configured_timeout"
        );
        assert!(
            error_string.contains("actual_duration"),
            "Error message should contain actual_duration"
        );
    }
}

#[test]
#[cfg(unix)]
fn test_timeout_error_actual_duration_accuracy() {
    // 测试实际执行时长的准确性
    let timeout = Duration::from_millis(100);
    let config = CommandConfig::new("sleep", vec!["10".to_string()]).with_timeout(timeout);
    let task_id = 555;

    let start = std::time::Instant::now();
    let result = execute_command_with_context(&config, task_id);
    let elapsed = start.elapsed();

    assert!(result.is_err(), "Command should timeout");

    if let Err(CommandError::Timeout {
        actual_duration, ..
    }) = result
    {
        // 实际执行时长应该与我们测量的时间接近
        let diff = actual_duration.abs_diff(elapsed);

        assert!(
            diff < Duration::from_millis(50),
            "Actual duration should match measured time. Measured: {:?}, Reported: {:?}, Diff: {:?}",
            elapsed,
            actual_duration,
            diff
        );
    }
}

#[test]
#[cfg(unix)]
fn test_timeout_error_with_different_timeouts() {
    // 测试不同的超时值都能正确报告
    let timeouts = [
        Duration::from_millis(30),
        Duration::from_millis(50),
        Duration::from_millis(80),
        Duration::from_millis(120),
    ];

    for (i, timeout) in timeouts.iter().enumerate() {
        let config = CommandConfig::new("sleep", vec!["10".to_string()]).with_timeout(*timeout);
        let task_id = i as u64;

        let result = execute_command_with_context(&config, task_id);

        assert!(result.is_err(), "Command should timeout");

        if let Err(error) = result {
            verify_timeout_error_details(&error, task_id, "sleep", *timeout);
        }
    }
}

#[test]
#[cfg(unix)]
fn test_timeout_error_context_completeness() {
    // 验证超时错误包含完整的错误上下文
    let timeout = Duration::from_millis(50);
    let config = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_timeout(timeout)
        .with_working_dir("/tmp");
    let task_id = 777;

    let result = execute_command_with_context(&config, task_id);

    assert!(result.is_err(), "Command should timeout");

    if let Err(CommandError::Timeout { context, .. }) = result {
        // 验证所有上下文字段
        assert_eq!(context.task_id, task_id);
        assert!(context.command.contains("sleep"));
        assert!(context.command.contains("10"));
        assert!(
            context.working_dir.to_string_lossy().contains("/tmp")
                || context.working_dir.to_string_lossy().contains("tmp")
        );

        // 验证时间戳
        let now = std::time::SystemTime::now();
        let elapsed = now
            .duration_since(context.timestamp)
            .expect("Timestamp should be in the past");
        assert!(elapsed < Duration::from_secs(5));
    }
}

// Windows 平台的占位测试
#[test]
#[cfg(not(unix))]
fn test_timeout_error_property_windows_placeholder() {
    // Windows 平台暂不支持此测试
    // 可以在未来添加 Windows 特定的超时测试
    println!("Timeout error property test is not supported on Windows");
}
