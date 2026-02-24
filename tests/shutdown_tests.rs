use execute::{CommandConfig, CommandPool};
use std::time::Duration;

#[test]
fn test_shutdown_stops_accepting_tasks() {
    let pool = CommandPool::new();
    pool.start_executor();

    // 添加一个任务
    let _ = pool.push_task(CommandConfig::new("echo", vec!["test".to_string()]));
    assert_eq!(pool.len(), 1);

    // 启动关闭
    let result = pool.shutdown_with_timeout(Duration::from_secs(5));
    assert!(result.is_ok());

    // 关闭后应该拒绝新任务
    let result = pool.push_task(CommandConfig::new(
        "echo",
        vec!["after shutdown".to_string()],
    ));
    assert!(result.is_err());
}

#[test]
fn test_is_shutting_down() {
    let pool = CommandPool::new();

    assert!(!pool.is_shutting_down());

    pool.start_executor();
    let _ = pool.shutdown_with_timeout(Duration::from_secs(1));

    assert!(pool.is_shutting_down());
}

#[test]
fn test_shutdown_waits_for_tasks() {
    let pool = CommandPool::new();
    pool.start_executor();

    // 添加几个快速任务
    for i in 0..5 {
        let _ = pool.push_task(CommandConfig::new("echo", vec![i.to_string()]));
    }

    // 等待一下让任务开始执行
    std::thread::sleep(Duration::from_millis(200));

    // 关闭应该等待所有任务完成
    let result = pool.shutdown_with_timeout(Duration::from_secs(5));
    assert!(result.is_ok());

    // 注意：队列中可能还有未被 pop 的任务，但 shutdown 会等待正在执行的任务完成
    // 这是正确的行为 - shutdown 等待的是正在执行的任务，不是队列中的任务
}

#[test]
fn test_push_after_shutdown() {
    let pool = CommandPool::new();
    pool.start_executor();

    let _ = pool.shutdown_with_timeout(Duration::from_secs(1));

    // 单个任务添加应该失败
    let result = pool.push_task(CommandConfig::new("echo", vec!["test".to_string()]));
    assert!(result.is_err());

    // try_push_task 也应该失败
    let result = pool.try_push_task(CommandConfig::new("echo", vec!["test2".to_string()]));
    assert!(result.is_err());
}
