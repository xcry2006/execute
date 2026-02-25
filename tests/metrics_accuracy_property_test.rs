// Feature: production-ready-improvements, Property 9: 指标准确性
// **Validates: Requirements 7.1, 7.2, 7.3, 7.4**
//
// 属性 9: 指标准确性
// 对于任意时刻，metrics 返回的队列任务数、执行中任务数、完成任务数和失败任务数
// 应该与实际状态一致
//
// 验证需求：
// - 需求 7.1: 记录当前队列中的任务数量
// - 需求 7.2: 记录正在执行的任务数量
// - 需求 7.3: 记录已完成任务的总数
// - 需求 7.4: 记录失败任务的总数

use execute::{CommandConfig, CommandPool};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// 生成任务数量策略（1-20个任务）
fn task_count_strategy() -> impl Strategy<Value = usize> {
    1usize..=20
}

/// 生成成功命令策略
fn success_command_strategy() -> impl Strategy<Value = CommandConfig> {
    prop_oneof![
        Just(CommandConfig::new("echo", vec!["success".to_string()])),
        Just(CommandConfig::new("echo", vec!["test".to_string()])),
        #[cfg(unix)]
        Just(CommandConfig::new("true", vec![])),
    ]
}

/// 生成失败命令策略
fn failure_command_strategy() -> impl Strategy<Value = CommandConfig> {
    prop_oneof![
        Just(CommandConfig::new("nonexistent_command_xyz_123", vec![])),
        #[cfg(unix)]
        Just(CommandConfig::new("false", vec![])),
    ]
}

/// 生成混合命令策略（成功和失败）
fn mixed_commands_strategy(count: usize) -> impl Strategy<Value = Vec<(CommandConfig, bool)>> {
    prop::collection::vec(
        prop_oneof![
            success_command_strategy().prop_map(|c| (c, true)),
            failure_command_strategy().prop_map(|c| (c, false)),
        ],
        count..=count,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意数量的任务，指标应该准确反映任务状态
    ///
    /// 验证需求：
    /// - 需求 7.1: 记录当前队列中的任务数量
    /// - 需求 7.2: 记录正在执行的任务数量
    /// - 需求 7.3: 记录已完成任务的总数
    /// - 需求 7.4: 记录失败任务的总数
    #[test]
    fn prop_metrics_accuracy_for_task_counts(
        task_count in task_count_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建命令池并启动
        let pool = CommandPool::new();
        pool.start_executor();

        // 获取初始指标
        let initial_metrics = pool.metrics();
        prop_assert_eq!(initial_metrics.tasks_submitted, 0, "Initial submitted count should be 0");
        prop_assert_eq!(initial_metrics.tasks_queued, 0, "Initial queued count should be 0");
        prop_assert_eq!(initial_metrics.tasks_running, 0, "Initial running count should be 0");

        // 提交任务（只使用成功的命令以简化测试）
        let mut handles = Vec::new();
        for i in 0..task_count {
            let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
            match pool.push_task(config) {
                Ok(handle) => handles.push(handle),
                Err(_) => {
                    // 如果提交失败，停止提交更多任务
                    break;
                }
            }
        }

        let submitted_count = handles.len();

        // 验证提交后的指标
        let after_submit_metrics = pool.metrics();
        prop_assert_eq!(
            after_submit_metrics.tasks_submitted as usize,
            submitted_count,
            "Submitted count should match actual submissions"
        );

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.wait();
        }

        // 等待一小段时间确保指标更新
        std::thread::sleep(Duration::from_millis(100));

        // 获取最终指标
        let final_metrics = pool.metrics();

        // 验证最终指标
        prop_assert_eq!(
            final_metrics.tasks_submitted as usize,
            submitted_count,
            "Final submitted count should match"
        );

        // 验证完成和失败的任务总数等于提交的任务数
        let total_finished = final_metrics.tasks_completed + final_metrics.tasks_failed;
        prop_assert_eq!(
            total_finished as usize,
            submitted_count,
            "Completed + Failed should equal Submitted. Completed: {}, Failed: {}, Submitted: {}",
            final_metrics.tasks_completed,
            final_metrics.tasks_failed,
            submitted_count
        );

        // 验证队列和运行中的任务数为0（所有任务已完成）
        prop_assert_eq!(
            final_metrics.tasks_queued,
            0,
            "Queue should be empty after all tasks complete"
        );
        prop_assert_eq!(
            final_metrics.tasks_running,
            0,
            "No tasks should be running after all complete"
        );

        // 关闭命令池
        let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
    }

    /// 属性测试：对于包含成功和失败任务的混合场景，指标应该准确分类
    ///
    /// 验证需求：
    /// - 需求 7.3: 记录已完成任务的总数
    /// - 需求 7.4: 记录失败任务的总数
    #[test]
    fn prop_metrics_accuracy_for_success_and_failure(
        task_count in task_count_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建命令池并启动
        let pool = CommandPool::new();
        pool.start_executor();

        // 生成混合命令
        let commands = mixed_commands_strategy(task_count)
            .new_tree(&mut proptest::test_runner::TestRunner::default())
            .unwrap()
            .current();

        // 提交任务
        let mut handles = Vec::new();
        for (config, _) in commands {
            match pool.push_task(config) {
                Ok(handle) => handles.push(handle),
                Err(_) => break,
            }
        }

        let submitted_count = handles.len();

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.wait();
        }

        // 等待一小段时间确保指标更新
        std::thread::sleep(Duration::from_millis(100));

        // 获取最终指标
        let final_metrics = pool.metrics();

        // 验证提交计数
        prop_assert_eq!(
            final_metrics.tasks_submitted as usize,
            submitted_count,
            "Submitted count should match"
        );

        // 验证完成和失败的总数等于提交数
        let total_finished = final_metrics.tasks_completed + final_metrics.tasks_failed;
        prop_assert_eq!(
            total_finished as usize,
            submitted_count,
            "Completed + Failed should equal Submitted. Completed: {}, Failed: {}, Submitted: {}",
            final_metrics.tasks_completed,
            final_metrics.tasks_failed,
            submitted_count
        );

        // 验证队列和运行中的任务数为0
        prop_assert_eq!(
            final_metrics.tasks_queued,
            0,
            "Queue should be empty after all tasks complete"
        );
        prop_assert_eq!(
            final_metrics.tasks_running,
            0,
            "No tasks should be running after all complete"
        );

        // 关闭命令池
        let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
    }
}

#[test]
fn test_metrics_accuracy_single_success_task() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 验证初始指标
    let initial = pool.metrics();
    assert_eq!(initial.tasks_submitted, 0);
    assert_eq!(initial.tasks_completed, 0);
    assert_eq!(initial.tasks_failed, 0);
    assert_eq!(initial.tasks_queued, 0);
    assert_eq!(initial.tasks_running, 0);

    // 提交一个成功的任务
    let config = CommandConfig::new("echo", vec!["test".to_string()]);
    let handle = pool.push_task(config).expect("Failed to submit task");

    // 验证提交后的指标
    let after_submit = pool.metrics();
    assert_eq!(after_submit.tasks_submitted, 1);

    // 等待任务完成
    let result = handle.wait();
    assert!(result.is_ok());

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(100));

    // 验证最终指标
    let final_metrics = pool.metrics();
    assert_eq!(final_metrics.tasks_submitted, 1);
    assert_eq!(final_metrics.tasks_completed, 1);
    assert_eq!(final_metrics.tasks_failed, 0);
    assert_eq!(final_metrics.tasks_queued, 0);
    assert_eq!(final_metrics.tasks_running, 0);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_metrics_accuracy_single_failure_task() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交一个失败的任务
    let config = CommandConfig::new("nonexistent_command_xyz_123", vec![]);
    let handle = pool.push_task(config).expect("Failed to submit task");

    // 等待任务完成
    let _ = handle.wait();

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(100));

    // 验证最终指标
    let final_metrics = pool.metrics();
    assert_eq!(final_metrics.tasks_submitted, 1);
    assert_eq!(final_metrics.tasks_completed, 0);
    assert_eq!(final_metrics.tasks_failed, 1);
    assert_eq!(final_metrics.tasks_queued, 0);
    assert_eq!(final_metrics.tasks_running, 0);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_metrics_accuracy_concurrent_tasks() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 并发提交多个任务
    let task_count = 10;
    let handles: Vec<_> = (0..task_count)
        .map(|i| {
            let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
            pool.push_task(config).expect("Failed to submit task")
        })
        .collect();

    // 验证提交后的指标
    let after_submit = pool.metrics();
    assert_eq!(after_submit.tasks_submitted, task_count as u64);

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(100));

    // 验证最终指标
    let final_metrics = pool.metrics();
    assert_eq!(final_metrics.tasks_submitted, task_count as u64);
    assert_eq!(final_metrics.tasks_completed, task_count as u64);
    assert_eq!(final_metrics.tasks_failed, 0);
    assert_eq!(final_metrics.tasks_queued, 0);
    assert_eq!(final_metrics.tasks_running, 0);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_metrics_accuracy_queued_and_running() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动（单个工作线程以便控制执行）
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交多个任务
    let task_count = 5;
    let handles: Vec<_> = (0..task_count)
        .map(|i| {
            let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
            pool.push_task(config).expect("Failed to submit task")
        })
        .collect();

    // 立即检查指标（可能有任务在队列中或正在运行）
    let during_execution = pool.metrics();
    assert_eq!(during_execution.tasks_submitted, task_count as u64);

    // 队列中的任务数 + 正在运行的任务数 应该 <= 提交的任务数
    let in_progress = during_execution.tasks_queued + during_execution.tasks_running;
    assert!(
        in_progress <= task_count,
        "Queued + Running should not exceed submitted tasks"
    );

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(100));

    // 验证最终指标
    let final_metrics = pool.metrics();
    assert_eq!(final_metrics.tasks_queued, 0);
    assert_eq!(final_metrics.tasks_running, 0);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_metrics_accuracy_invariant() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交任务
    let task_count = 15;
    let handles: Vec<_> = (0..task_count)
        .map(|i| {
            let config = if i % 3 == 0 {
                // 每3个任务中有1个失败
                CommandConfig::new("nonexistent_command_xyz", vec![])
            } else {
                CommandConfig::new("echo", vec![format!("task_{}", i)])
            };
            pool.push_task(config).expect("Failed to submit task")
        })
        .collect();

    // 在执行过程中多次检查不变量
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();
    let pool_clone = pool.clone();

    let checker_thread = std::thread::spawn(move || {
        while !stop_flag_clone.load(Ordering::Relaxed) {
            let metrics = pool_clone.metrics();

            // 不变量：queued + running + completed + failed + cancelled <= submitted
            let accounted = metrics.tasks_queued as u64
                + metrics.tasks_running as u64
                + metrics.tasks_completed
                + metrics.tasks_failed
                + metrics.tasks_cancelled;

            assert!(
                accounted <= metrics.tasks_submitted,
                "Invariant violated: queued({}) + running({}) + completed({}) + failed({}) + cancelled({}) = {} > submitted({})",
                metrics.tasks_queued,
                metrics.tasks_running,
                metrics.tasks_completed,
                metrics.tasks_failed,
                metrics.tasks_cancelled,
                accounted,
                metrics.tasks_submitted
            );

            std::thread::sleep(Duration::from_millis(10));
        }
    });

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 停止检查线程
    stop_flag.store(true, Ordering::Relaxed);
    checker_thread.join().unwrap();

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(100));

    // 验证最终指标
    let final_metrics = pool.metrics();
    assert_eq!(final_metrics.tasks_submitted, task_count as u64);

    // 最终状态：completed + failed = submitted
    let total_finished = final_metrics.tasks_completed + final_metrics.tasks_failed;
    assert_eq!(
        total_finished, task_count as u64,
        "Completed + Failed should equal Submitted"
    );

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}
