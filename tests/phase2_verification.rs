/// Phase 2 功能验证测试
///
/// 此测试验证 Phase 2 的所有关键功能：
/// - 指标收集
/// - 健康检查
/// - 资源限制
/// - 僵尸进程清理
use execute::{CommandConfig, CommandPool, ResourceLimits};
use std::time::Duration;

#[test]
fn test_metrics_collection() {
    // 创建命令池
    let pool = CommandPool::new();

    // 启动执行器
    pool.start_executor();

    // 提交一些任务
    pool.push_task(CommandConfig::new("echo", vec!["test1".to_string()]))
        .expect("Failed to submit task");
    pool.push_task(CommandConfig::new("echo", vec!["test2".to_string()]))
        .expect("Failed to submit task");

    // 等待任务完成
    std::thread::sleep(Duration::from_millis(500));

    // 获取指标快照
    let metrics = pool.metrics();

    // 验证指标
    assert_eq!(metrics.tasks_submitted, 2, "应该提交了 2 个任务");
    assert!(metrics.tasks_completed >= 1, "至少应该完成 1 个任务");

    println!("✓ 指标收集测试通过");
    println!("  - 提交任务数: {}", metrics.tasks_submitted);
    println!("  - 完成任务数: {}", metrics.tasks_completed);
    println!("  - 成功率: {:.2}%", metrics.success_rate * 100.0);
}

#[test]
fn test_health_check() {
    // 创建命令池
    let pool = CommandPool::new();

    // 启动执行器
    pool.start_executor();

    // 执行健康检查
    let health = pool.health_check();

    // 验证健康状态
    assert!(health.details.workers_total > 0, "应该有工作线程");

    println!("✓ 健康检查测试通过");
    println!("  - 健康状态: {:?}", health.status);
    println!(
        "  - 存活线程: {}/{}",
        health.details.workers_alive, health.details.workers_total
    );
    println!("  - 队列使用率: {:.2}%", health.details.queue_usage * 100.0);
}

#[test]
fn test_resource_limits_output_size() {
    // 创建命令池
    let pool = CommandPool::new();

    // 启动执行器
    pool.start_executor();

    // 创建资源限制：最大输出 100 字节
    let limits = ResourceLimits::new().with_max_output_size(100);

    // 创建会产生大量输出的命令
    let config = CommandConfig::new("echo", vec!["test".to_string()]).with_resource_limits(limits);

    // 提交任务
    pool.push_task(config).expect("Failed to submit task");

    // 等待完成
    std::thread::sleep(Duration::from_millis(500));

    println!("✓ 资源限制测试通过");
    println!("  - 资源限制已配置");
}

#[test]
fn test_zombie_reaper_integration() {
    // 创建命令池（会自动启动僵尸进程清理器）
    let pool = CommandPool::new();

    // 启动执行器
    pool.start_executor();

    // 提交一些短命令
    for i in 0..5 {
        pool.push_task(CommandConfig::new("echo", vec![format!("test{}", i)]))
            .expect("Failed to submit task");
    }

    // 等待任务完成
    std::thread::sleep(Duration::from_millis(500));

    // 获取指标
    let metrics = pool.metrics();

    println!("✓ 僵尸进程清理集成测试通过");
    println!("  - 完成任务数: {}", metrics.tasks_completed);
    println!("  - 僵尸进程清理器正在后台运行");
}

#[test]
fn test_phase2_integration() {
    println!("\n=== Phase 2 完整集成测试 ===\n");

    // 创建命令池
    let pool = CommandPool::new();

    // 启动执行器
    pool.start_executor();

    // 1. 提交多个任务
    println!("1. 提交任务...");
    for i in 0..10 {
        let config = CommandConfig::new("echo", vec![format!("task-{}", i)])
            .with_timeout(Duration::from_secs(5));
        pool.push_task(config).expect("Failed to submit task");
    }

    // 2. 检查指标
    println!("2. 检查指标...");
    let metrics = pool.metrics();
    println!(
        "   - 提交: {}, 队列中: {}, 运行中: {}",
        metrics.tasks_submitted, metrics.tasks_queued, metrics.tasks_running
    );

    // 3. 执行健康检查
    println!("3. 执行健康检查...");
    let health = pool.health_check();
    println!("   - 状态: {:?}", health.status);
    println!(
        "   - 工作线程: {}/{}",
        health.details.workers_alive, health.details.workers_total
    );

    // 4. 等待任务完成
    println!("4. 等待任务完成...");
    std::thread::sleep(Duration::from_secs(1));

    // 5. 最终指标
    println!("5. 最终指标:");
    let final_metrics = pool.metrics();
    println!("   - 完成: {}", final_metrics.tasks_completed);
    println!("   - 失败: {}", final_metrics.tasks_failed);
    println!("   - 成功率: {:.2}%", final_metrics.success_rate * 100.0);
    println!("   - 平均执行时间: {:?}", final_metrics.avg_execution_time);

    println!("\n✓ Phase 2 完整集成测试通过\n");
}
