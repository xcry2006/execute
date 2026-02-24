use execute::{CommandConfig, TimeoutConfig};
use std::time::Duration;

#[test]
fn test_command_config_with_timeouts() {
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(5))
        .with_execution_timeout(Duration::from_secs(30));

    let cmd = CommandConfig::new("sleep", vec!["10".to_string()]).with_timeouts(timeout_config);

    let config = cmd.timeout_config().unwrap();
    assert_eq!(config.spawn_timeout(), Some(Duration::from_secs(5)));
    assert_eq!(config.execution_timeout(), Some(Duration::from_secs(30)));
}

#[test]
fn test_command_config_without_timeouts() {
    let cmd = CommandConfig::new("echo", vec!["hello".to_string()]);
    assert!(cmd.timeout_config().is_none());
}

#[test]
fn test_command_config_with_only_spawn_timeout() {
    let timeout_config = TimeoutConfig::new().with_spawn_timeout(Duration::from_secs(3));

    let cmd = CommandConfig::new("ls", vec!["-la".to_string()]).with_timeouts(timeout_config);

    let config = cmd.timeout_config().unwrap();
    assert_eq!(config.spawn_timeout(), Some(Duration::from_secs(3)));
    assert!(config.execution_timeout().is_none());
}

#[test]
fn test_command_config_with_only_execution_timeout() {
    let timeout_config = TimeoutConfig::new().with_execution_timeout(Duration::from_secs(60));

    let cmd = CommandConfig::new("curl", vec!["https://example.com".to_string()])
        .with_timeouts(timeout_config);

    let config = cmd.timeout_config().unwrap();
    assert!(config.spawn_timeout().is_none());
    assert_eq!(config.execution_timeout(), Some(Duration::from_secs(60)));
}

#[test]
fn test_command_config_builder_pattern_with_timeouts() {
    let cmd = CommandConfig::new("test", vec!["arg".to_string()])
        .with_working_dir("/tmp")
        .with_timeout(Duration::from_secs(10))
        .with_timeouts(
            TimeoutConfig::new()
                .with_spawn_timeout(Duration::from_secs(2))
                .with_execution_timeout(Duration::from_secs(20)),
        );

    assert_eq!(cmd.working_dir(), Some("/tmp"));
    assert_eq!(cmd.timeout(), Some(Duration::from_secs(10)));

    let timeout_config = cmd.timeout_config().unwrap();
    assert_eq!(timeout_config.spawn_timeout(), Some(Duration::from_secs(2)));
    assert_eq!(
        timeout_config.execution_timeout(),
        Some(Duration::from_secs(20))
    );
}
