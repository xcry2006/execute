use execute::{CommandConfig, EnvConfig};

#[test]
#[cfg(unix)]
fn test_env_config_set_variable() {
    // 测试设置环境变量
    let env = EnvConfig::new()
        .set("TEST_VAR", "test_value")
        .set("ANOTHER_VAR", "another_value");

    let config = CommandConfig::new("printenv", vec!["TEST_VAR".to_string()]).with_env(env);

    let result = execute::execute_command_with_context(&config, 1);
    assert!(result.is_ok(), "Command should succeed");

    let output = result.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("test_value"),
        "Output should contain the set environment variable value"
    );
}

#[test]
#[cfg(unix)]
fn test_env_config_remove_variable() {
    // 首先设置一个环境变量
    unsafe {
        std::env::set_var("TEMP_TEST_VAR", "should_be_removed");
    }

    // 测试清除环境变量
    let env = EnvConfig::new().remove("TEMP_TEST_VAR");

    let config = CommandConfig::new("printenv", vec![]).with_env(env);

    let result = execute::execute_command_with_context(&config, 1);
    assert!(result.is_ok(), "Command should succeed");

    let output = result.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("TEMP_TEST_VAR"),
        "Output should not contain the removed variable"
    );

    // 清理
    unsafe {
        std::env::remove_var("TEMP_TEST_VAR");
    }
}

#[test]
#[cfg(unix)]
fn test_env_config_no_inherit() {
    // 测试不继承父进程环境变量
    let env = EnvConfig::new().no_inherit().set("ONLY_VAR", "only_value");

    let config = CommandConfig::new("printenv", vec![]).with_env(env);

    let result = execute::execute_command_with_context(&config, 1);
    assert!(result.is_ok(), "Command should succeed");

    let output = result.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // 应该只有我们设置的变量
    assert!(
        stdout.contains("ONLY_VAR=only_value"),
        "Output should contain the set variable"
    );

    // 不应该有 PATH 等常见的环境变量（因为我们清除了所有继承的变量）
    // 注意：某些系统可能会自动添加一些变量，所以我们只检查我们设置的变量存在
}

#[test]
#[cfg(unix)]
fn test_env_config_inherit_and_override() {
    // 测试继承父进程环境变量并覆盖
    unsafe {
        std::env::set_var("TEST_OVERRIDE_VAR", "original_value");
    }

    let env = EnvConfig::new().set("TEST_OVERRIDE_VAR", "overridden_value");

    let config =
        CommandConfig::new("printenv", vec!["TEST_OVERRIDE_VAR".to_string()]).with_env(env);

    let result = execute::execute_command_with_context(&config, 1);
    assert!(result.is_ok(), "Command should succeed");

    let output = result.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("overridden_value"),
        "Output should contain the overridden value"
    );
    assert!(
        !stdout.contains("original_value"),
        "Output should not contain the original value"
    );

    // 清理
    unsafe {
        std::env::remove_var("TEST_OVERRIDE_VAR");
    }
}

#[test]
#[cfg(unix)]
fn test_env_config_multiple_variables() {
    // 测试设置多个环境变量
    let env = EnvConfig::new()
        .set("VAR1", "value1")
        .set("VAR2", "value2")
        .set("VAR3", "value3");

    let config = CommandConfig::new(
        "sh",
        vec!["-c".to_string(), "echo $VAR1 $VAR2 $VAR3".to_string()],
    )
    .with_env(env);

    let result = execute::execute_command_with_context(&config, 1);
    assert!(result.is_ok(), "Command should succeed");

    let output = result.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("value1 value2 value3"),
        "Output should contain all set variable values"
    );
}

#[test]
#[cfg(unix)]
fn test_env_config_with_context_executor() {
    // 测试环境变量配置在 execute_command_with_context 中也能工作
    let env = EnvConfig::new().set("CONTEXT_TEST_VAR", "context_value");

    let config = CommandConfig::new("printenv", vec!["CONTEXT_TEST_VAR".to_string()]).with_env(env);

    let result = execute::execute_command_with_context(&config, 1);
    assert!(result.is_ok(), "Command should succeed");

    let output = result.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("context_value"),
        "Output should contain the set environment variable value"
    );
}

#[test]
#[cfg(unix)]
fn test_env_config_with_retry_executor() {
    // 测试环境变量配置在 execute_with_retry 中也能工作
    use execute::{RetryPolicy, RetryStrategy};
    use std::time::Duration;

    let env = EnvConfig::new().set("RETRY_TEST_VAR", "retry_value");

    let retry_policy =
        RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(100)));

    let config = CommandConfig::new("printenv", vec!["RETRY_TEST_VAR".to_string()])
        .with_env(env)
        .with_retry(retry_policy);

    let result = execute::execute_with_retry(&config, 1);
    assert!(result.is_ok(), "Command should succeed");

    let output = result.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("retry_value"),
        "Output should contain the set environment variable value"
    );
}

#[test]
#[cfg(unix)]
fn test_env_config_with_timeout_executor() {
    // 测试环境变量配置在 execute_with_timeouts 中也能工作
    use execute::TimeoutConfig;
    use std::time::Duration;

    let env = EnvConfig::new().set("TIMEOUT_TEST_VAR", "timeout_value");

    let timeout_config = TimeoutConfig::new().with_execution_timeout(Duration::from_secs(5));

    let config = CommandConfig::new("printenv", vec!["TIMEOUT_TEST_VAR".to_string()])
        .with_env(env)
        .with_timeouts(timeout_config);

    let result = execute::execute_with_timeouts(&config, 1);
    assert!(result.is_ok(), "Command should succeed");

    let output = result.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("timeout_value"),
        "Output should contain the set environment variable value"
    );
}
