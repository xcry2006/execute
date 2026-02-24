// Feature: production-ready-improvements, Property 10: 成功率计算
// **Validates: Requirements 7.6**
//
// 属性 10: 成功率计算
// 对于任意任务集合，成功率应该等于成功任务数除以总任务数
//
// 验证需求：
// - 需求 7.6: 记录任务成功率

use execute::{CommandConfig, CommandPool};
use proptest::prelude::*;
use std::time::Duration;

/// 生成任务数量策略（1-30个任务）
fn task_count_strategy() -> impl Strategy<Value = usize> {
    1usize..=30
}

/// 生成成功任务比例策略（0.0 到 1.0）
fn success_ratio_strategy() -> impl Strategy<Value = f64> {
    0.0..=1.0
}

/// 生成成功命令策略
#[allow(dead_code)]
fn success_command_strategy() -> impl Strategy<Value = CommandConfig> {
    prop_oneof![
        Just(CommandConfig::new("echo", vec!["success".to_string()])),
        Just(CommandConfig::new("echo", vec!["test".to_string()])),
        #[cfg(unix)]
        Just(CommandConfig::new("true", vec![])),
    ]
}

/// 生成失败命令策略
#[allow(dead_code)]
fn failure_command_strategy() -> impl Strategy<Value = CommandConfig> {
    prop_oneof![
        Just(CommandConfig::new("nonexistent_command_xyz_123", vec![])),
        #[cfg(unix)]
        Just(CommandConfig::new("false", vec![])),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意任务集合，成功率应该等于成功任务数除以总任务数
    ///
    /// 验证需求：
    /// - 需求 7.6: 记录任务成功率
    ///
    /// 测试策略：
    /// 1. 生成随机数量的任务
    /// 2. 根据成功比例生成成功和失败的命令
    /// 3. 提交所有任务并等待完成
    /// 4. 验证 success_rate = tasks_completed / tasks_submitted
    #[test]
    fn prop_success_rate_calculation(
        task_count in task_count_strategy(),
        success_ratio in success_ratio_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建命令池并启动
        let pool = CommandPool::new();
        pool.start_executor();

        // 计算成功和失败任务的数量
        let success_count = (task_count as f64 * success_ratio).round() as usize;
        let failure_count = task_count - success_count;

        // 提交任务
        let mut handles = Vec::new();
        
        // 提交成功任务
        for i in 0..success_count {
            let config = CommandConfig::new("echo", vec![format!("success_{}", i)]);
            match pool.push_task(config) {
                Ok(handle) => handles.push(handle),
                Err(_) => break,
            }
        }
        
        // 提交失败任务
        for i in 0..failure_count {
            let config = CommandConfig::new("nonexistent_command_xyz", vec![format!("{}", i)]);
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
            "Submitted count should match actual submissions"
        );

        // 验证成功率计算
        let expected_success_rate = if submitted_count > 0 {
            (final_metrics.tasks_completed as f64) / (submitted_count as f64)
        } else {
            0.0
        };

        // 允许浮点数误差（epsilon = 0.0001）
        let epsilon = 0.0001;
        prop_assert!(
            (final_metrics.success_rate - expected_success_rate).abs() < epsilon,
            "Success rate mismatch: expected {:.4}, got {:.4}. Completed: {}, Submitted: {}",
            expected_success_rate,
            final_metrics.success_rate,
            final_metrics.tasks_completed,
            submitted_count
        );

        // 验证成功率在有效范围内 [0.0, 1.0]
        prop_assert!(
            final_metrics.success_rate >= 0.0 && final_metrics.success_rate <= 1.0,
            "Success rate should be between 0.0 and 1.0, got {}",
            final_metrics.success_rate
        );

        // 关闭命令池
        let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
    }

    /// 属性测试：全部成功的情况，成功率应该为 1.0
    ///
    /// 验证需求：
    /// - 需求 7.6: 记录任务成功率
    #[test]
    fn prop_success_rate_all_success(
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

        // 提交全部成功的任务
        let mut handles = Vec::new();
        for i in 0..task_count {
            let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
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

        // 等待指标更新
        std::thread::sleep(Duration::from_millis(100));

        // 获取最终指标
        let final_metrics = pool.metrics();

        // 验证成功率为 1.0
        let epsilon = 0.0001;
        prop_assert!(
            (final_metrics.success_rate - 1.0).abs() < epsilon,
            "Success rate should be 1.0 for all successful tasks, got {}. Completed: {}, Submitted: {}",
            final_metrics.success_rate,
            final_metrics.tasks_completed,
            submitted_count
        );

        // 验证所有任务都成功
        prop_assert_eq!(
            final_metrics.tasks_completed as usize,
            submitted_count,
            "All tasks should be completed"
        );
        prop_assert_eq!(
            final_metrics.tasks_failed,
            0,
            "No tasks should fail"
        );

        // 关闭命令池
        let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
    }

    /// 属性测试：全部失败的情况，成功率应该为 0.0
    ///
    /// 验证需求：
    /// - 需求 7.6: 记录任务成功率
    #[test]
    fn prop_success_rate_all_failure(
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

        // 提交全部失败的任务
        let mut handles = Vec::new();
        for i in 0..task_count {
            let config = CommandConfig::new("nonexistent_command_xyz", vec![format!("{}", i)]);
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

        // 等待指标更新
        std::thread::sleep(Duration::from_millis(100));

        // 获取最终指标
        let final_metrics = pool.metrics();

        // 验证成功率为 0.0
        let epsilon = 0.0001;
        prop_assert!(
            final_metrics.success_rate.abs() < epsilon,
            "Success rate should be 0.0 for all failed tasks, got {}. Completed: {}, Failed: {}, Submitted: {}",
            final_metrics.success_rate,
            final_metrics.tasks_completed,
            final_metrics.tasks_failed,
            submitted_count
        );

        // 验证所有任务都失败
        prop_assert_eq!(
            final_metrics.tasks_completed,
            0,
            "No tasks should be completed"
        );
        prop_assert_eq!(
            final_metrics.tasks_failed as usize,
            submitted_count,
            "All tasks should fail"
        );

        // 关闭命令池
        let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
    }
}

#[test]
fn test_success_rate_zero_tasks() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 不提交任何任务
    let metrics = pool.metrics();

    // 验证成功率为 0.0（没有任务时）
    assert_eq!(metrics.success_rate, 0.0, "Success rate should be 0.0 when no tasks submitted");
    assert_eq!(metrics.tasks_submitted, 0);
    assert_eq!(metrics.tasks_completed, 0);
    assert_eq!(metrics.tasks_failed, 0);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_success_rate_single_success() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交一个成功的任务
    let config = CommandConfig::new("echo", vec!["test".to_string()]);
    let handle = pool.push_task(config).expect("Failed to submit task");
    let _ = handle.wait();

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(100));

    // 验证成功率为 1.0
    let metrics = pool.metrics();
    assert_eq!(metrics.success_rate, 1.0, "Success rate should be 1.0 for single successful task");
    assert_eq!(metrics.tasks_submitted, 1);
    assert_eq!(metrics.tasks_completed, 1);
    assert_eq!(metrics.tasks_failed, 0);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_success_rate_single_failure() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交一个失败的任务
    let config = CommandConfig::new("nonexistent_command_xyz", vec![]);
    let handle = pool.push_task(config).expect("Failed to submit task");
    let _ = handle.wait();

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(100));

    // 验证成功率为 0.0
    let metrics = pool.metrics();
    assert_eq!(metrics.success_rate, 0.0, "Success rate should be 0.0 for single failed task");
    assert_eq!(metrics.tasks_submitted, 1);
    assert_eq!(metrics.tasks_completed, 0);
    assert_eq!(metrics.tasks_failed, 1);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_success_rate_mixed_tasks() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交混合任务：3个成功，2个失败
    let mut handles = Vec::new();
    
    // 3个成功任务
    for i in 0..3 {
        let config = CommandConfig::new("echo", vec![format!("success_{}", i)]);
        handles.push(pool.push_task(config).expect("Failed to submit task"));
    }
    
    // 2个失败任务
    for i in 0..2 {
        let config = CommandConfig::new("nonexistent_command_xyz", vec![format!("{}", i)]);
        handles.push(pool.push_task(config).expect("Failed to submit task"));
    }

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(100));

    // 验证成功率为 0.6 (3/5)
    let metrics = pool.metrics();
    let expected_rate = 3.0 / 5.0;
    let epsilon = 0.0001;
    assert!(
        (metrics.success_rate - expected_rate).abs() < epsilon,
        "Success rate should be 0.6, got {}",
        metrics.success_rate
    );
    assert_eq!(metrics.tasks_submitted, 5);
    assert_eq!(metrics.tasks_completed, 3);
    assert_eq!(metrics.tasks_failed, 2);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_success_rate_incremental_updates() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交第一个成功任务
    let config1 = CommandConfig::new("echo", vec!["task1".to_string()]);
    let handle1 = pool.push_task(config1).expect("Failed to submit task");
    let _ = handle1.wait();
    std::thread::sleep(Duration::from_millis(100));

    // 验证成功率为 1.0
    let metrics1 = pool.metrics();
    assert_eq!(metrics1.success_rate, 1.0, "Success rate should be 1.0 after first success");

    // 提交第二个失败任务
    let config2 = CommandConfig::new("nonexistent_command_xyz", vec![]);
    let handle2 = pool.push_task(config2).expect("Failed to submit task");
    let _ = handle2.wait();
    std::thread::sleep(Duration::from_millis(100));

    // 验证成功率为 0.5 (1/2)
    let metrics2 = pool.metrics();
    let expected_rate = 0.5;
    let epsilon = 0.0001;
    assert!(
        (metrics2.success_rate - expected_rate).abs() < epsilon,
        "Success rate should be 0.5 after one success and one failure, got {}",
        metrics2.success_rate
    );

    // 提交第三个成功任务
    let config3 = CommandConfig::new("echo", vec!["task3".to_string()]);
    let handle3 = pool.push_task(config3).expect("Failed to submit task");
    let _ = handle3.wait();
    std::thread::sleep(Duration::from_millis(100));

    // 验证成功率为 0.6667 (2/3)
    let metrics3 = pool.metrics();
    let expected_rate = 2.0 / 3.0;
    assert!(
        (metrics3.success_rate - expected_rate).abs() < epsilon,
        "Success rate should be 0.6667 after two successes and one failure, got {}",
        metrics3.success_rate
    );

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}
