use execute::{TimeoutConfig, TimeoutError};
use std::time::Duration;

#[test]
fn test_timeout_config_creation() {
    let config = TimeoutConfig::new();
    assert!(config.spawn_timeout().is_none());
    assert!(config.execution_timeout().is_none());
}

#[test]
fn test_timeout_config_with_spawn_timeout() {
    let config = TimeoutConfig::new().with_spawn_timeout(Duration::from_secs(5));
    assert_eq!(config.spawn_timeout(), Some(Duration::from_secs(5)));
    assert!(config.execution_timeout().is_none());
}

#[test]
fn test_timeout_config_with_execution_timeout() {
    let config = TimeoutConfig::new().with_execution_timeout(Duration::from_secs(30));
    assert!(config.spawn_timeout().is_none());
    assert_eq!(config.execution_timeout(), Some(Duration::from_secs(30)));
}

#[test]
fn test_timeout_config_with_both_timeouts() {
    let config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(5))
        .with_execution_timeout(Duration::from_secs(30));
    assert_eq!(config.spawn_timeout(), Some(Duration::from_secs(5)));
    assert_eq!(config.execution_timeout(), Some(Duration::from_secs(30)));
}

#[test]
fn test_timeout_config_builder_pattern() {
    // Test that builder pattern works correctly
    let config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(3))
        .with_execution_timeout(Duration::from_secs(60));

    assert_eq!(config.spawn_timeout().unwrap().as_secs(), 3);
    assert_eq!(config.execution_timeout().unwrap().as_secs(), 60);
}

#[test]
fn test_timeout_error_spawn_timeout() {
    let error = TimeoutError::SpawnTimeout(Duration::from_secs(5));
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Spawn timeout"));
    assert!(error_msg.contains("5s"));
}

#[test]
fn test_timeout_error_execution_timeout() {
    let error = TimeoutError::ExecutionTimeout(Duration::from_secs(30));
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Execution timeout"));
    assert!(error_msg.contains("30s"));
}

#[test]
fn test_timeout_error_equality() {
    let error1 = TimeoutError::SpawnTimeout(Duration::from_secs(5));
    let error2 = TimeoutError::SpawnTimeout(Duration::from_secs(5));
    let error3 = TimeoutError::ExecutionTimeout(Duration::from_secs(5));

    assert_eq!(error1, error2);
    assert_ne!(error1, error3);
}

#[test]
fn test_timeout_error_clone() {
    let error = TimeoutError::SpawnTimeout(Duration::from_secs(5));
    let cloned = error.clone();
    assert_eq!(error, cloned);
}
