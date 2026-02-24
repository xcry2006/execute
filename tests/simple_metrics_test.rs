use execute::{CommandConfig, CommandPool, ExecutionConfig, ExecutionMode};
use std::time::Duration;

#[test]
fn test_metrics_basic() {
    // 创建命令池
    let config = ExecutionConfig {
        mode: ExecutionMode::Process,
        workers: 2,
        concurrency_limit: None,
        zombie_reaper_interval: None,
    };
    let pool = CommandPool::with_config(config);

    // 启动执行器
    pool.start_executor();

    // 提交一些任务
    for i in 0..3 {
        let cmd = CommandConfig::new("echo", vec![format!("test_{}", i)]);
        pool.push_task(cmd).unwrap();
    }

    // 等待任务完成
    std::thread::sleep(Duration::from_millis(500));

    // 获取指标快照
    let metrics = pool.metrics();

    // 验证指标
    println!("Metrics:");
    println!("  Submitted: {}", metrics.tasks_submitted);
    println!("  Completed: {}", metrics.tasks_completed);
    println!("  P50: {:?}", metrics.p50_execution_time);
    println!("  P95: {:?}", metrics.p95_execution_time);
    println!("  P99: {:?}", metrics.p99_execution_time);

    // 基本断言
    assert_eq!(metrics.tasks_submitted, 3);

    // 使用 shutdown 而不是 stop
    let _ = pool.shutdown_with_timeout(Duration::from_secs(5));
}
