/// 测试任务取消时返回正确的 Cancelled 错误
///
/// 验证需求 13.5: WHEN 任务被取消时，THE System SHALL 返回 Cancelled 错误
/// 验证需求 13.6: THE TaskHandle SHALL 提供 is_cancelled() 方法查询取消状态
use execute::{CommandConfig, CommandPool, ExecuteError};
use std::time::Duration;

#[test]
fn test_cancelled_task_returns_cancelled_error() {
    // 创建命令池
    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(10));

    // 提交一个长时间运行的任务
    let handle = pool
        .push_task(CommandConfig::new("sleep", vec!["10".to_string()]))
        .expect("Failed to submit task");

    // 立即取消任务
    handle.cancel().expect("Failed to cancel task");

    // 验证 is_cancelled() 返回 true（需求 13.6）
    assert!(handle.is_cancelled(), "Task should be cancelled");

    // 等待任务结果
    let result = handle.wait();

    // 验证返回 Cancelled 错误（需求 13.5）
    assert!(result.is_err(), "Cancelled task should return error");

    match result {
        Err(ExecuteError::Cancelled(task_id)) => {
            println!("✓ Task {} returned Cancelled error as expected", task_id);
            assert_eq!(task_id, handle.id(), "Task ID should match");
        }
        Err(other) => {
            panic!("Expected ExecuteError::Cancelled, got: {:?}", other);
        }
        Ok(_) => {
            panic!("Expected error, got success");
        }
    }

    // 清理
    pool.shutdown_with_timeout(Duration::from_secs(1))
        .expect("Failed to shutdown pool");
}

#[test]
fn test_is_cancelled_method() {
    // 创建命令池
    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(10));

    // 提交任务
    let handle = pool
        .push_task(CommandConfig::new("echo", vec!["test".to_string()]))
        .expect("Failed to submit task");

    // 验证初始状态：未取消（需求 13.6）
    assert!(
        !handle.is_cancelled(),
        "Task should not be cancelled initially"
    );

    // 取消任务
    handle.cancel().expect("Failed to cancel task");

    // 验证取消后状态：已取消（需求 13.6）
    assert!(
        handle.is_cancelled(),
        "Task should be cancelled after cancel()"
    );

    // 清理
    pool.shutdown_with_timeout(Duration::from_secs(1))
        .expect("Failed to shutdown pool");
}

#[test]
fn test_wait_on_cancelled_task() {
    // 创建命令池
    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(10));

    // 提交任务
    let handle = pool
        .push_task(CommandConfig::new("sleep", vec!["5".to_string()]))
        .expect("Failed to submit task");

    // 取消任务
    handle.cancel().expect("Failed to cancel task");

    // wait() 应该返回 Cancelled 错误（需求 13.5）
    let result = handle.wait();

    assert!(
        result.is_err(),
        "wait() should return error for cancelled task"
    );
    assert!(
        matches!(result, Err(ExecuteError::Cancelled(_))),
        "wait() should return ExecuteError::Cancelled"
    );

    // 清理
    pool.shutdown_with_timeout(Duration::from_secs(1))
        .expect("Failed to shutdown pool");
}
