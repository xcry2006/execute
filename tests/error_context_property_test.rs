// Feature: production-ready-improvements, Property 5: 错误上下文完整性
// **Validates: Requirements 3.1, 3.2, 3.3, 3.4**
//
// 属性 5: 错误上下文完整性
// 对于任意失败的命令，错误信息应该包含命令字符串、工作目录、任务 ID 和时间戳
//
// 验证需求：
// - 需求 3.1: 命令执行失败时，错误信息中包含完整的命令字符串
// - 需求 3.2: 命令执行失败时，错误信息中包含工作目录
// - 需求 3.3: 命令执行失败时，错误信息中包含任务 ID
// - 需求 3.4: 命令执行失败时，错误信息中包含失败时间戳

use execute::{CommandConfig, CommandError, execute_command_with_context};
use proptest::prelude::*;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// 生成会失败的命令策略（只包含会导致 spawn 失败的命令）
fn failing_command_strategy() -> impl Strategy<Value = (String, Vec<String>)> {
    prop_oneof![
        // 不存在的命令
        Just(("nonexistent_command_xyz".to_string(), vec![])),
        Just(("invalid_cmd_123".to_string(), vec![])),
        Just(("fake_program".to_string(), vec!["arg1".to_string()])),
        Just(("missing_executable".to_string(), vec![])),
        Just(("not_a_real_command".to_string(), vec!["test".to_string()])),
    ]
}

/// 生成任务 ID 策略
fn task_id_strategy() -> impl Strategy<Value = u64> {
    1u64..10000u64
}

/// 生成工作目录策略
fn working_dir_strategy() -> impl Strategy<Value = Option<PathBuf>> {
    prop_oneof![
        Just(None),
        Just(Some(PathBuf::from("/tmp"))),
        Just(Some(PathBuf::from("."))),
        #[cfg(unix)]
        Just(Some(PathBuf::from("/var/tmp"))),
    ]
}

/// 生成超时命令策略（用于测试超时错误）
#[cfg(unix)]
fn timeout_command_strategy() -> impl Strategy<Value = (String, Vec<String>, Duration)> {
    prop_oneof![
        Just((
            "sleep".to_string(),
            vec!["10".to_string()],
            Duration::from_millis(50)
        )),
        Just((
            "sleep".to_string(),
            vec!["5".to_string()],
            Duration::from_millis(100)
        )),
    ]
}

/// 验证 ErrorContext 包含所有必需字段
fn verify_error_context(
    error: &CommandError,
    expected_task_id: u64,
    expected_command_contains: &str,
    expected_working_dir: &str,
) {
    match error {
        CommandError::SpawnFailed { context, .. }
        | CommandError::ExecutionFailed { context, .. }
        | CommandError::Timeout { context, .. } => {
            // 验证需求 3.3: 包含任务 ID
            assert_eq!(
                context.task_id, expected_task_id,
                "Error context should contain correct task_id"
            );

            // 验证需求 3.1: 包含完整的命令字符串
            assert!(
                context.command.contains(expected_command_contains),
                "Error context should contain command string. Expected '{}' in '{}'",
                expected_command_contains,
                context.command
            );

            // 验证需求 3.2: 包含工作目录
            let working_dir_str = context.working_dir.to_string_lossy();
            assert!(
                working_dir_str.contains(expected_working_dir)
                    || expected_working_dir.contains(&*working_dir_str),
                "Error context should contain working directory. Expected '{}', got '{}'",
                expected_working_dir,
                working_dir_str
            );

            // 验证需求 3.4: 包含时间戳
            // 时间戳应该是最近的（在过去 10 秒内）
            let now = SystemTime::now();
            let elapsed = now
                .duration_since(context.timestamp)
                .expect("Timestamp should be in the past");
            assert!(
                elapsed < Duration::from_secs(10),
                "Error context timestamp should be recent (within 10 seconds). Elapsed: {:?}",
                elapsed
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意失败的命令，错误上下文应该包含所有必需信息
    ///
    /// 此测试验证：
    /// - 需求 3.1: 错误信息包含完整的命令字符串
    /// - 需求 3.2: 错误信息包含工作目录
    /// - 需求 3.3: 错误信息包含任务 ID
    /// - 需求 3.4: 错误信息包含失败时间戳
    #[test]
    fn prop_error_context_completeness_for_spawn_failures(
        (cmd, args) in failing_command_strategy(),
        task_id in task_id_strategy(),
        working_dir in working_dir_strategy(),
    ) {
        // 创建命令配置
        let mut config = CommandConfig::new(&cmd, args.clone());
        if let Some(dir) = &working_dir {
            config = config.with_working_dir(&dir.to_string_lossy());
        }

        // 执行命令（应该失败）
        let result = execute_command_with_context(&config, task_id);

        // 验证命令失败
        prop_assert!(result.is_err(), "Command should fail");

        // 获取错误并验证上下文
        if let Err(error) = result {
            let expected_working_dir = working_dir
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());

            verify_error_context(&error, task_id, &cmd, &expected_working_dir);

            // 对于 SpawnFailed 错误，验证错误类型正确
            if matches!(error, CommandError::SpawnFailed { .. }) {
                // SpawnFailed 错误应该包含源错误
                prop_assert!(
                    format!("{:?}", error).contains("source"),
                    "SpawnFailed error should contain source error"
                );
            }
        }
    }
}

#[cfg(unix)]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// 属性测试：对于任意超时的命令，错误上下文应该包含所有必需信息
    ///
    /// 此测试验证超时错误也包含完整的错误上下文
    #[test]
    fn prop_error_context_completeness_for_timeout(
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

        // 获取错误并验证上下文
        if let Err(error) = result {
            verify_error_context(&error, task_id, &cmd, ".");

            // 对于 Timeout 错误，验证包含超时信息
            if let CommandError::Timeout {
                configured_timeout,
                actual_duration,
                ..
            } = error
            {
                prop_assert_eq!(
                    configured_timeout, timeout,
                    "Timeout error should contain configured timeout"
                );
                // actual_duration should be close to timeout (within a small margin)
                // Due to timing precision, it might be slightly less or more
                prop_assert!(
                    actual_duration >= timeout * 9 / 10,
                    "Actual duration should be close to the configured timeout. Expected ~{:?}, got {:?}",
                    timeout,
                    actual_duration
                );
            } else {
                return Err(TestCaseError::fail("Expected Timeout error"));
            }
        }
    }
}

// 单元测试：验证特定错误场景

#[test]
fn test_error_context_for_nonexistent_command() {
    let config = CommandConfig::new("nonexistent_command_xyz", vec![]);
    let task_id = 42;

    let result = execute_command_with_context(&config, task_id);

    assert!(result.is_err(), "Command should fail");

    if let Err(error) = result {
        // 验证错误类型
        assert!(
            matches!(error, CommandError::SpawnFailed { .. }),
            "Should be SpawnFailed error"
        );

        // 验证错误上下文
        verify_error_context(&error, task_id, "nonexistent_command_xyz", ".");
    }
}

#[test]
fn test_error_context_with_custom_working_dir() {
    let config =
        CommandConfig::new("invalid_cmd", vec!["arg1".to_string()]).with_working_dir("/tmp");
    let task_id = 123;

    let result = execute_command_with_context(&config, task_id);

    assert!(result.is_err(), "Command should fail");

    if let Err(error) = result {
        verify_error_context(&error, task_id, "invalid_cmd", "/tmp");
    }
}

#[test]
#[cfg(unix)]
fn test_error_context_for_timeout() {
    let config =
        CommandConfig::new("sleep", vec!["10".to_string()]).with_timeout(Duration::from_millis(50));
    let task_id = 999;

    let result = execute_command_with_context(&config, task_id);

    assert!(result.is_err(), "Command should timeout");

    if let Err(error) = result {
        // 验证错误类型
        assert!(
            matches!(error, CommandError::Timeout { .. }),
            "Should be Timeout error"
        );

        // 验证错误上下文
        verify_error_context(&error, task_id, "sleep", ".");

        // 验证超时信息（需求 3.5 的一部分）
        if let CommandError::Timeout {
            configured_timeout,
            actual_duration,
            ..
        } = error
        {
            assert_eq!(configured_timeout, Duration::from_millis(50));
            // Allow some timing tolerance
            assert!(
                actual_duration >= Duration::from_millis(45),
                "Actual duration should be close to timeout: {:?}",
                actual_duration
            );
        }
    }
}

#[test]
fn test_error_context_display_format() {
    let config = CommandConfig::new("test_cmd", vec!["arg1".to_string(), "arg2".to_string()])
        .with_working_dir("/test/dir");
    let task_id = 777;

    let result = execute_command_with_context(&config, task_id);

    if let Err(error) = result {
        let error_string = format!("{}", error);

        // 验证错误消息包含所有关键信息
        assert!(
            error_string.contains("task_id=777"),
            "Error message should contain task_id"
        );
        assert!(
            error_string.contains("test_cmd"),
            "Error message should contain command"
        );
        assert!(
            error_string.contains("/test/dir") || error_string.contains("test/dir"),
            "Error message should contain working directory"
        );
    }
}

#[test]
fn test_error_context_with_multiple_args() {
    let args = vec!["arg1".to_string(), "arg2".to_string(), "arg3".to_string()];
    let config = CommandConfig::new("fake_program", args.clone());
    let task_id = 555;

    let result = execute_command_with_context(&config, task_id);

    assert!(result.is_err(), "Command should fail");

    if let Err(error) = result {
        verify_error_context(&error, task_id, "fake_program", ".");

        // 验证命令字符串包含所有参数
        let error_string = format!("{}", error);
        for arg in &args {
            assert!(
                error_string.contains(arg),
                "Error message should contain argument: {}",
                arg
            );
        }
    }
}
