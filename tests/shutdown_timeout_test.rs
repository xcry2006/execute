use execute::{CommandConfig, CommandPool};
use std::time::Duration;

#[test]
fn test_shutdown_timeout_with_long_running_task() {
    let pool = CommandPool::new();
    pool.start_executor();

    // 添加一个会运行 5 秒的任务
    let _ = pool.push_task(CommandConfig::new("sleep", vec!["5".to_string()]));

    // 使用 1 秒超时关闭，应该超时
    let result = pool.shutdown_with_timeout(Duration::from_secs(1));

    // 因为任务运行 5 秒，而超时只有 1 秒，所以应该超时
    assert!(result.is_err());
}

#[test]
fn test_shutdown_timeout_with_quick_tasks() {
    let pool = CommandPool::new();
    pool.start_executor();

    // 添加几个快速任务
    for i in 0..3 {
        let _ = pool.push_task(CommandConfig::new("echo", vec![i.to_string()]));
    }

    // 使用足够长的超时，应该成功
    let result = pool.shutdown_with_timeout(Duration::from_secs(5));
    assert!(result.is_ok());
}

#[test]
fn test_shutdown_timeout_immediate() {
    let pool = CommandPool::new();
    pool.start_executor();

    // 立即关闭，没有任何任务
    let result = pool.shutdown_with_timeout(Duration::from_millis(100));
    assert!(result.is_ok());
}
