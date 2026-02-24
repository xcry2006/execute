use execute::{CommandConfig, CommandPool, ExecutionConfig, HealthStatus};
use std::time::Duration;

fn main() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== Health Check Demo ===\n");

    // 创建命令池
    let pool = CommandPool::with_config(ExecutionConfig {
        workers: 4,
        ..Default::default()
    });

    // 场景 1: 未启动执行器 - 应该是 Unhealthy
    println!("Scenario 1: Pool without workers");
    let health = pool.health_check();
    print_health_status(&health);

    // 启动执行器
    pool.start_executor();
    std::thread::sleep(Duration::from_millis(200));

    // 场景 2: 正常运行 - 应该是 Healthy
    println!("\nScenario 2: Pool with workers running");
    let health = pool.health_check();
    print_health_status(&health);

    // 场景 3: 提交一些任务
    println!("\nScenario 3: Pool with tasks");
    for i in 0..5 {
        let cmd = CommandConfig::new("echo", vec![format!("Task {}", i)]);
        let _ = pool.push_task(cmd);
    }

    let health = pool.health_check();
    print_health_status(&health);

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(1));

    // 场景 4: 任务完成后
    println!("\nScenario 4: After tasks complete");
    let health = pool.health_check();
    print_health_status(&health);

    // 优雅关闭
    println!("\nShutting down...");
    let _ = pool.shutdown_with_timeout(Duration::from_secs(5));
    println!("Shutdown complete!");
}

fn print_health_status(health: &execute::HealthCheck) {
    println!("Health Status: {:?}", health.status);
    println!("Details:");
    println!(
        "  Workers: {}/{} alive",
        health.details.workers_alive, health.details.workers_total
    );
    println!("  Queue usage: {:.1}%", health.details.queue_usage * 100.0);
    println!(
        "  Long running tasks: {}",
        health.details.long_running_tasks
    );
    println!(
        "  Avg task duration: {:?}",
        health.details.avg_task_duration
    );

    match &health.status {
        HealthStatus::Healthy => {
            println!("  ✓ System is healthy");
        }
        HealthStatus::Degraded { issues } => {
            println!("  ⚠ System is degraded:");
            for issue in issues {
                println!("    - {}", issue);
            }
        }
        HealthStatus::Unhealthy { issues } => {
            println!("  ✗ System is unhealthy:");
            for issue in issues {
                println!("    - {}", issue);
            }
        }
    }
}
