use execute::{CommandConfig, RetryPolicy, RetryStrategy, execute_with_retry};
use std::time::Duration;

#[test]
#[cfg(unix)]
fn test_retry_succeeds_on_first_attempt() {
    // 命令第一次就成功，不需要重试
    let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_millis(100)));
    let config = CommandConfig::new("echo", vec!["hello".to_string()]).with_retry(policy);

    let result = execute_with_retry(&config, 1);
    assert!(result.is_ok());

    let output = result.unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}

#[test]
#[cfg(unix)]
fn test_retry_with_timeout_failure() {
    // 命令总是超时，应该在达到最大重试次数后返回错误
    let policy = RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(50)));
    let config = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_timeout(Duration::from_millis(100))
        .with_retry(policy);

    let start = std::time::Instant::now();
    let result = execute_with_retry(&config, 2);
    let elapsed = start.elapsed();

    // 应该失败（超时）
    assert!(result.is_err());

    // 应该至少等待了 3 次超时（初始 + 2 次重试）+ 2 次重试延迟
    // 3 * 100ms + 2 * 50ms = 400ms
    assert!(elapsed >= Duration::from_millis(400));
}

#[test]
#[cfg(unix)]
fn test_retry_with_exponential_backoff() {
    // 测试指数退避策略
    let policy = RetryPolicy::new(
        3,
        RetryStrategy::ExponentialBackoff {
            initial: Duration::from_millis(10),
            max: Duration::from_secs(1),
            multiplier: 2.0,
        },
    );
    let config = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_timeout(Duration::from_millis(50))
        .with_retry(policy);

    let start = std::time::Instant::now();
    let result = execute_with_retry(&config, 3);
    let elapsed = start.elapsed();

    // 应该失败（超时）
    assert!(result.is_err());

    // 应该等待了：4 次超时（初始 + 3 次重试）+ 3 次重试延迟（10ms + 20ms + 40ms）
    // 4 * 50ms + 70ms = 270ms
    assert!(elapsed >= Duration::from_millis(270));
}

#[test]
#[cfg(unix)]
fn test_no_retry_policy() {
    // 没有配置重试策略，超时后不应该重试
    let config = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_timeout(Duration::from_millis(100));

    let start = std::time::Instant::now();
    let result = execute_with_retry(&config, 4);
    let elapsed = start.elapsed();

    // 应该失败（超时），但只尝试一次
    assert!(result.is_err());
    // 应该只等待一次超时，不重试
    assert!(elapsed >= Duration::from_millis(100));
    assert!(elapsed < Duration::from_millis(200));
}

#[test]
#[cfg(unix)]
fn test_retry_with_spawn_failure() {
    // 测试命令启动失败的情况（不存在的命令）
    let policy = RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(50)));
    let config = CommandConfig::new("nonexistent_command_12345", vec![]).with_retry(policy);

    let start = std::time::Instant::now();
    let result = execute_with_retry(&config, 5);
    let elapsed = start.elapsed();

    // 应该失败（命令不存在）
    assert!(result.is_err());

    // 应该尝试 3 次（初始 + 2 次重试），加上 2 次重试延迟
    // 2 * 50ms = 100ms（命令启动失败很快，主要是延迟时间）
    assert!(elapsed >= Duration::from_millis(100));
}

#[test]
#[cfg(unix)]
fn test_retry_eventually_succeeds() {
    // 测试重试最终成功的情况
    // 使用一个脚本，前几次失败，最后成功
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let script_path = "/tmp/retry_test_script.sh";
    let counter_path = "/tmp/retry_test_counter.txt";

    // 清理旧的计数器文件
    let _ = fs::remove_file(counter_path);

    // 创建测试脚本：前两次超时，第三次快速完成
    let script = format!(
        r#"#!/bin/bash
if [ ! -f {} ]; then
    echo "0" > {}
fi
count=$(cat {})
if [ "$count" -lt "2" ]; then
    echo $((count + 1)) > {}
    sleep 10
fi
echo "success"
exit 0
"#,
        counter_path, counter_path, counter_path, counter_path
    );

    fs::write(script_path, script).unwrap();
    let mut perms = fs::metadata(script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(script_path, perms).unwrap();

    // 配置重试策略：最多重试 3 次，超时 100ms
    let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_millis(50)));
    let config = CommandConfig::new(script_path, vec![])
        .with_timeout(Duration::from_millis(100))
        .with_retry(policy);

    let result = execute_with_retry(&config, 6);

    // 清理
    let _ = fs::remove_file(script_path);
    let _ = fs::remove_file(counter_path);

    // 应该成功（第三次尝试成功）
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "success");
}
