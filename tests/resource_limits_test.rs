use execute::{CommandConfig, ResourceLimits, execute_command_with_context};

#[test]
fn test_resource_limits_configuration() {
    // 测试创建资源限制配置
    let limits = ResourceLimits::new()
        .with_max_output_size(1024)
        .with_max_memory(100 * 1024 * 1024);

    assert_eq!(limits.max_output_size, Some(1024));
    assert_eq!(limits.max_memory, Some(100 * 1024 * 1024));
}

#[test]
fn test_command_config_with_resource_limits() {
    // 测试在 CommandConfig 中设置资源限制
    let limits = ResourceLimits::new().with_max_output_size(2048);

    let config = CommandConfig::new("echo", vec!["hello".to_string()]).with_resource_limits(limits);

    assert!(config.resource_limits().is_some());
    assert_eq!(
        config.resource_limits().unwrap().max_output_size,
        Some(2048)
    );
}

#[test]
#[cfg(unix)]
fn test_execute_command_with_output_limit() {
    // 测试输出大小限制
    // 创建一个会产生大量输出的命令
    let limits = ResourceLimits::new().with_max_output_size(100); // 限制为 100 字节

    let config = CommandConfig::new("echo", vec!["hello world from test".to_string()])
        .with_resource_limits(limits);

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 应该成功执行
    assert!(result.is_ok(), "Command should execute successfully");

    let output = result.unwrap();
    // 输出应该被限制
    assert!(
        output.stdout.len() <= 100,
        "Output should be limited to 100 bytes"
    );
}

#[test]
#[cfg(unix)]
fn test_execute_command_without_limits() {
    // 测试没有资源限制的情况
    let config = CommandConfig::new("echo", vec!["hello".to_string()]);

    let result = execute_command_with_context(&config, 1);

    assert!(result.is_ok(), "Command should execute successfully");
    let output = result.unwrap();
    assert!(output.status.success());
}

#[test]
fn test_default_resource_limits() {
    // 测试默认资源限制
    let limits = ResourceLimits::default();

    assert_eq!(limits.max_output_size, None);
    assert_eq!(limits.max_memory, None);
}
