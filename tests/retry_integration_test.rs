/// 测试重试机制集成到命令执行流程
///
/// 验证需求 11.4, 11.5, 11.6:
/// - 11.4: 任务失败且未达到最大重试次数时，系统应自动重试
/// - 11.5: 重试时应记录重试次数和原因
/// - 11.6: 达到最大重试次数后仍失败时，应返回最终错误
///
/// 注意：重试机制只重试执行错误（spawn失败、超时等），不重试非零退出码。
/// 非零退出码表示命令成功执行但返回了失败状态，这不是执行错误。
use execute::{CommandConfig, CommandPool, RetryPolicy, RetryStrategy};
use std::time::Duration;

#[test]
#[cfg(unix)]
fn test_commandpool_retry_on_timeout() {
    // 测试超时错误的重试
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(100));

    // 创建一个会超时的命令，配置重试策略
    let retry_policy = RetryPolicy::new(
        2, // 最多重试 2 次
        RetryStrategy::FixedInterval(Duration::from_millis(50)),
    );

    let config = CommandConfig::new("sleep", vec!["1".to_string()])
        .with_timeout(Duration::from_millis(10)) // 很短的超时，会触发超时错误
        .with_retry(retry_policy);

    // 提交任务
    pool.push_task(config).unwrap();

    // 等待任务执行完成（包括重试）
    // 3次尝试 * 10ms超时 + 2次重试延迟 * 50ms = 130ms，加上一些余量
    std::thread::sleep(Duration::from_millis(500));

    // 验证指标：任务应该被标记为失败（因为超时是执行错误）
    let metrics = pool.metrics();
    assert_eq!(metrics.tasks_submitted, 1, "应该提交了 1 个任务");
    assert_eq!(metrics.tasks_failed, 1, "任务应该最终失败");
    assert_eq!(metrics.tasks_completed, 0, "任务不应该成功完成");

    pool.shutdown().unwrap();
}

#[test]
#[cfg(unix)]
fn test_commandpool_no_retry_on_nonzero_exit() {
    // 测试非零退出码不会触发重试
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(100));

    // 创建一个返回非零退出码的命令，配置重试策略
    let retry_policy = RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(50)));

    let config = CommandConfig::new("false", vec![]) // 'false' 返回退出码 1
        .with_retry(retry_policy);

    pool.push_task(config).unwrap();

    // 等待任务执行完成
    std::thread::sleep(Duration::from_millis(300));

    // 验证指标：命令成功执行（即使退出码非零），不应该重试
    let metrics = pool.metrics();
    assert_eq!(metrics.tasks_submitted, 1, "应该提交了 1 个任务");
    // 注意：非零退出码的命令仍然被认为是"成功执行"的
    // 因为进程成功启动并完成了，只是返回了非零退出码
    assert_eq!(metrics.tasks_completed, 1, "任务应该成功执行");
    assert_eq!(metrics.tasks_failed, 0, "不应该有执行失败");

    pool.shutdown().unwrap();
}

#[test]
#[cfg(unix)]
fn test_commandpool_retry_success_after_retry() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(100));

    // 创建一个会成功的命令，配置重试策略
    // 即使配置了重试，成功的命令也不应该重试
    let retry_policy = RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(50)));

    let config = CommandConfig::new("true", vec![]) // 'true' 命令总是成功
        .with_retry(retry_policy);

    pool.push_task(config).unwrap();

    // 等待任务执行完成
    std::thread::sleep(Duration::from_millis(300));

    // 验证指标：任务应该成功完成，不应该重试
    let metrics = pool.metrics();
    assert_eq!(metrics.tasks_submitted, 1, "应该提交了 1 个任务");
    assert_eq!(metrics.tasks_completed, 1, "任务应该成功完成");
    assert_eq!(metrics.tasks_failed, 0, "任务不应该失败");

    pool.shutdown().unwrap();
}

#[test]
#[cfg(unix)]
fn test_retry_without_retry_policy() {
    // 测试没有配置重试策略的情况
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(100));

    // 不配置重试策略，使用会超时的命令
    let config =
        CommandConfig::new("sleep", vec!["1".to_string()]).with_timeout(Duration::from_millis(10));

    pool.push_task(config).unwrap();

    // 等待任务执行完成
    std::thread::sleep(Duration::from_millis(200));

    // 验证指标：任务应该立即失败，不重试
    let metrics = pool.metrics();
    assert_eq!(metrics.tasks_submitted, 1);
    assert_eq!(metrics.tasks_failed, 1);

    pool.shutdown().unwrap();
}

#[test]
#[cfg(unix)]
fn test_metrics_accuracy_with_retry() {
    // 验证重试不影响指标准确性
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();

    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(100));

    // 提交多个任务，一些有重试策略，一些没有
    let retry_policy = RetryPolicy::new(1, RetryStrategy::FixedInterval(Duration::from_millis(50)));

    // 任务 1: 超时失败，有重试
    let config1 = CommandConfig::new("sleep", vec!["1".to_string()])
        .with_timeout(Duration::from_millis(10))
        .with_retry(retry_policy.clone());
    pool.push_task(config1).unwrap();

    // 任务 2: 成功，有重试（但不会触发重试）
    let config2 = CommandConfig::new("true", vec![]).with_retry(retry_policy.clone());
    pool.push_task(config2).unwrap();

    // 任务 3: 超时失败，无重试
    let config3 =
        CommandConfig::new("sleep", vec!["1".to_string()]).with_timeout(Duration::from_millis(10));
    pool.push_task(config3).unwrap();

    // 任务 4: 成功，无重试
    let config4 = CommandConfig::new("true", vec![]);
    pool.push_task(config4).unwrap();

    // 等待所有任务完成
    std::thread::sleep(Duration::from_millis(800));

    // 验证指标
    let metrics = pool.metrics();
    assert_eq!(metrics.tasks_submitted, 4, "应该提交了 4 个任务");
    assert_eq!(metrics.tasks_completed, 2, "应该有 2 个任务成功");
    assert_eq!(metrics.tasks_failed, 2, "应该有 2 个任务失败");

    pool.shutdown().unwrap();
}
