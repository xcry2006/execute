use execute::{ConfigError, PoolConfigBuilder};
use std::time::Duration;

#[test]
fn test_valid_config() {
    let config = PoolConfigBuilder::new()
        .thread_count(4)
        .queue_capacity(100)
        .default_timeout(Duration::from_secs(30))
        .poll_interval(Duration::from_millis(100))
        .build();

    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.thread_count, 4);
    assert_eq!(config.queue_capacity, Some(100));
    assert_eq!(config.default_timeout, Some(Duration::from_secs(30)));
    assert_eq!(config.poll_interval, Duration::from_millis(100));
}

#[test]
fn test_default_values() {
    let config = PoolConfigBuilder::new().build();

    assert!(config.is_ok());
    let config = config.unwrap();
    assert!(config.thread_count >= 1);
    assert_eq!(config.queue_capacity, None);
    assert_eq!(config.default_timeout, None);
    assert_eq!(config.poll_interval, Duration::from_millis(100));
}

#[test]
fn test_invalid_thread_count_zero() {
    let result = PoolConfigBuilder::new().thread_count(0).build();

    assert!(result.is_err());
    match result.unwrap_err() {
        ConfigError::InvalidThreadCount(count) => {
            assert_eq!(count, 0);
        }
        _ => panic!("Expected InvalidThreadCount error"),
    }
}

#[test]
fn test_invalid_queue_capacity_zero() {
    let result = PoolConfigBuilder::new().queue_capacity(0).build();

    assert!(result.is_err());
    match result.unwrap_err() {
        ConfigError::InvalidQueueCapacity(capacity) => {
            assert_eq!(capacity, 0);
        }
        _ => panic!("Expected InvalidQueueCapacity error"),
    }
}

#[test]
fn test_invalid_timeout_zero() {
    let result = PoolConfigBuilder::new()
        .default_timeout(Duration::ZERO)
        .build();

    assert!(result.is_err());
    match result.unwrap_err() {
        ConfigError::InvalidTimeout(timeout) => {
            assert_eq!(timeout, Duration::ZERO);
        }
        _ => panic!("Expected InvalidTimeout error"),
    }
}

#[test]
fn test_invalid_poll_interval_zero() {
    let result = PoolConfigBuilder::new()
        .poll_interval(Duration::ZERO)
        .build();

    assert!(result.is_err());
    match result.unwrap_err() {
        ConfigError::InvalidPollInterval(interval) => {
            assert_eq!(interval, Duration::ZERO);
        }
        _ => panic!("Expected InvalidPollInterval error"),
    }
}

#[test]
fn test_thread_count_exceeds_limit() {
    // 使用一个非常大的线程数，应该超过系统限制
    let result = PoolConfigBuilder::new().thread_count(1_000_000).build();

    assert!(result.is_err());
    match result.unwrap_err() {
        ConfigError::ThreadCountExceedsLimit(requested, limit) => {
            assert_eq!(requested, 1_000_000);
            assert!(limit > 0);
            assert!(limit < 1_000_000);
        }
        _ => panic!("Expected ThreadCountExceedsLimit error"),
    }
}

#[test]
fn test_error_messages() {
    // 测试错误消息的清晰度
    let err = PoolConfigBuilder::new()
        .thread_count(0)
        .build()
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Invalid thread count"));
    assert!(msg.contains("must be >= 1"));

    let err = PoolConfigBuilder::new()
        .queue_capacity(0)
        .build()
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Invalid queue capacity"));
    assert!(msg.contains("must be >= 1"));

    let err = PoolConfigBuilder::new()
        .default_timeout(Duration::ZERO)
        .build()
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Invalid timeout"));
    assert!(msg.contains("must be positive"));

    let err = PoolConfigBuilder::new()
        .poll_interval(Duration::ZERO)
        .build()
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Invalid poll interval"));
    assert!(msg.contains("must be positive"));
}
