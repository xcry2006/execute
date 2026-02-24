// Feature: production-ready-improvements, Property 7: CommandPoolSeg 停止行为
// **Validates: Requirements 4.2, 4.3**
//
// 属性 7: CommandPoolSeg 停止行为
// 对于任意队列中的任务，调用 stop 后它们应该继续执行完成，但新任务提交应该失败
//
// 验证需求：
// - 需求 4.2: 当 stop() 被调用时，CommandPoolSeg 应停止接受新任务提交
// - 需求 4.3: 当 stop() 被调用时，CommandPoolSeg 应继续执行队列中已有的任务

use execute::{CommandConfig, CommandPoolSeg, SubmitError};
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
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// 属性测试：对于任意任务，在 stop 被调用后提交应该失败
    ///
    /// 此测试验证：
    /// - 需求 4.2: 当 stop() 被调用时，CommandPoolSeg 应停止接受新任务提交
    ///
    /// 测试策略：
    /// 1. 创建 CommandPoolSeg 并启动执行器
    /// 2. 可选地提交一些初始任务
    /// 3. 调用 stop
    /// 4. 尝试提交新任务
    /// 5. 验证所有新任务提交都失败，返回 SubmitError::Stopped
    #[test]
    fn prop_stop_rejects_new_tasks(
        initial_task_count in 0usize..=5,
        new_task_count in task_count_strategy(),
    ) {
        // 创建 CommandPoolSeg
        let pool = CommandPoolSeg::new();
        pool.start_executor_with_workers(Duration::from_millis(50), 2);

        // 提交一些初始任务（可选）
        for _ in 0..initial_task_count {
            let _ = pool.push_task(CommandConfig::new("echo", vec!["initial".to_string()]));
        }

        // 调用 stop
        pool.stop();

        // 验证 is_stopped 返回 true
        prop_assert!(
            pool.is_stopped(),
            "is_stopped() should return true after calling stop()"
        );

        // 等待一小段时间让 stop 标志生效
        std::thread::sleep(Duration::from_millis(50));

        // 尝试提交新任务，所有提交都应该失败
        let mut all_rejected = true;
        let mut rejection_count = 0;

        for _ in 0..new_task_count {
            let result = pool.push_task(CommandConfig::new("echo", vec!["after_stop".to_string()]));
            
            match result {
                Err(SubmitError::Stopped) => {
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
                        "Expected SubmitError::Stopped, got: {:?}",
                        e
                    );
                }
            }
        }

        prop_assert!(
            all_rejected,
            "All task submissions after stop should be rejected"
        );
        prop_assert_eq!(
            rejection_count,
            new_task_count,
            "All {} tasks should be rejected with Stopped error",
            new_task_count
        );
    }

    /// 属性测试：队列中的任务在 stop 后应该继续执行完成
    ///
    /// 此测试验证：
    /// - 需求 4.3: 当 stop() 被调用时，CommandPoolSeg 应继续执行队列中已有的任务
    ///
    /// 测试策略：
    /// 1. 创建 CommandPoolSeg 并启动执行器
    /// 2. 提交一些任务到队列
    /// 3. 调用 stop
    /// 4. 等待足够的时间让任务完成
    /// 5. 验证所有任务都已执行完成（队列为空）
    #[test]
    fn prop_stop_completes_queued_tasks(
        task_count in task_count_strategy(),
    ) {
        // 创建 CommandPoolSeg
        let pool = CommandPoolSeg::new();
        
        // 提交任务到队列
        for i in 0..task_count {
            let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
            let result = pool.push_task(config);
            prop_assert!(
                result.is_ok(),
                "Task submission should succeed before stop"
            );
        }

        // 启动执行器（在提交任务后启动，确保任务在队列中）
        pool.start_executor_with_workers(Duration::from_millis(50), 2);

        // 立即调用 stop
        pool.stop();

        // 验证 is_stopped 返回 true
        prop_assert!(
            pool.is_stopped(),
            "is_stopped() should return true after calling stop()"
        );

        // 等待足够的时间让所有任务完成
        // 每个任务最多需要 100ms，加上一些缓冲时间
        let wait_time = Duration::from_millis(200 + (task_count as u64 * 50));
        std::thread::sleep(wait_time);

        // 验证队列为空（所有任务都已被处理）
        prop_assert!(
            pool.is_empty(),
            "Queue should be empty after tasks complete"
        );

        // 验证停止后不能提交新任务
        let submit_result = pool.push_task(CommandConfig::new("echo", vec!["after_stop".to_string()]));
        prop_assert!(
            matches!(submit_result, Err(SubmitError::Stopped)),
            "Should not accept new tasks after stop"
        );
    }

    /// 属性测试：在 stop 过程中提交的任务应该被拒绝
    ///
    /// 此测试验证即使在 stop 调用期间，新任务也应该被拒绝
    #[test]
    fn prop_stop_rejects_concurrent_submissions(
        task_count in task_count_strategy(),
        cmd in command_strategy(),
    ) {
        // 创建 CommandPoolSeg
        let pool = CommandPoolSeg::new();
        pool.start_executor_with_workers(Duration::from_millis(50), 2);

        // 提交一些短时间运行的任务
        for _ in 0..3 {
            let _ = pool.push_task(CommandConfig::new("sleep", vec!["0.2".to_string()]));
        }

        // 在另一个线程中调用 stop
        let pool_clone = pool.clone();
        let stop_handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            pool_clone.stop();
        });

        // 等待一小段时间让 stop 开始
        std::thread::sleep(Duration::from_millis(100));

        // 尝试提交新任务
        let mut rejected_count = 0;
        for _ in 0..task_count {
            match pool.push_task(cmd.clone()) {
                Err(SubmitError::Stopped) => {
                    rejected_count += 1;
                }
                Ok(_) => {
                    // 如果在 stop 设置标志前提交成功，这是可以接受的
                }
                Err(e) => {
                    prop_assert!(
                        false,
                        "Expected SubmitError::Stopped or Ok, got: {:?}",
                        e
                    );
                }
            }
        }

        // 等待 stop 完成
        let stop_result = stop_handle.join();
        prop_assert!(
            stop_result.is_ok(),
            "Stop thread should complete successfully"
        );

        // 至少应该有一些任务被拒绝（因为我们在 stop 后等待了一段时间）
        prop_assert!(
            rejected_count > 0,
            "At least some tasks should be rejected during/after stop"
        );
    }
}

// 单元测试：验证特定的停止行为场景

#[test]
fn test_stop_rejects_single_task() {
    let pool = CommandPoolSeg::new();
    pool.start_executor_with_workers(Duration::from_millis(50), 2);

    // 停止命令池
    pool.stop();

    // 验证 is_stopped
    assert!(pool.is_stopped(), "is_stopped() should return true");

    // 尝试提交任务
    let submit_result = pool.push_task(CommandConfig::new("echo", vec!["test".to_string()]));

    // 应该返回 Stopped 错误
    match submit_result {
        Err(SubmitError::Stopped) => {
            // 正确的行为
        }
        Ok(_) => {
            panic!("Task submission should fail after stop");
        }
        Err(e) => {
            panic!("Expected SubmitError::Stopped, got: {:?}", e);
        }
    }
}

#[test]
fn test_stop_rejects_multiple_tasks() {
    let pool = CommandPoolSeg::new();
    pool.start_executor_with_workers(Duration::from_millis(50), 2);

    // 停止命令池
    pool.stop();

    // 尝试提交多个任务
    for i in 0..10 {
        let result = pool.push_task(CommandConfig::new("echo", vec![format!("task_{}", i)]));
        
        assert!(
            matches!(result, Err(SubmitError::Stopped)),
            "Task {} submission should fail with Stopped error",
            i
        );
    }
}

#[test]
fn test_stop_completes_queued_tasks() {
    let pool = CommandPoolSeg::new();

    // 提交任务到队列（在启动执行器前）
    for i in 0..5 {
        let result = pool.push_task(CommandConfig::new("echo", vec![format!("task_{}", i)]));
        assert!(result.is_ok(), "Task submission should succeed");
    }

    // 启动执行器
    pool.start_executor_with_workers(Duration::from_millis(50), 2);

    // 立即停止
    pool.stop();

    // 等待任务完成
    std::thread::sleep(Duration::from_millis(500));

    // 队列应该为空
    assert!(pool.is_empty(), "Queue should be empty after tasks complete");
}

#[test]
fn test_stop_idempotent() {
    let pool = CommandPoolSeg::new();
    pool.start_executor_with_workers(Duration::from_millis(50), 2);

    // 第一次停止
    pool.stop();
    assert!(pool.is_stopped(), "Should be stopped after first stop()");

    // 第二次停止应该也能处理
    pool.stop();
    assert!(pool.is_stopped(), "Should still be stopped after second stop()");

    // 尝试提交任务应该失败
    let result = pool.push_task(CommandConfig::new("echo", vec!["test".to_string()]));
    assert!(
        matches!(result, Err(SubmitError::Stopped)),
        "Should reject tasks after multiple stop() calls"
    );
}

#[test]
fn test_stop_before_executor_start() {
    let pool = CommandPoolSeg::new();

    // 在启动执行器前停止
    pool.stop();
    assert!(pool.is_stopped(), "Should be stopped");

    // 尝试提交任务应该失败
    let result = pool.push_task(CommandConfig::new("echo", vec!["test".to_string()]));
    assert!(
        matches!(result, Err(SubmitError::Stopped)),
        "Should reject tasks even before executor starts"
    );

    // 启动执行器（应该立即退出，因为已经停止）
    pool.start_executor_with_workers(Duration::from_millis(50), 2);

    // 等待一小段时间
    std::thread::sleep(Duration::from_millis(100));

    // 仍然应该拒绝任务
    let result = pool.push_task(CommandConfig::new("echo", vec!["test".to_string()]));
    assert!(
        matches!(result, Err(SubmitError::Stopped)),
        "Should still reject tasks after executor start"
    );
}

#[test]
fn test_stop_with_long_running_tasks() {
    let pool = CommandPoolSeg::new();

    // 提交一些长时间运行的任务
    for _ in 0..3 {
        let _ = pool.push_task(CommandConfig::new("sleep", vec!["0.3".to_string()]));
    }

    // 启动执行器
    pool.start_executor_with_workers(Duration::from_millis(50), 2);

    // 等待任务开始执行
    std::thread::sleep(Duration::from_millis(100));

    // 停止
    pool.stop();

    // 验证不能提交新任务
    let result = pool.push_task(CommandConfig::new("echo", vec!["new".to_string()]));
    assert!(
        matches!(result, Err(SubmitError::Stopped)),
        "Should reject new tasks immediately after stop"
    );

    // 等待任务完成
    std::thread::sleep(Duration::from_millis(500));

    // 队列应该为空
    assert!(pool.is_empty(), "Queue should be empty after tasks complete");
}

#[test]
fn test_stop_with_multiple_workers() {
    let pool = CommandPoolSeg::new();

    // 提交多个任务
    for i in 0..10 {
        let _ = pool.push_task(CommandConfig::new("echo", vec![format!("task_{}", i)]));
    }

    // 启动多个 worker
    pool.start_executor_with_workers(Duration::from_millis(50), 4);

    // 停止
    pool.stop();

    // 等待任务完成
    std::thread::sleep(Duration::from_millis(500));

    // 队列应该为空
    assert!(pool.is_empty(), "Queue should be empty after tasks complete");

    // 不能提交新任务
    let result = pool.push_task(CommandConfig::new("echo", vec!["new".to_string()]));
    assert!(
        matches!(result, Err(SubmitError::Stopped)),
        "Should reject new tasks after stop"
    );
}

#[test]
fn test_is_stopped_before_stop() {
    let pool = CommandPoolSeg::new();
    
    // 在调用 stop 前，is_stopped 应该返回 false
    assert!(!pool.is_stopped(), "Should not be stopped initially");
    
    // 应该能够提交任务
    let result = pool.push_task(CommandConfig::new("echo", vec!["test".to_string()]));
    assert!(result.is_ok(), "Should accept tasks before stop");
}

#[test]
fn test_stop_with_empty_queue() {
    let pool = CommandPoolSeg::new();
    pool.start_executor_with_workers(Duration::from_millis(50), 2);

    // 不提交任何任务，直接停止
    pool.stop();

    // 应该立即停止
    assert!(pool.is_stopped(), "Should be stopped");
    assert!(pool.is_empty(), "Queue should be empty");

    // 不能提交新任务
    let result = pool.push_task(CommandConfig::new("echo", vec!["test".to_string()]));
    assert!(
        matches!(result, Err(SubmitError::Stopped)),
        "Should reject tasks after stop"
    );
}
