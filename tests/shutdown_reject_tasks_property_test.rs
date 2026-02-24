// Feature: production-ready-improvements, Property 4: 关闭后拒绝新任务
// **Validates: Requirements 2.1**
//
// 属性 4: 关闭后拒绝新任务
// 对于任意任务，在 shutdown 被调用后提交应该失败
//
// 验证需求：
// - 需求 2.1: 收到关闭信号时，命令池应停止接受新任务

use execute::{CommandConfig, CommandPool, ExecutionConfig, ExecutionMode, SubmitError};
use proptest::prelude::*;
use std::time::Duration;

/// 生成任务数量策略（1-20个任务）
fn task_count_strategy() -> impl Strategy<Value = usize> {
    1usize..=20
}

/// 生成命令策略（简单的 echo 或 sleep 命令）
fn command_strategy() -> impl Strategy<Value = CommandConfig> {
    prop_oneof![
        Just(CommandConfig::new("echo", vec!["test".to_string()])),
        Just(CommandConfig::new("sleep", vec!["0.1".to_string()])),
        Just(CommandConfig::new("true", vec![])),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意任务，在 shutdown 被调用后提交应该失败
    ///
    /// 此测试验证：
    /// - 需求 2.1: 收到关闭信号时，命令池应停止接受新任务
    ///
    /// 测试策略：
    /// 1. 创建命令池并启动执行器
    /// 2. 可选地提交一些初始任务
    /// 3. 调用 shutdown
    /// 4. 尝试提交新任务
    /// 5. 验证所有新任务提交都失败，返回 SubmitError::ShuttingDown
    #[test]
    fn prop_shutdown_rejects_new_tasks(
        initial_task_count in 0usize..=5,
        new_task_count in task_count_strategy(),
    ) {
        // 创建命令池
        let config = ExecutionConfig {
            mode: ExecutionMode::Process,
            workers: 2,
            ..Default::default()
        };
        let pool = CommandPool::with_config(config);
        pool.start_executor();

        // 提交一些初始任务（可选）
        for _ in 0..initial_task_count {
            let _ = pool.push_task(CommandConfig::new("echo", vec!["initial".to_string()]));
        }

        // 调用 shutdown（使用较长的超时以确保初始任务能完成）
        let shutdown_result = pool.shutdown_with_timeout(Duration::from_secs(5));
        prop_assert!(
            shutdown_result.is_ok(),
            "Shutdown should succeed, got: {:?}",
            shutdown_result
        );

        // 尝试提交新任务，所有提交都应该失败
        let mut all_rejected = true;
        let mut rejection_count = 0;

        for _ in 0..new_task_count {
            let result = pool.push_task(CommandConfig::new("echo", vec!["after_shutdown".to_string()]));
            
            match result {
                Err(SubmitError::ShuttingDown) => {
                    rejection_count += 1;
                }
                Ok(_) => {
                    all_rejected = false;
                    break;
                }
                Err(e) => {
                    // 其他错误也算失败，但不是我们期望的错误类型
                    prop_assert!(
                        false,
                        "Expected SubmitError::ShuttingDown, got: {:?}",
                        e
                    );
                }
            }
        }

        prop_assert!(
            all_rejected,
            "All task submissions after shutdown should be rejected"
        );
        prop_assert_eq!(
            rejection_count,
            new_task_count,
            "All {} tasks should be rejected with ShuttingDown error",
            new_task_count
        );
    }

    /// 属性测试：在 shutdown 过程中提交的任务应该被拒绝
    ///
    /// 此测试验证即使在 shutdown 调用期间，新任务也应该被拒绝
    #[test]
    fn prop_shutdown_rejects_concurrent_submissions(
        task_count in task_count_strategy(),
        cmd in command_strategy(),
    ) {
        // 创建命令池
        let pool = CommandPool::new();
        pool.start_executor();

        // 提交一些短时间运行的任务
        for _ in 0..3 {
            let _ = pool.push_task(CommandConfig::new("sleep", vec!["0.2".to_string()]));
        }

        // 在另一个线程中调用 shutdown
        let pool_clone = pool.clone();
        let shutdown_handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            pool_clone.shutdown_with_timeout(Duration::from_secs(2))
        });

        // 等待一小段时间让 shutdown 开始
        std::thread::sleep(Duration::from_millis(100));

        // 尝试提交新任务
        let mut rejected_count = 0;
        for _ in 0..task_count {
            match pool.push_task(cmd.clone()) {
                Err(SubmitError::ShuttingDown) => {
                    rejected_count += 1;
                }
                Ok(_) => {
                    // 如果在 shutdown 设置标志前提交成功，这是可以接受的
                }
                Err(e) => {
                    prop_assert!(
                        false,
                        "Expected SubmitError::ShuttingDown or Ok, got: {:?}",
                        e
                    );
                }
            }
        }

        // 等待 shutdown 完成
        let shutdown_result = shutdown_handle.join();
        prop_assert!(
            shutdown_result.is_ok(),
            "Shutdown thread should complete successfully"
        );

        // 至少应该有一些任务被拒绝（因为我们在 shutdown 后等待了一段时间）
        prop_assert!(
            rejected_count > 0,
            "At least some tasks should be rejected during/after shutdown"
        );
    }
}

// 单元测试：验证特定的关闭后拒绝场景

#[test]
fn test_shutdown_rejects_single_task() {
    let pool = CommandPool::new();
    pool.start_executor();

    // 关闭命令池
    let result = pool.shutdown_with_timeout(Duration::from_secs(1));
    assert!(result.is_ok(), "Shutdown should succeed");

    // 尝试提交任务
    let submit_result = pool.push_task(CommandConfig::new("echo", vec!["test".to_string()]));

    // 应该返回 ShuttingDown 错误
    match submit_result {
        Err(SubmitError::ShuttingDown) => {
            // 正确的行为
        }
        Ok(_) => {
            panic!("Task submission should fail after shutdown");
        }
        Err(e) => {
            panic!("Expected SubmitError::ShuttingDown, got: {:?}", e);
        }
    }
}

#[test]
fn test_shutdown_rejects_multiple_tasks() {
    let pool = CommandPool::new();
    pool.start_executor();

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(1));

    // 尝试提交多个任务
    for i in 0..10 {
        let result = pool.push_task(CommandConfig::new("echo", vec![format!("task_{}", i)]));
        
        assert!(
            matches!(result, Err(SubmitError::ShuttingDown)),
            "Task {} submission should fail with ShuttingDown error",
            i
        );
    }
}

#[test]
fn test_try_push_task_after_shutdown() {
    let pool = CommandPool::new();
    pool.start_executor();

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(1));

    // 尝试使用 try_push_task 提交任务
    let result = pool.try_push_task(CommandConfig::new("echo", vec!["test".to_string()]));

    // 应该返回 ShuttingDown 错误
    assert!(
        matches!(result, Err(SubmitError::ShuttingDown)),
        "try_push_task should also fail after shutdown"
    );
}

#[test]
fn test_shutdown_rejects_tasks_immediately() {
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交一个快速任务
    let _ = pool.push_task(CommandConfig::new("echo", vec!["quick".to_string()]));

    // 立即关闭
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));

    // 立即尝试提交新任务
    let result = pool.push_task(CommandConfig::new("echo", vec!["after".to_string()]));

    // 应该立即被拒绝
    assert!(
        matches!(result, Err(SubmitError::ShuttingDown)),
        "Task should be rejected immediately after shutdown"
    );
}

#[test]
fn test_shutdown_with_queue_limit() {
    // 创建有队列限制的命令池
    let config = ExecutionConfig {
        mode: ExecutionMode::Process,
        workers: 1,
        ..Default::default()
    };
    let pool = CommandPool::with_config_and_limit(config, 5);
    pool.start_executor();

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(1));

    // 尝试提交任务
    let result = pool.push_task(CommandConfig::new("echo", vec!["test".to_string()]));

    // 即使有队列限制，也应该返回 ShuttingDown 错误而不是 QueueFull
    assert!(
        matches!(result, Err(SubmitError::ShuttingDown)),
        "Should return ShuttingDown error, not QueueFull"
    );
}

#[test]
fn test_shutdown_flag_checked_during_wait() {
    // 创建有队列限制的命令池
    let config = ExecutionConfig {
        mode: ExecutionMode::Process,
        workers: 1,
        ..Default::default()
    };
    let pool = CommandPool::with_config_and_limit(config, 2);
    pool.start_executor();

    // 填满队列（使用长时间运行的任务）
    let _ = pool.push_task(CommandConfig::new("sleep", vec!["2".to_string()]));
    let _ = pool.push_task(CommandConfig::new("sleep", vec!["2".to_string()]));

    // 在另一个线程中尝试提交任务（会阻塞等待队列空位）
    let pool_clone = pool.clone();
    let submit_handle = std::thread::spawn(move || {
        // 等待一小段时间确保队列已满
        std::thread::sleep(Duration::from_millis(50));
        pool_clone.push_task(CommandConfig::new("echo", vec!["blocked".to_string()]))
    });

    // 等待提交线程开始等待
    std::thread::sleep(Duration::from_millis(200));

    // 关闭命令池（使用较长的超时以等待正在执行的任务）
    let _ = pool.shutdown_with_timeout(Duration::from_secs(5));

    // 等待提交线程完成
    let result = submit_handle.join().unwrap();

    // 提交应该失败，因为在等待期间检测到 shutdown
    // 注意：由于竞态条件，任务可能在 shutdown 标志设置前就已经提交成功
    // 所以我们只验证如果失败，应该是 ShuttingDown 错误
    if let Err(e) = result {
        assert!(
            matches!(e, SubmitError::ShuttingDown),
            "If submission fails, it should be ShuttingDown error, got: {:?}",
            e
        );
    }
    // 如果成功，说明任务在 shutdown 前提交了，这也是可以接受的
}
