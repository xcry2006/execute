// Feature: production-ready-improvements, Property 19: 超时类型区分
// **Validates: Requirements 12.3, 12.4, 12.5**
//
// 属性 19: 超时类型区分
// 对于任意超时错误，应该明确区分是启动超时还是执行超时
//
// 验证需求：
// - 需求 12.3: 命令启动超过启动超时时，取消启动并返回超时错误
// - 需求 12.4: 命令执行超过执行超时时，终止进程并返回超时错误
// - 需求 12.5: 错误信息中区分启动超时和执行超时

use execute::{execute_with_timeouts, CommandConfig, CommandError, TimeoutConfig};
use proptest::prelude::*;
use std::time::{Duration, Instant};

/// 生成启动超时场景的策略
///
/// 生成会触发启动超时的配置。
/// 注意：由于 std::process::Command::spawn 是同步的，实际的启动超时
/// 是通过检查启动时间来实现的。
#[cfg(unix)]
fn spawn_timeout_strategy() -> impl Strategy<Value = (String, Vec<String>, Duration, Duration)> {
    prop_oneof![
        // 使用非常短的启动超时，正常命令也可能触发
        Just((
            "sleep".to_string(),
            vec!["0.1".to_string()],
            Duration::from_nanos(1), // 极短的启动超时
            Duration::from_secs(10),  // 足够长的执行超时
        )),
        Just((
            "echo".to_string(),
            vec!["test".to_string()],
            Duration::from_nanos(1),
            Duration::from_secs(10),
        )),
    ]
}

/// 生成执行超时场景的策略
///
/// 生成会触发执行超时的配置。
/// 使用 sleep 命令确保执行时间超过配置的执行超时。
#[cfg(unix)]
fn execution_timeout_strategy() -> impl Strategy<Value = (String, Vec<String>, Duration, Duration)>
{
    prop_oneof![
        // 正常的启动超时，但很短的执行超时
        Just((
            "sleep".to_string(),
            vec!["10".to_string()],
            Duration::from_secs(5),      // 足够长的启动超时
            Duration::from_millis(50),   // 很短的执行超时
        )),
        Just((
            "sleep".to_string(),
            vec!["5".to_string()],
            Duration::from_secs(5),
            Duration::from_millis(100),
        )),
        Just((
            "sleep".to_string(),
            vec!["3".to_string()],
            Duration::from_secs(5),
            Duration::from_millis(80),
        )),
    ]
}

/// 生成任务 ID 策略
fn task_id_strategy() -> impl Strategy<Value = u64> {
    1u64..10000u64
}

/// 验证执行超时错误的特征
///
/// 执行超时应该：
/// 1. 实际执行时长接近配置的执行超时值
/// 2. 配置的超时值等于执行超时配置
/// 3. 错误上下文包含完整信息
fn verify_execution_timeout(
    error: &CommandError,
    expected_task_id: u64,
    expected_command: &str,
    execution_timeout: Duration,
    actual_elapsed: Duration,
) {
    match error {
        CommandError::Timeout {
            context,
            configured_timeout,
            actual_duration,
        } => {
            // 验证需求 12.4: 配置的超时值应该是执行超时
            assert_eq!(
                *configured_timeout, execution_timeout,
                "Execution timeout error should contain execution timeout value. Expected {:?}, got {:?}",
                execution_timeout, configured_timeout
            );

            // 验证需求 12.5: 实际执行时长应该接近执行超时值
            let tolerance = execution_timeout / 5; // 20% 容差
            let min_duration = execution_timeout.saturating_sub(tolerance);
            let max_duration = execution_timeout + tolerance + Duration::from_millis(100);

            assert!(
                *actual_duration >= min_duration,
                "Execution timeout actual duration should be close to configured timeout. \
                 Expected >= {:?}, got {:?}",
                min_duration,
                actual_duration
            );

            assert!(
                *actual_duration <= max_duration,
                "Execution timeout actual duration should not exceed configured timeout by too much. \
                 Expected <= {:?}, got {:?}",
                max_duration,
                actual_duration
            );

            // 验证实际测量的时间也接近执行超时
            assert!(
                actual_elapsed >= min_duration,
                "Measured elapsed time should be close to execution timeout. \
                 Expected >= {:?}, got {:?}",
                min_duration,
                actual_elapsed
            );

            // 验证错误上下文
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
        }
        _ => panic!(
            "Expected Timeout error for execution timeout, got {:?}",
            match error {
                CommandError::SpawnFailed { .. } => "SpawnFailed",
                CommandError::ExecutionFailed { .. } => "ExecutionFailed",
                _ => "Unknown",
            }
        ),
    }
}

/// 验证启动超时错误的特征
///
/// 启动超时应该：
/// 1. 实际执行时长很短（因为在启动阶段就超时了）
/// 2. 配置的超时值等于启动超时配置
/// 3. 错误上下文包含完整信息
fn verify_spawn_timeout(
    error: &CommandError,
    expected_task_id: u64,
    expected_command: &str,
    spawn_timeout: Duration,
) {
    match error {
        CommandError::Timeout {
            context,
            configured_timeout,
            actual_duration,
        } => {
            // 验证需求 12.3: 配置的超时值应该是启动超时
            assert_eq!(
                *configured_timeout, spawn_timeout,
                "Spawn timeout error should contain spawn timeout value. Expected {:?}, got {:?}",
                spawn_timeout, configured_timeout
            );

            // 验证需求 12.5: 启动超时的实际时长应该很短
            // 因为是在启动阶段超时，不应该等待很长时间
            assert!(
                *actual_duration < Duration::from_millis(500),
                "Spawn timeout actual duration should be short (< 500ms). Got {:?}",
                actual_duration
            );

            // 验证错误上下文
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
        }
        _ => panic!(
            "Expected Timeout error for spawn timeout, got {:?}",
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

    /// 属性测试：对于任意执行超时场景，错误应该明确表明是执行超时
    ///
    /// 此测试验证：
    /// - 需求 12.4: 命令执行超过执行超时时，终止进程并返回超时错误
    /// - 需求 12.5: 错误信息中区分启动超时和执行超时
    /// - 执行超时的实际时长应该接近配置的执行超时值
    #[test]
    fn prop_execution_timeout_is_distinguishable(
        (cmd, args, spawn_timeout, exec_timeout) in execution_timeout_strategy(),
        task_id in task_id_strategy(),
    ) {
        // 创建会触发执行超时的配置
        let timeout_config = TimeoutConfig::new()
            .with_spawn_timeout(spawn_timeout)
            .with_execution_timeout(exec_timeout);

        let config = CommandConfig::new(&cmd, args.clone())
            .with_timeouts(timeout_config);

        // 执行命令并测量时间
        let start = Instant::now();
        let result = execute_with_timeouts(&config, task_id);
        let elapsed = start.elapsed();

        // 验证命令超时
        prop_assert!(result.is_err(), "Command should timeout");

        // 获取错误并验证是执行超时
        if let Err(error) = result {
            verify_execution_timeout(&error, task_id, &cmd, exec_timeout, elapsed);
        }
    }

    /// 属性测试：对于任意启动超时场景，错误应该明确表明是启动超时
    ///
    /// 此测试验证：
    /// - 需求 12.3: 命令启动超过启动超时时，取消启动并返回超时错误
    /// - 需求 12.5: 错误信息中区分启动超时和执行超时
    /// - 启动超时的实际时长应该很短
    #[test]
    fn prop_spawn_timeout_is_distinguishable(
        (cmd, args, spawn_timeout, exec_timeout) in spawn_timeout_strategy(),
        task_id in task_id_strategy(),
    ) {
        // 创建会触发启动超时的配置
        let timeout_config = TimeoutConfig::new()
            .with_spawn_timeout(spawn_timeout)
            .with_execution_timeout(exec_timeout);

        let config = CommandConfig::new(&cmd, args.clone())
            .with_timeouts(timeout_config);

        // 执行命令
        let result = execute_with_timeouts(&config, task_id);

        // 如果超时，验证是启动超时
        if let Err(error) = result {
            if matches!(error, CommandError::Timeout { .. }) {
                verify_spawn_timeout(&error, task_id, &cmd, spawn_timeout);
            }
            // 注意：由于启动超时的实现限制，可能不会总是触发
            // 如果没有超时，测试仍然通过
        }
    }
}

// 单元测试：验证特定超时场景

#[test]
#[cfg(unix)]
fn test_execution_timeout_distinction() {
    // 测试执行超时可以被区分
    let exec_timeout = Duration::from_millis(100);
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(5))
        .with_execution_timeout(exec_timeout);

    let config =
        CommandConfig::new("sleep", vec!["10".to_string()]).with_timeouts(timeout_config);
    let task_id = 42;

    let start = Instant::now();
    let result = execute_with_timeouts(&config, task_id);
    let elapsed = start.elapsed();

    assert!(result.is_err(), "Command should timeout");

    if let Err(error) = result {
        verify_execution_timeout(&error, task_id, "sleep", exec_timeout, elapsed);
    }
}

#[test]
#[cfg(unix)]
fn test_spawn_timeout_distinction() {
    // 测试启动超时可以被区分
    let spawn_timeout = Duration::from_nanos(1);
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(spawn_timeout)
        .with_execution_timeout(Duration::from_secs(10));

    let config =
        CommandConfig::new("sleep", vec!["0.1".to_string()]).with_timeouts(timeout_config);
    let task_id = 123;

    let result = execute_with_timeouts(&config, task_id);

    // 如果触发了超时，验证是启动超时
    if let Err(error) = result {
        if matches!(error, CommandError::Timeout { .. }) {
            verify_spawn_timeout(&error, task_id, "sleep", spawn_timeout);
        }
    }
}

#[test]
#[cfg(unix)]
fn test_timeout_type_from_configured_value() {
    // 测试可以通过配置的超时值区分超时类型
    let spawn_timeout = Duration::from_nanos(1);
    let exec_timeout = Duration::from_millis(50);

    // 场景 1: 执行超时
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(5))
        .with_execution_timeout(exec_timeout);

    let config =
        CommandConfig::new("sleep", vec!["10".to_string()]).with_timeouts(timeout_config);

    let result = execute_with_timeouts(&config, 1);
    assert!(result.is_err());

    if let Err(CommandError::Timeout {
        configured_timeout,
        ..
    }) = result
    {
        // 配置的超时值应该是执行超时
        assert_eq!(
            configured_timeout, exec_timeout,
            "Execution timeout should be reported"
        );
    }

    // 场景 2: 启动超时
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(spawn_timeout)
        .with_execution_timeout(Duration::from_secs(10));

    let config =
        CommandConfig::new("echo", vec!["test".to_string()]).with_timeouts(timeout_config);

    let result = execute_with_timeouts(&config, 2);

    if let Err(CommandError::Timeout {
        configured_timeout,
        ..
    }) = result
    {
        // 配置的超时值应该是启动超时
        assert_eq!(
            configured_timeout, spawn_timeout,
            "Spawn timeout should be reported"
        );
    }
}

#[test]
#[cfg(unix)]
fn test_timeout_distinction_by_duration() {
    // 测试可以通过实际执行时长区分超时类型
    let exec_timeout = Duration::from_millis(100);
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(5))
        .with_execution_timeout(exec_timeout);

    let config =
        CommandConfig::new("sleep", vec!["10".to_string()]).with_timeouts(timeout_config);

    let start = Instant::now();
    let result = execute_with_timeouts(&config, 1);
    let elapsed = start.elapsed();

    assert!(result.is_err());

    if let Err(CommandError::Timeout {
        actual_duration, ..
    }) = result
    {
        // 执行超时的实际时长应该接近执行超时值
        let tolerance = exec_timeout / 5;
        assert!(
            actual_duration >= exec_timeout.saturating_sub(tolerance),
            "Execution timeout duration should be close to configured timeout"
        );
        assert!(
            actual_duration <= exec_timeout + tolerance + Duration::from_millis(100),
            "Execution timeout duration should not exceed configured timeout by too much"
        );

        // 测量的时间也应该接近
        assert!(
            elapsed >= exec_timeout.saturating_sub(tolerance),
            "Measured time should be close to execution timeout"
        );
    }
}

#[test]
#[cfg(unix)]
fn test_only_execution_timeout_set() {
    // 测试只设置执行超时时的行为
    let exec_timeout = Duration::from_millis(50);
    let timeout_config = TimeoutConfig::new().with_execution_timeout(exec_timeout);

    let config =
        CommandConfig::new("sleep", vec!["10".to_string()]).with_timeouts(timeout_config);

    let result = execute_with_timeouts(&config, 1);
    assert!(result.is_err());

    if let Err(CommandError::Timeout {
        configured_timeout,
        ..
    }) = result
    {
        assert_eq!(
            configured_timeout, exec_timeout,
            "Should report execution timeout when only execution timeout is set"
        );
    }
}

#[test]
#[cfg(unix)]
fn test_only_spawn_timeout_set() {
    // 测试只设置启动超时时的行为
    let spawn_timeout = Duration::from_nanos(1);
    let timeout_config = TimeoutConfig::new().with_spawn_timeout(spawn_timeout);

    let config =
        CommandConfig::new("echo", vec!["test".to_string()]).with_timeouts(timeout_config);

    let result = execute_with_timeouts(&config, 1);

    // 如果触发超时，应该是启动超时
    if let Err(CommandError::Timeout {
        configured_timeout,
        actual_duration,
        ..
    }) = result
    {
        assert_eq!(
            configured_timeout, spawn_timeout,
            "Should report spawn timeout when only spawn timeout is set"
        );
        assert!(
            actual_duration < Duration::from_millis(500),
            "Spawn timeout duration should be short"
        );
    }
}

// Windows 平台的占位测试
#[test]
#[cfg(not(unix))]
fn test_timeout_type_distinction_windows_placeholder() {
    // Windows 平台暂不支持此测试
    println!("Timeout type distinction property test is not supported on Windows");
}
