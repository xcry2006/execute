use execute::CommandConfig;
use std::time::Duration;

#[test]
fn command_config_new_sets_defaults() {
    let cfg = CommandConfig::new("echo", vec!["hello".to_string()]);

    assert_eq!(cfg.program(), "echo");
    assert_eq!(cfg.args(), &["hello".to_string()]);
    assert!(cfg.working_dir().is_none());
    assert_eq!(cfg.timeout(), Some(Duration::from_secs(10)));
}

#[test]
fn command_config_with_working_dir_sets_dir() {
    let cfg = CommandConfig::new("echo", vec!["hi".to_string()])
        .with_working_dir("/tmp");

    assert_eq!(cfg.working_dir(), Some("/tmp"));
}

#[test]
fn command_config_with_timeout_sets_timeout() {
    let cfg = CommandConfig::new("sleep", vec!["1".to_string()])
        .with_timeout(Duration::from_millis(250));

    assert_eq!(cfg.timeout(), Some(Duration::from_millis(250)));
}

