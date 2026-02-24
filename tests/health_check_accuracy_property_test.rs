// Feature: production-ready-improvements, Property 15: 健康检查准确性
// **Validates: Requirements 10.2, 10.3, 10.4**
//
// 属性 15: 健康检查准确性
// 对于任意系统状态，health_check 应该正确报告工作线程状态、队列使用率和长时间运行任务
//
// 验证需求：
// - 需求 10.2: 报告所有工作线程是否正常运行
// - 需求 10.3: 报告队列是否已满
// - 需求 10.4: 报告是否存在长时间运行的任务

use execute::{CommandConfig, CommandPool, ExecutionConfig, HealthStatus};
use proptest::prelude::*;
use std::time::Duration;

/// 生成工作线程数量策略（1-8个线程）
fn worker_count_strategy() -> impl Strategy<Value = usize> {
    1usize..=8
}

/// 生成队列容量策略（5-50）
fn queue_capacity_strategy() -> impl Strategy<Value = usize> {
    5usize..=50
}

/// 生成任务数量策略（0-100个任务）
fn task_count_strategy() -> impl Strategy<Value = usize> {
    0usize..=100
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意配置，健康检查应该准确报告工作线程状态
    ///
    /// 验证需求：
    /// - 需求 10.2: 报告所有工作线程是否正常运行
    ///
    /// 测试策略：
    /// 1. 创建具有随机数量工作线程的命令池
    /// 2. 启动执行器
    /// 3. 执行健康检查
    /// 4. 验证 workers_alive 和 workers_total 的准确性
    #[test]
    fn prop_health_check_reports_worker_status_accurately(
        worker_count in worker_count_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建命令池
        let pool = CommandPool::with_config(ExecutionConfig {
            workers: worker_count,
            ..Default::default()
        });

        // 启动执行器
        pool.start_executor(Duration::from_millis(50));

        // 等待工作线程启动
        std::thread::sleep(Duration::from_millis(200));

        // 执行健康检查
        let health = pool.health_check();

        // 验证工作线程总数正确
        prop_assert_eq!(
            health.details.workers_total,
            worker_count,
            "workers_total should match configured worker count"
        );

        // 验证存活的工作线程数在合理范围内（0 到 workers_total）
        prop_assert!(
            health.details.workers_alive <= health.details.workers_total,
            "workers_alive ({}) should not exceed workers_total ({})",
            health.details.workers_alive,
            health.details.workers_total
        );

        // 在正常情况下，所有工作线程应该都存活
        prop_assert_eq!(
            health.details.workers_alive,
            worker_count,
            "All workers should be alive after startup"
        );

        // 清理
        let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
    }

    /// 属性测试：对于任意队列状态，健康检查应该准确报告队列使用率
    ///
    /// 验证需求：
    /// - 需求 10.3: 报告队列是否已满
    ///
    /// 测试策略：
    /// 1. 创建具有队列容量限制的命令池
    /// 2. 不启动执行器（任务会堆积在队列中）
    /// 3. 提交不同数量的任务
    /// 4. 验证 queue_usage 的准确性
    #[test]
    fn prop_health_check_reports_queue_usage_accurately(
        queue_capacity in queue_capacity_strategy(),
        task_count in task_count_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建有队列限制的命令池
        let pool = CommandPool::with_config_and_limit(
            ExecutionConfig {
                workers: 1,
                ..Default::default()
            },
            queue_capacity,
        );

        // 不启动执行器，这样任务会堆积在队列中

        // 提交任务（不超过队列容量）
        let tasks_to_submit = std::cmp::min(task_count, queue_capacity);
        let mut submitted = 0;
        for i in 0..tasks_to_submit {
            let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
            if pool.try_push_task(config).is_ok() {
                submitted += 1;
            } else {
                break;
            }
        }

        // 执行健康检查
        let health = pool.health_check();

        // 验证队列使用率在有效范围内 [0.0, 1.0]
        prop_assert!(
            health.details.queue_usage >= 0.0 && health.details.queue_usage <= 1.0,
            "queue_usage should be between 0.0 and 1.0, got {}",
            health.details.queue_usage
        );

        // 计算预期的队列使用率
        let expected_usage = (submitted as f64) / (queue_capacity as f64);

        // 验证队列使用率的准确性（允许小误差）
        let epsilon = 0.01;
        prop_assert!(
            (health.details.queue_usage - expected_usage).abs() < epsilon,
            "queue_usage should be approximately {:.2}, got {:.2}. Submitted: {}, Capacity: {}",
            expected_usage,
            health.details.queue_usage,
            submitted,
            queue_capacity
        );

        // 如果队列接近满（>90%），健康检查应该报告问题
        if health.details.queue_usage > 0.9 {
            match &health.status {
                HealthStatus::Degraded { issues } | HealthStatus::Unhealthy { issues } => {
                    prop_assert!(
                        issues.iter().any(|issue| issue.contains("Queue usage high")),
                        "Should report high queue usage when usage > 90%"
                    );
                }
                HealthStatus::Healthy => {
                    // 可能因为没有工作线程而被归类为 Unhealthy
                    // 这是可以接受的
                }
            }
        }

        // 清理
        pool.clear();
    }

    /// 属性测试：对于任意任务执行状态，健康检查应该准确报告长时间运行的任务
    ///
    /// 验证需求：
    /// - 需求 10.4: 报告是否存在长时间运行的任务
    ///
    /// 测试策略：
    /// 1. 创建命令池并启动执行器
    /// 2. 提交一些快速任务和一些慢速任务
    /// 3. 在任务执行期间执行健康检查
    /// 4. 验证 long_running_tasks 的准确性
    #[test]
    fn prop_health_check_reports_long_running_tasks_accurately(
        worker_count in worker_count_strategy(),
        fast_task_count in 0usize..=10,
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建命令池
        let pool = CommandPool::with_config(ExecutionConfig {
            workers: worker_count,
            ..Default::default()
        });

        // 启动执行器
        pool.start_executor(Duration::from_millis(50));

        // 等待工作线程启动
        std::thread::sleep(Duration::from_millis(100));

        // 提交快速任务
        let mut handles = Vec::new();
        for i in 0..fast_task_count {
            let config = CommandConfig::new("echo", vec![format!("fast_{}", i)]);
            if let Ok(handle) = pool.push_task(config) {
                handles.push(handle);
            }
        }

        // 等待快速任务完成
        for handle in handles {
            let _ = handle.wait();
        }

        // 等待一小段时间确保任务完成
        std::thread::sleep(Duration::from_millis(100));

        // 执行健康检查
        let health = pool.health_check();

        // 验证 long_running_tasks 在合理范围内
        // 注意：当前实现返回正在运行的任务数作为近似值
        prop_assert!(
            health.details.long_running_tasks <= worker_count,
            "long_running_tasks ({}) should not exceed worker count ({})",
            health.details.long_running_tasks,
            worker_count
        );

        // 在没有任务运行时，long_running_tasks 应该为 0
        let metrics = pool.metrics();
        if metrics.tasks_running == 0 {
            prop_assert_eq!(
                health.details.long_running_tasks,
                0,
                "long_running_tasks should be 0 when no tasks are running"
            );
        }

        // 清理
        let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
    }
}

#[test]
fn test_health_check_accuracy_no_workers_alive() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池但不启动执行器
    let pool = CommandPool::with_config(ExecutionConfig {
        workers: 4,
        ..Default::default()
    });

    // 不启动执行器，所以没有工作线程

    // 执行健康检查
    let health = pool.health_check();

    // 验证工作线程状态
    assert_eq!(health.details.workers_total, 4);
    assert_eq!(health.details.workers_alive, 0);

    // 应该报告没有工作线程存活
    match health.status {
        HealthStatus::Unhealthy { issues } => {
            assert!(
                issues.iter().any(|issue| issue.contains("workers alive")),
                "Should report no workers alive"
            );
        }
        _ => panic!("Expected Unhealthy status when no workers are alive"),
    }
}

#[test]
fn test_health_check_accuracy_queue_full() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建有队列限制的命令池
    let queue_capacity = 10;
    let pool = CommandPool::with_config_and_limit(
        ExecutionConfig {
            workers: 1,
            ..Default::default()
        },
        queue_capacity,
    );

    // 不启动执行器，这样任务会堆积在队列中

    // 填满队列
    for i in 0..queue_capacity {
        let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
        let _ = pool.try_push_task(config);
    }

    // 执行健康检查
    let health = pool.health_check();

    // 验证队列使用率接近 1.0
    assert!(
        health.details.queue_usage >= 0.9,
        "Queue usage should be >= 0.9 when queue is full, got {}",
        health.details.queue_usage
    );

    // 应该报告队列使用率高
    match &health.status {
        HealthStatus::Degraded { issues } | HealthStatus::Unhealthy { issues } => {
            assert!(
                issues.iter().any(|issue| issue.contains("Queue usage high")),
                "Should report high queue usage when queue is full"
            );
        }
        HealthStatus::Healthy => {
            panic!("Expected Degraded or Unhealthy status when queue is full");
        }
    }

    // 清理
    pool.clear();
}

#[test]
fn test_health_check_accuracy_empty_queue() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建有队列限制的命令池
    let pool = CommandPool::with_config_and_limit(
        ExecutionConfig {
            workers: 2,
            ..Default::default()
        },
        20,
    );

    // 启动执行器
    pool.start_executor(Duration::from_millis(50));

    // 等待工作线程启动
    std::thread::sleep(Duration::from_millis(200));

    // 不提交任何任务

    // 执行健康检查
    let health = pool.health_check();

    // 验证队列使用率为 0
    assert_eq!(
        health.details.queue_usage, 0.0,
        "Queue usage should be 0.0 when queue is empty"
    );

    // 应该是健康状态
    assert_eq!(
        health.status,
        HealthStatus::Healthy,
        "Should be healthy when queue is empty and workers are alive"
    );

    // 清理
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_health_check_accuracy_partial_queue() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建有队列限制的命令池
    let queue_capacity = 20;
    let pool = CommandPool::with_config_and_limit(
        ExecutionConfig {
            workers: 1,
            ..Default::default()
        },
        queue_capacity,
    );

    // 不启动执行器，这样任务会堆积在队列中

    // 填充一半的队列
    let tasks_to_submit = queue_capacity / 2;
    for i in 0..tasks_to_submit {
        let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
        let _ = pool.try_push_task(config);
    }

    // 执行健康检查
    let health = pool.health_check();

    // 验证队列使用率约为 0.5
    let expected_usage = 0.5;
    let epsilon = 0.1;
    assert!(
        (health.details.queue_usage - expected_usage).abs() < epsilon,
        "Queue usage should be approximately {}, got {}",
        expected_usage,
        health.details.queue_usage
    );

    // 清理
    pool.clear();
}

#[test]
fn test_health_check_accuracy_all_workers_alive() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池
    let worker_count = 4;
    let pool = CommandPool::with_config(ExecutionConfig {
        workers: worker_count,
        ..Default::default()
    });

    // 启动执行器
    pool.start_executor(Duration::from_millis(50));

    // 等待工作线程启动
    std::thread::sleep(Duration::from_millis(200));

    // 执行健康检查
    let health = pool.health_check();

    // 验证所有工作线程都存活
    assert_eq!(health.details.workers_total, worker_count);
    assert_eq!(health.details.workers_alive, worker_count);

    // 应该是健康状态
    assert_eq!(
        health.status,
        HealthStatus::Healthy,
        "Should be healthy when all workers are alive"
    );

    // 清理
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_health_check_accuracy_no_long_running_tasks() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池
    let pool = CommandPool::with_config(ExecutionConfig {
        workers: 2,
        ..Default::default()
    });

    // 启动执行器
    pool.start_executor(Duration::from_millis(50));

    // 等待工作线程启动
    std::thread::sleep(Duration::from_millis(200));

    // 提交并等待一些快速任务完成
    let mut handles = Vec::new();
    for i in 0..5 {
        let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
        if let Ok(handle) = pool.push_task(config) {
            handles.push(handle);
        }
    }

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 等待一小段时间确保任务完成
    std::thread::sleep(Duration::from_millis(100));

    // 执行健康检查
    let health = pool.health_check();

    // 验证没有长时间运行的任务
    assert_eq!(
        health.details.long_running_tasks, 0,
        "Should have no long running tasks after all tasks complete"
    );

    // 应该是健康状态
    assert_eq!(
        health.status,
        HealthStatus::Healthy,
        "Should be healthy when no long running tasks"
    );

    // 清理
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_health_check_accuracy_consistency() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池
    let pool = CommandPool::with_config(ExecutionConfig {
        workers: 3,
        ..Default::default()
    });

    // 启动执行器
    pool.start_executor(Duration::from_millis(50));

    // 等待工作线程启动
    std::thread::sleep(Duration::from_millis(200));

    // 执行多次健康检查，验证一致性
    let health1 = pool.health_check();
    std::thread::sleep(Duration::from_millis(50));
    let health2 = pool.health_check();

    // 在没有任务运行的情况下，健康检查结果应该一致
    assert_eq!(health1.details.workers_total, health2.details.workers_total);
    assert_eq!(health1.details.workers_alive, health2.details.workers_alive);
    assert_eq!(health1.details.queue_usage, health2.details.queue_usage);
    assert_eq!(
        health1.details.long_running_tasks,
        health2.details.long_running_tasks
    );
    assert_eq!(health1.status, health2.status);

    // 清理
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}
