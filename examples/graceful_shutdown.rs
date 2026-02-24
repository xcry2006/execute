use execute::{CommandConfig, CommandPool, ShutdownConfig};
use std::time::Duration;

fn main() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== 优雅关闭示例 ===\n");

    // 创建命令池
    let mut pool = CommandPool::new();

    // 配置关闭行为：60秒超时
    pool.set_shutdown_config(ShutdownConfig::new(Duration::from_secs(60)));

    // 启动执行器
    pool.start_executor(Duration::from_millis(100));
    println!("命令池已启动\n");

    // 提交一些任务
    println!("提交任务...");
    for i in 1..=5 {
        let _ = pool.push_task(CommandConfig::new("echo", vec![format!("任务 {}", i)]));
        println!("  - 已提交任务 {}", i);
    }
    println!("已提交 5 个任务\n");

    // 等待一下让任务开始执行
    std::thread::sleep(Duration::from_millis(500));

    // 开始优雅关闭
    println!("开始优雅关闭...");
    match pool.shutdown_with_timeout(Duration::from_secs(10)) {
        Ok(_) => println!("✓ 优雅关闭成功！所有任务已完成。\n"),
        Err(e) => println!("✗ 关闭失败: {}\n", e),
    }

    // 尝试在关闭后提交任务
    println!("尝试在关闭后提交任务...");
    match pool.push_task(CommandConfig::new("echo", vec!["关闭后的任务".to_string()])) {
        Ok(_) => println!("  - 任务提交成功（不应该发生）"),
        Err(e) => println!("  - 任务提交失败（预期行为）: {}", e),
    }

    println!("\n=== 示例完成 ===");
}
