use execute::{CommandConfig, CommandPool, ExecutionConfig};
use std::time::Duration;

fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Zombie Reaper Demo ===\n");

    // 创建带有僵尸进程清理器的命令池
    // 每 5 秒检查一次僵尸进程
    let config = ExecutionConfig::new()
        .with_workers(4)
        .with_zombie_reaper_interval(Duration::from_secs(5));

    let pool = CommandPool::with_config(config);

    println!("Starting command pool with zombie reaper (check interval: 5s)...");
    pool.start_executor(Duration::from_millis(100));

    // 提交一些任务
    println!("\nSubmitting 10 tasks...");
    for i in 0..10 {
        let task = CommandConfig::new(
            "sh",
            vec![
                "-c".to_string(),
                format!(
                    "echo 'Task {} running' && sleep 0.1 && echo 'Task {} done'",
                    i, i
                ),
            ],
        );
        pool.push_task(task).unwrap();
    }

    println!("Tasks submitted. Waiting for completion...");

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(3));

    // 检查指标
    let metrics = pool.metrics();
    println!("\n=== Metrics ===");
    println!("Tasks submitted: {}", metrics.tasks_submitted);
    println!("Tasks completed: {}", metrics.tasks_completed);
    println!("Tasks failed: {}", metrics.tasks_failed);
    println!("Success rate: {:.2}%", metrics.success_rate * 100.0);

    // 优雅关闭
    println!("\nShutting down command pool...");
    pool.shutdown().unwrap();
    println!("Command pool shut down successfully.");

    println!("\n=== Demo Complete ===");
    println!("The zombie reaper automatically cleaned up any zombie processes");
    println!("that were created during task execution.");
}
