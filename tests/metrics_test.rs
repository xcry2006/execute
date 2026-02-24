use execute::{CommandConfig, CommandPool, ExecutionConfig, ExecutionMode};
use std::time::Duration;

#[test]
fn test_metrics_collection() {
    // 创建命令池
    let config = ExecutionConfig {
        mode: ExecutionMode::Process,
        workers: 2,
        concurrency_limit: None,
        zombie_reaper_interval: None,
    };
    let pool = CommandPool::with_config(config);

    // 启动执行器
    pool.start_executor(Duration::from_millis(50));

    // 提交一些任务
    for i in 0..5 {
        let cmd = CommandConfig::new("echo", vec![format!("test_{}", i)]);
        pool.push_task(cmd).unwrap();
    }

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(2));

    // 停止执行器
    pool.stop();

    // 获取指标快照
    let metrics = pool.metrics();

    // 验证指标
    println!("Metrics snapshot:");
    println!("  Tasks submitted: {}", metrics.tasks_submitted);
    println!("  Tasks completed: {}", metrics.tasks_completed);
    println!("  Tasks failed: {}", metrics.tasks_failed);
    println!("  Tasks queued: {}", metrics.tasks_queued);
    println!("  Tasks running: {}", metrics.tasks_running);
    println!("  Success rate: {:.2}%", metrics.success_rate * 100.0);
    println!("  Avg execution time: {:?}", metrics.avg_execution_time);
    println!("  Min execution time: {:?}", metrics.min_execution_time);
    println!("  Max execution time: {:?}", metrics.max_execution_time);
    println!("  P50 execution time: {:?}", metrics.p50_execution_time);
    println!("  P95 execution time: {:?}", metrics.p95_execution_time);
    println!("  P99 execution time: {:?}", metrics.p99_execution_time);

    // 基本断言
    assert_eq!(metrics.tasks_submitted, 5);
    assert!(
        metrics.tasks_completed > 0,
        "At least some tasks should complete"
    );

    // 如果有完成的任务，验证百分位数不为零
    if metrics.tasks_completed > 0 {
        assert!(metrics.avg_execution_time > Duration::ZERO);
        assert!(metrics.p50_execution_time > Duration::ZERO);
        // P95 和 P99 可能为零，如果样本太少
    }
}

#[test]
fn test_metrics_percentiles_with_many_tasks() {
    // 创建命令池
    let config = ExecutionConfig {
        mode: ExecutionMode::Process,
        workers: 4,
        concurrency_limit: None,
        zombie_reaper_interval: None,
    };
    let pool = CommandPool::with_config(config);

    // 启动执行器
    pool.start_executor(Duration::from_millis(50));

    // 提交更多任务以获得更好的百分位数统计
    for i in 0..20 {
        let cmd = CommandConfig::new("echo", vec![format!("test_{}", i)]);
        pool.push_task(cmd).unwrap();
    }

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(3));

    // 停止执行器（在获取指标前停止）
    pool.stop();

    // 获取指标快照
    let metrics = pool.metrics();

    println!("\nMetrics with more tasks:");
    println!("  Tasks completed: {}", metrics.tasks_completed);
    println!("  P50: {:?}", metrics.p50_execution_time);
    println!("  P95: {:?}", metrics.p95_execution_time);
    println!("  P99: {:?}", metrics.p99_execution_time);

    // 验证百分位数的合理性
    if metrics.tasks_completed >= 10 {
        assert!(metrics.p50_execution_time > Duration::ZERO);
        assert!(metrics.p95_execution_time >= metrics.p50_execution_time);
        assert!(metrics.p99_execution_time >= metrics.p95_execution_time);
        // P99 可能略高于 max 由于 histogram 的插值，所以我们允许一些误差
        // 只验证它们在同一数量级
        assert!(
            metrics.p99_execution_time.as_micros() <= metrics.max_execution_time.as_micros() * 2
        );
    }
}

#[test]
fn test_metrics_success_rate() {
    // 创建命令池
    let config = ExecutionConfig {
        mode: ExecutionMode::Process,
        workers: 2,
        concurrency_limit: None,
        zombie_reaper_interval: None,
    };
    let pool = CommandPool::with_config(config);

    // 启动执行器
    pool.start_executor(Duration::from_millis(50));

    // 提交一些成功的任务
    for i in 0..5 {
        let cmd = CommandConfig::new("echo", vec![format!("success_{}", i)]);
        pool.push_task(cmd).unwrap();
    }

    // 提交一些会失败的任务
    for i in 0..3 {
        let cmd = CommandConfig::new("nonexistent_command_xyz", vec![format!("fail_{}", i)]);
        pool.push_task(cmd).unwrap();
    }

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(3));

    // 停止执行器
    pool.stop();

    // 获取指标快照
    let metrics = pool.metrics();

    println!("\nMetrics with failures:");
    println!("  Tasks submitted: {}", metrics.tasks_submitted);
    println!("  Tasks completed: {}", metrics.tasks_completed);
    println!("  Tasks failed: {}", metrics.tasks_failed);
    println!("  Success rate: {:.2}%", metrics.success_rate * 100.0);

    // 验证指标
    assert_eq!(metrics.tasks_submitted, 8);
    assert!(metrics.tasks_failed > 0, "Some tasks should fail");
    assert!(
        metrics.success_rate < 1.0,
        "Success rate should be less than 100%"
    );
}
