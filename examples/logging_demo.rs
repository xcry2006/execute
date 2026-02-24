use execute::{CommandConfig, CommandPool, LogConfig, LogLevel};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志系统
    let log_config = LogConfig::new().with_level(LogLevel::Debug);

    log_config.init()?;

    println!("=== 日志和指标演示 ===\n");

    // 创建命令池
    let pool = CommandPool::new();

    // 提交一些任务
    println!("提交任务...");
    let _ = pool.push_task(CommandConfig::new(
        "echo",
        vec!["Hello, World!".to_string()],
    ));
    let _ = pool.push_task(CommandConfig::new("echo", vec!["Task 2".to_string()]));
    let _ = pool.push_task(CommandConfig::new("echo", vec!["Task 3".to_string()]));

    // 启动执行器
    pool.start_executor();

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(1));

    // 获取指标
    let metrics = pool.metrics();
    println!("\n=== 执行指标 ===");
    println!("提交任务数: {}", metrics.tasks_submitted);
    println!("完成任务数: {}", metrics.tasks_completed);
    println!("失败任务数: {}", metrics.tasks_failed);
    println!("队列中任务: {}", metrics.tasks_queued);
    println!("执行中任务: {}", metrics.tasks_running);
    println!("成功率: {:.2}%", metrics.success_rate * 100.0);
    println!("平均执行时间: {:?}", metrics.avg_execution_time);
    println!("最小执行时间: {:?}", metrics.min_execution_time);
    println!("最大执行时间: {:?}", metrics.max_execution_time);

    Ok(())
}
