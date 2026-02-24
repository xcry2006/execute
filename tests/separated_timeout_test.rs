use execute::{CommandConfig, TimeoutConfig, execute_with_timeouts};
use std::time::Duration;

#[test]
#[cfg(unix)]
fn test_execute_with_spawn_timeout_fast_spawn() {
    // 测试快速启动的命令不会触发启动超时
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(5))
        .with_execution_timeout(Duration::from_secs(10));

    let config =
        CommandConfig::new("echo", vec!["hello".to_string()]).with_timeouts(timeout_config);

    let result = execute_with_timeouts(&config, 1);
    assert!(result.is_ok(), "Fast spawn should succeed");
}

#[test]
#[cfg(unix)]
fn test_execute_with_execution_timeout() {
    // 测试执行超时
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(5))
        .with_execution_timeout(Duration::from_millis(100));

    let config = CommandConfig::new("sleep", vec!["10".to_string()]).with_timeouts(timeout_config);

    let result = execute_with_timeouts(&config, 1);
    assert!(result.is_err(), "Long running command should timeout");

    // 验证是执行超时错误
    if let Err(e) = result {
        let error_msg = format!("{}", e);
        assert!(error_msg.contains("Timeout") || error_msg.contains("timeout"));
    }
}

#[test]
#[cfg(unix)]
fn test_execute_with_only_execution_timeout() {
    // 测试只设置执行超时
    let timeout_config = TimeoutConfig::new().with_execution_timeout(Duration::from_secs(1));

    let config =
        CommandConfig::new("echo", vec!["hello".to_string()]).with_timeouts(timeout_config);

    let result = execute_with_timeouts(&config, 1);
    assert!(result.is_ok(), "Command should complete within timeout");
}

#[test]
#[cfg(unix)]
fn test_execute_with_only_spawn_timeout() {
    // 测试只设置启动超时
    let timeout_config = TimeoutConfig::new().with_spawn_timeout(Duration::from_secs(5));

    let config =
        CommandConfig::new("echo", vec!["hello".to_string()]).with_timeouts(timeout_config);

    let result = execute_with_timeouts(&config, 1);
    assert!(result.is_ok(), "Command should complete successfully");
}

#[test]
#[cfg(unix)]
fn test_execute_without_timeout_config_fallback() {
    // 测试没有超时配置时回退到标准执行
    let config = CommandConfig::new("echo", vec!["hello".to_string()]);

    let result = execute_with_timeouts(&config, 1);
    assert!(result.is_ok(), "Command should execute successfully");
}

#[test]
#[cfg(unix)]
fn test_execute_with_both_timeouts() {
    // 测试同时设置启动和执行超时
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(2))
        .with_execution_timeout(Duration::from_secs(5));

    let config = CommandConfig::new("echo", vec!["test".to_string()]).with_timeouts(timeout_config);

    let result = execute_with_timeouts(&config, 1);
    assert!(result.is_ok(), "Command should complete successfully");

    if let Ok(output) = result {
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("test"));
    }
}

#[test]
#[cfg(unix)]
fn test_execution_timeout_kills_process() {
    // 测试执行超时会终止进程
    let timeout_config = TimeoutConfig::new().with_execution_timeout(Duration::from_millis(200));

    let config = CommandConfig::new("sleep", vec!["5".to_string()]).with_timeouts(timeout_config);

    let start = std::time::Instant::now();
    let result = execute_with_timeouts(&config, 1);
    let elapsed = start.elapsed();

    assert!(result.is_err(), "Command should timeout");
    assert!(elapsed < Duration::from_secs(1), "Should timeout quickly");
}
