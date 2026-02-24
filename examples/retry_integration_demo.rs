/// 演示重试机制集成到命令执行流程
///
/// 此示例展示了如何在 CommandPool 中使用重试机制。
/// 重试机制会自动重试执行错误（如超时、spawn失败），但不会重试非零退出码。
use execute::{CommandConfig, CommandPool, RetryPolicy, RetryStrategy};
use std::time::Duration;

fn main() {
    // 初始化日志系统
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== 重试机制集成演示 ===\n");

    // 示例 1: CommandPool 中使用固定间隔重试策略
    demo_commandpool_fixed_interval_retry();

    // 示例 2: CommandPool 中使用指数退避重试策略
    demo_commandpool_exponential_backoff_retry();

    // 示例 3: 重试不影响指标准确性
    demo_metrics_accuracy_with_retry();

    println!("\n=== 演示完成 ===");
}

fn demo_commandpool_fixed_interval_retry() {
    println!("1. CommandPool - 固定间隔重试策略");
    println!("   配置: 最多重试 2 次，每次间隔 100ms");

    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(50));

    // 创建会超时的命令，配置固定间隔重试
    let retry_policy = RetryPolicy::new(
        2, // 最多重试 2 次
        RetryStrategy::FixedInterval(Duration::from_millis(100)),
    );

    let config = CommandConfig::new("sleep", vec!["2".to_string()])
        .with_timeout(Duration::from_millis(50)) // 50ms 超时
        .with_retry(retry_policy);

    println!("   提交任务: sleep 2 (超时 50ms)");
    pool.push_task(config).unwrap();

    // 等待任务完成（包括重试）
    // 3次尝试 * 50ms + 2次重试间隔 * 100ms = 350ms
    std::thread::sleep(Duration::from_millis(500));

    let metrics = pool.metrics();
    println!(
        "   结果: 提交={}, 失败={}",
        metrics.tasks_submitted, metrics.tasks_failed
    );
    println!("   说明: 任务会重试 2 次，每次都超时，最终失败\n");

    pool.shutdown().unwrap();
}

fn demo_commandpool_exponential_backoff_retry() {
    println!("2. CommandPool - 指数退避重试策略");
    println!("   配置: 最多重试 3 次，初始间隔 50ms，最大 500ms，倍数 2.0");

    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(50));

    // 创建会超时的命令，配置指数退避重试
    let retry_policy = RetryPolicy::new(
        3, // 最多重试 3 次
        RetryStrategy::ExponentialBackoff {
            initial: Duration::from_millis(50),
            max: Duration::from_millis(500),
            multiplier: 2.0,
        },
    );

    let config = CommandConfig::new("sleep", vec!["2".to_string()])
        .with_timeout(Duration::from_millis(30))
        .with_retry(retry_policy);

    println!("   提交任务: sleep 2 (超时 30ms)");
    pool.push_task(config).unwrap();

    // 等待任务完成
    // 重试间隔: 50ms, 100ms, 200ms
    std::thread::sleep(Duration::from_millis(600));

    let metrics = pool.metrics();
    println!(
        "   结果: 提交={}, 失败={}",
        metrics.tasks_submitted, metrics.tasks_failed
    );
    println!("   说明: 重试间隔逐渐增加 (50ms -> 100ms -> 200ms)\n");

    pool.shutdown().unwrap();
}

fn demo_metrics_accuracy_with_retry() {
    println!("3. 重试不影响指标准确性");
    println!("   验证: 即使有重试，指标仍然准确记录任务数量");

    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(50));

    let retry_policy = RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(50)));

    // 任务 1: 会超时并重试的任务
    let config1 = CommandConfig::new("sleep", vec!["2".to_string()])
        .with_timeout(Duration::from_millis(30))
        .with_retry(retry_policy.clone());
    pool.push_task(config1).unwrap();

    // 任务 2: 成功的任务（有重试配置但不会触发）
    let config2 =
        CommandConfig::new("echo", vec!["hello".to_string()]).with_retry(retry_policy.clone());
    pool.push_task(config2).unwrap();

    // 任务 3: 会超时的任务（无重试）
    let config3 =
        CommandConfig::new("sleep", vec!["2".to_string()]).with_timeout(Duration::from_millis(30));
    pool.push_task(config3).unwrap();

    // 任务 4: 成功的任务（无重试）
    let config4 = CommandConfig::new("echo", vec!["world".to_string()]);
    pool.push_task(config4).unwrap();

    println!("   提交了 4 个任务:");
    println!("   - 任务 1: 超时 + 重试 (会失败)");
    println!("   - 任务 2: 成功 + 重试配置 (不触发重试)");
    println!("   - 任务 3: 超时 + 无重试 (会失败)");
    println!("   - 任务 4: 成功 + 无重试");

    // 等待所有任务完成
    std::thread::sleep(Duration::from_millis(800));

    let metrics = pool.metrics();
    println!("\n   指标结果:");
    println!("   - 提交任务数: {}", metrics.tasks_submitted);
    println!("   - 成功任务数: {}", metrics.tasks_completed);
    println!("   - 失败任务数: {}", metrics.tasks_failed);
    println!("   - 成功率: {:.1}%", metrics.success_rate * 100.0);
    println!("\n   说明: 重试只影响单个任务的执行过程，不影响任务计数");

    pool.shutdown().unwrap();
}
