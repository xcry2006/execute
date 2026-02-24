use execute::{CommandConfig, CommandPool, ExecutionConfig, HealthStatus};
use std::time::Duration;

#[test]
fn test_health_check_healthy() {
    // 创建命令池
    let pool = CommandPool::with_config(ExecutionConfig {
        workers: 2,
        ..Default::default()
    });

    // 启动执行器
    pool.start_executor();

    // 等待一小段时间让工作线程启动
    std::thread::sleep(Duration::from_millis(200));

    // 执行健康检查
    let health = pool.health_check();

    // 验证健康状态
    assert_eq!(health.status, HealthStatus::Healthy);
    assert_eq!(health.details.workers_total, 2);
    assert_eq!(health.details.workers_alive, 2);
    assert!(health.details.queue_usage >= 0.0 && health.details.queue_usage <= 1.0);

    // 清理 - 使用 shutdown 而不是 stop
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_health_check_degraded_high_queue_usage() {
    // 创建有队列限制的命令池
    let pool = CommandPool::with_config_and_limit(
        ExecutionConfig {
            workers: 1,
            ..Default::default()
        },
        10, // 队列容量为 10
    );

    // 不启动执行器，这样任务会堆积在队列中

    // 添加 10 个任务填满队列（使用 try_push_task 避免阻塞）
    for i in 0..10 {
        let task = CommandConfig::new("echo", vec![format!("task {}", i)]);
        let _ = pool.try_push_task(task);
    }

    // 执行健康检查
    let health = pool.health_check();

    // 验证队列使用率高
    assert!(health.details.queue_usage >= 0.9);

    // 应该是 Degraded 或 Unhealthy 状态（因为队列使用率高且没有工作线程）
    match health.status {
        HealthStatus::Degraded { issues } | HealthStatus::Unhealthy { issues } => {
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.contains("Queue usage high")
                        || issue.contains("workers alive"))
            );
        }
        _ => panic!("Expected Degraded or Unhealthy status due to high queue usage and no workers"),
    }

    // 清理
    pool.clear();
}

#[test]
fn test_health_check_unhealthy_no_workers() {
    // 创建命令池但不启动执行器
    let pool = CommandPool::with_config(ExecutionConfig {
        workers: 2,
        ..Default::default()
    });

    // 不启动执行器，所以没有工作线程

    // 执行健康检查
    let health = pool.health_check();

    // 验证没有存活的工作线程
    assert_eq!(health.details.workers_alive, 0);
    assert_eq!(health.details.workers_total, 2);

    // 应该是 Unhealthy 状态（因为没有工作线程）
    match health.status {
        HealthStatus::Unhealthy { issues } => {
            assert!(issues.iter().any(|issue| issue.contains("workers alive")));
        }
        _ => panic!("Expected Unhealthy status due to no workers"),
    }
}

#[test]
fn test_health_check_details() {
    // 创建命令池
    let pool = CommandPool::with_config(ExecutionConfig {
        workers: 4,
        ..Default::default()
    });

    pool.start_executor();

    // 等待工作线程启动
    std::thread::sleep(Duration::from_millis(200));

    // 执行健康检查
    let health = pool.health_check();

    // 验证详细信息
    assert_eq!(health.details.workers_total, 4);
    assert!(health.details.workers_alive <= 4);
    assert!(health.details.queue_usage >= 0.0);
    assert!(health.details.queue_usage <= 1.0);
    assert_eq!(health.details.long_running_tasks, 0); // 没有任务在运行

    // 清理 - 使用 shutdown 而不是 stop
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}
