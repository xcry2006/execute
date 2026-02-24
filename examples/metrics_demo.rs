/// 指标收集系统演示
///
/// 此示例展示如何使用指标收集系统监控命令池的性能和健康状态。
/// 包括：
/// - 任务计数（提交、完成、失败、取消）
/// - 队列状态（队列中、执行中）
/// - 执行时间统计（平均值、百分位数）
/// - 成功率计算
use execute::{CommandConfig, CommandPool, ExecutionConfig, LogConfig, LogLevel};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    let log_config = LogConfig::new().with_level(LogLevel::Info);
    log_config.init()?;

    println!("=== 指标收集系统演示 ===\n");

    // 创建命令池
    let config = ExecutionConfig::new().with_workers(4);
    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 场景 1: 初始状态
    println!("场景 1: 初始状态");
    print_metrics(&pool);
    println!();

    // 场景 2: 提交一些快速任务
    println!("场景 2: 提交 10 个快速任务");
    for i in 0..10 {
        let config = CommandConfig::new("echo", vec![format!("任务 {}", i)]);
        pool.push_task(config)?;
    }

    // 立即检查指标（任务在队列中）
    println!("提交后立即检查:");
    print_metrics(&pool);

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(1));
    println!("\n任务完成后:");
    print_metrics(&pool);
    println!();

    // 场景 3: 混合成功和失败的任务
    println!("场景 3: 混合成功和失败的任务");

    // 成功的任务
    for i in 0..5 {
        let config = CommandConfig::new("echo", vec![format!("成功任务 {}", i)]);
        pool.push_task(config)?;
    }

    // 失败的任务（命令不存在）
    for i in 0..3 {
        let config = CommandConfig::new(&format!("nonexistent_command_{}", i), vec![]);
        let _ = pool.push_task(config); // 忽略提交错误
    }

    // 超时的任务
    for _i in 0..2 {
        let config = CommandConfig::new("sleep", vec!["5".to_string()])
            .with_timeout(Duration::from_millis(50));
        pool.push_task(config)?;
    }

    std::thread::sleep(Duration::from_secs(1));
    print_metrics(&pool);
    println!();

    // 场景 4: 不同执行时间的任务
    println!("场景 4: 不同执行时间的任务（用于统计分析）");

    // 快速任务
    for _ in 0..20 {
        let config = CommandConfig::new("echo", vec!["快速".to_string()]);
        pool.push_task(config)?;
    }

    // 中等速度任务
    for _ in 0..10 {
        let config = CommandConfig::new(
            "sh",
            vec!["-c".to_string(), "sleep 0.1 && echo 中等".to_string()],
        );
        pool.push_task(config)?;
    }

    // 慢速任务
    for _ in 0..5 {
        let config = CommandConfig::new(
            "sh",
            vec!["-c".to_string(), "sleep 0.3 && echo 慢速".to_string()],
        );
        pool.push_task(config)?;
    }

    std::thread::sleep(Duration::from_secs(2));
    println!("执行时间分布:");
    print_detailed_metrics(&pool);
    println!();

    // 场景 5: 实时监控
    println!("场景 5: 实时监控（每秒更新）");

    // 提交一批任务
    for i in 0..20 {
        let config = CommandConfig::new(
            "sh",
            vec![
                "-c".to_string(),
                format!("sleep 0.{} && echo 任务 {}", i % 5, i),
            ],
        );
        pool.push_task(config)?;
    }

    // 每秒打印指标
    for second in 1..=5 {
        std::thread::sleep(Duration::from_secs(1));
        println!("\n第 {} 秒:", second);
        print_compact_metrics(&pool);
    }

    println!();

    // 优雅关闭
    println!("关闭命令池...");
    pool.shutdown_with_timeout(Duration::from_secs(5))?;

    // 最终指标
    println!("\n=== 最终指标 ===");
    print_detailed_metrics(&pool);

    println!("\n=== 演示完成 ===");
    Ok(())
}

/// 打印基本指标
fn print_metrics(pool: &CommandPool) {
    let metrics = pool.metrics();

    println!("  任务计数:");
    println!("    - 已提交: {}", metrics.tasks_submitted);
    println!("    - 已完成: {}", metrics.tasks_completed);
    println!("    - 已失败: {}", metrics.tasks_failed);
    println!("    - 队列中: {}", metrics.tasks_queued);
    println!("    - 执行中: {}", metrics.tasks_running);
    println!("  成功率: {:.2}%", metrics.success_rate * 100.0);
}

/// 打印详细指标（包括执行时间统计）
fn print_detailed_metrics(pool: &CommandPool) {
    let metrics = pool.metrics();

    println!("  任务统计:");
    println!("    - 已提交: {}", metrics.tasks_submitted);
    println!("    - 已完成: {}", metrics.tasks_completed);
    println!("    - 已失败: {}", metrics.tasks_failed);
    println!("    - 已取消: {}", metrics.tasks_cancelled);
    println!("    - 成功率: {:.2}%", metrics.success_rate * 100.0);

    println!("  执行时间:");
    println!("    - 平均: {:?}", metrics.avg_execution_time);
    println!("    - 最小: {:?}", metrics.min_execution_time);
    println!("    - 最大: {:?}", metrics.max_execution_time);
    println!("    - P50 (中位数): {:?}", metrics.p50_execution_time);
    println!("    - P95: {:?}", metrics.p95_execution_time);
    println!("    - P99: {:?}", metrics.p99_execution_time);

    println!("  当前状态:");
    println!("    - 队列中: {}", metrics.tasks_queued);
    println!("    - 执行中: {}", metrics.tasks_running);
}

/// 打印紧凑格式的指标（用于实时监控）
fn print_compact_metrics(pool: &CommandPool) {
    let metrics = pool.metrics();

    println!(
        "  提交:{} 完成:{} 失败:{} 队列:{} 执行:{} 成功率:{:.1}% 平均:{:?}",
        metrics.tasks_submitted,
        metrics.tasks_completed,
        metrics.tasks_failed,
        metrics.tasks_queued,
        metrics.tasks_running,
        metrics.success_rate * 100.0,
        metrics.avg_execution_time
    );
}
