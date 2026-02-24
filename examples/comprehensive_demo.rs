/// 综合示例 - 展示所有主要功能
///
/// 此示例演示了命令池库的主要生产环境就绪功能：
/// - 结构化日志和追踪
/// - 指标收集
/// - 健康检查
/// - 优雅关闭
/// - 错误重试
/// - 超时控制
/// - 环境变量
/// - 资源限制
/// - 僵尸进程清理
use execute::{
    CommandConfig, CommandPool, EnvConfig, ExecutionConfig, HealthStatus, LogConfig, LogLevel,
    ResourceLimits, RetryPolicy, RetryStrategy,
};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 配置结构化日志
    let log_config = LogConfig::new().with_level(LogLevel::Info);
    log_config.init()?;

    println!("=== 命令池综合功能演示 ===\n");

    // 2. 创建带有完整配置的命令池
    let config = ExecutionConfig::new()
        .with_workers(4)
        .with_zombie_reaper_interval(Duration::from_secs(10));

    let pool = CommandPool::with_config(config);

    // 启动执行器
    pool.start_executor();
    println!("✓ 命令池已启动 (4 个工作线程)\n");

    // 3. 演示各种功能
    demo_basic_execution(&pool);
    demo_retry_mechanism(&pool);
    demo_timeout_control(&pool);
    demo_environment_variables(&pool);
    demo_resource_limits(&pool);

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(2));

    // 4. 检查健康状态
    demo_health_check(&pool);

    // 5. 查看指标
    demo_metrics(&pool);

    // 6. 优雅关闭
    println!("\n=== 优雅关闭 ===");
    println!("开始关闭命令池...");
    pool.shutdown_with_timeout(Duration::from_secs(10))?;
    println!("✓ 命令池已成功关闭\n");

    println!("=== 演示完成 ===");
    Ok(())
}

fn demo_basic_execution(pool: &CommandPool) {
    println!("=== 1. 基本任务执行 ===");

    for i in 1..=3 {
        let config = CommandConfig::new("echo", vec![format!("任务 {}", i)]);
        pool.push_task(config).unwrap();
    }

    println!("✓ 已提交 3 个基本任务\n");
    std::thread::sleep(Duration::from_millis(500));
}

fn demo_retry_mechanism(pool: &CommandPool) {
    println!("=== 2. 重试机制 ===");

    // 配置指数退避重试策略
    let retry_policy = RetryPolicy::new(
        2,
        RetryStrategy::ExponentialBackoff {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(1),
            multiplier: 2.0,
        },
    );

    // 提交一个会超时的任务（将会重试）
    let config = CommandConfig::new("sleep", vec!["2".to_string()])
        .with_timeout(Duration::from_millis(50))
        .with_retry(retry_policy);

    pool.push_task(config).unwrap();
    println!("✓ 已提交带重试的任务 (最多重试 2 次)\n");
}

fn demo_timeout_control(pool: &CommandPool) {
    println!("=== 3. 超时控制 ===");

    let config = CommandConfig::new("echo", vec!["超时控制测试".to_string()])
        .with_timeout(Duration::from_secs(5));

    pool.push_task(config).unwrap();
    println!("✓ 已提交带超时控制的任务\n");
}

fn demo_environment_variables(pool: &CommandPool) {
    println!("=== 4. 环境变量支持 ===");

    // 配置环境变量
    let env_config = EnvConfig::new()
        .set("DEMO_VAR", "Hello from environment")
        .set("CUSTOM_PATH", "/custom/path");

    let config = CommandConfig::new(
        "sh",
        vec!["-c".to_string(), "echo \"DEMO_VAR=$DEMO_VAR\"".to_string()],
    )
    .with_env(env_config);

    pool.push_task(config).unwrap();
    println!("✓ 已提交带环境变量的任务\n");
}

fn demo_resource_limits(pool: &CommandPool) {
    println!("=== 5. 资源限制 ===");

    // 配置资源限制
    let limits = ResourceLimits::new()
        .with_max_output_size(1024) // 1 KB
        .with_max_memory(50 * 1024 * 1024); // 50 MB

    let config = CommandConfig::new("ls", vec!["-la".to_string()]).with_resource_limits(limits);

    pool.push_task(config).unwrap();
    println!("✓ 已提交带资源限制的任务\n");
}

fn demo_health_check(pool: &CommandPool) {
    println!("=== 健康检查 ===");

    let health = pool.health_check();

    match &health.status {
        HealthStatus::Healthy => {
            println!("✓ 系统健康");
        }
        HealthStatus::Degraded { issues } => {
            println!("⚠ 系统降级:");
            for issue in issues {
                println!("  - {}", issue);
            }
        }
        HealthStatus::Unhealthy { issues } => {
            println!("✗ 系统不健康:");
            for issue in issues {
                println!("  - {}", issue);
            }
        }
    }

    println!(
        "工作线程: {}/{} 存活",
        health.details.workers_alive, health.details.workers_total
    );
    println!("队列使用率: {:.1}%", health.details.queue_usage * 100.0);
    println!("长时间运行任务: {}", health.details.long_running_tasks);
    println!();
}

fn demo_metrics(pool: &CommandPool) {
    println!("=== 执行指标 ===");

    let metrics = pool.metrics();

    println!("任务统计:");
    println!("  - 已提交: {}", metrics.tasks_submitted);
    println!("  - 已完成: {}", metrics.tasks_completed);
    println!("  - 已失败: {}", metrics.tasks_failed);
    println!("  - 已取消: {}", metrics.tasks_cancelled);
    println!("  - 队列中: {}", metrics.tasks_queued);
    println!("  - 执行中: {}", metrics.tasks_running);

    println!("\n性能指标:");
    println!("  - 成功率: {:.2}%", metrics.success_rate * 100.0);
    println!("  - 平均执行时间: {:?}", metrics.avg_execution_time);
    println!("  - 最小执行时间: {:?}", metrics.min_execution_time);
    println!("  - 最大执行时间: {:?}", metrics.max_execution_time);
    println!("  - P50 执行时间: {:?}", metrics.p50_execution_time);
    println!("  - P95 执行时间: {:?}", metrics.p95_execution_time);
    println!("  - P99 执行时间: {:?}", metrics.p99_execution_time);
}
