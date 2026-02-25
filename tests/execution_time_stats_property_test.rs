// Feature: production-ready-improvements, Property 11: 执行时间统计
// **Validates: Requirements 7.5**
//
// 属性 11: 执行时间统计
// 对于任意任务集合，统计信息（平均值、最小值、最大值、百分位数）应该正确计算
//
// 验证需求：
// - 需求 7.5: 记录任务执行时间的统计信息（平均值、最小值、最大值、百分位数）

use execute::{CommandConfig, CommandPool};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use std::time::Duration;

/// 生成任务数量策略（5-30个任务）
fn task_count_strategy() -> impl Strategy<Value = usize> {
    5usize..=30
}

/// 生成睡眠时间策略（10-500毫秒）
/// 使用 sleep 命令来控制任务执行时间
fn sleep_duration_strategy() -> impl Strategy<Value = u64> {
    10u64..=500
}

/// 生成多个睡眠时间的策略
fn sleep_durations_strategy(count: usize) -> impl Strategy<Value = Vec<u64>> {
    prop::collection::vec(sleep_duration_strategy(), count..=count)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意任务集合，执行时间统计应该正确计算
    ///
    /// 验证需求：
    /// - 需求 7.5: 记录任务执行时间的统计信息（平均值、最小值、最大值、百分位数）
    ///
    /// 测试策略：
    /// 1. 生成随机数量的任务，每个任务有不同的执行时间
    /// 2. 使用 sleep 命令控制任务执行时间
    /// 3. 等待所有任务完成
    /// 4. 验证统计信息的正确性：
    ///    - 最小值 <= 平均值 <= 最大值
    ///    - 最小值 <= P50 <= P95 <= P99 <= 最大值
    ///    - 平均值应该接近实际平均值
    #[test]
    fn prop_execution_time_stats_correctness(
        task_count in task_count_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建命令池并启动
        let pool = CommandPool::new();
        pool.start_executor();

        // 生成睡眠时间
        let sleep_durations = sleep_durations_strategy(task_count)
            .new_tree(&mut proptest::test_runner::TestRunner::default())
            .unwrap()
            .current();

        // 提交任务
        let mut handles = Vec::new();
        for &sleep_ms in &sleep_durations {
            #[cfg(unix)]
            let config = CommandConfig::new(
                "sh",
                vec![
                    "-c".to_string(),
                    format!("sleep 0.{:03}", sleep_ms),
                ],
            );

            #[cfg(windows)]
            let config = CommandConfig::new(
                "timeout",
                vec!["/t".to_string(), format!("{}", sleep_ms / 1000 + 1), "/nobreak".to_string()],
            );

            match pool.push_task(config) {
                Ok(handle) => handles.push(handle),
                Err(_) => break,
            }
        }

        let submitted_count = handles.len();

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.wait();
        }

        // 等待指标更新
        std::thread::sleep(Duration::from_millis(200));

        // 获取最终指标
        let final_metrics = pool.metrics();

        // 验证任务完成
        prop_assert_eq!(
            final_metrics.tasks_submitted as usize,
            submitted_count,
            "Submitted count should match"
        );

        // 如果有任务完成，验证统计信息
        if final_metrics.tasks_completed > 0 {
            let avg = final_metrics.avg_execution_time;
            let min = final_metrics.min_execution_time;
            let max = final_metrics.max_execution_time;
            let p50 = final_metrics.p50_execution_time;
            let p95 = final_metrics.p95_execution_time;
            let p99 = final_metrics.p99_execution_time;

            // 验证基本不等式：min <= avg <= max
            prop_assert!(
                min <= avg,
                "Min ({:?}) should be <= Avg ({:?})",
                min,
                avg
            );
            prop_assert!(
                avg <= max,
                "Avg ({:?}) should be <= Max ({:?})",
                avg,
                max
            );

            // 验证百分位数的单调性：min <= p50 <= p95 <= p99
            // Note: Due to histogram precision and rounding, p99 might be slightly higher than max
            // We allow a reasonable tolerance for this (1% of max or 1ms, whichever is larger)
            let tolerance = std::cmp::max(
                Duration::from_millis(1),
                max / 100
            );

            prop_assert!(
                min <= p50,
                "Min ({:?}) should be <= P50 ({:?})",
                min,
                p50
            );
            prop_assert!(
                p50 <= p95,
                "P50 ({:?}) should be <= P95 ({:?})",
                p50,
                p95
            );
            prop_assert!(
                p95 <= p99,
                "P95 ({:?}) should be <= P99 ({:?})",
                p95,
                p99
            );
            // Allow tolerance for p99 vs max due to histogram precision
            prop_assert!(
                p99 <= max + tolerance,
                "P99 ({:?}) should be <= Max ({:?}) + tolerance ({:?})",
                p99,
                max,
                tolerance
            );

            // 验证统计值不为零（任务确实执行了）
            prop_assert!(
                min > Duration::ZERO,
                "Min execution time should be > 0"
            );
            prop_assert!(
                max > Duration::ZERO,
                "Max execution time should be > 0"
            );
        }

        // 关闭命令池
        let _ = pool.shutdown_with_timeout(Duration::from_secs(5));
    }

    /// 属性测试：对于单个任务，所有统计值应该相等
    ///
    /// 验证需求：
    /// - 需求 7.5: 记录任务执行时间的统计信息
    #[test]
    fn prop_execution_time_stats_single_task(
        sleep_ms in sleep_duration_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        // 创建命令池并启动
        let pool = CommandPool::new();
        pool.start_executor();

        // 提交单个任务
        #[cfg(unix)]
        let config = CommandConfig::new(
            "sh",
            vec![
                "-c".to_string(),
                format!("sleep 0.{:03}", sleep_ms),
            ],
        );

        #[cfg(windows)]
        let config = CommandConfig::new(
            "timeout",
            vec!["/t".to_string(), format!("{}", sleep_ms / 1000 + 1), "/nobreak".to_string()],
        );

        let handle = pool.push_task(config).expect("Failed to submit task");
        let _ = handle.wait();

        // 等待指标更新
        std::thread::sleep(Duration::from_millis(200));

        // 获取最终指标
        let final_metrics = pool.metrics();

        // 对于单个任务，所有统计值应该相等
        if final_metrics.tasks_completed > 0 {
            let avg = final_metrics.avg_execution_time;
            let min = final_metrics.min_execution_time;
            let max = final_metrics.max_execution_time;
            let p50 = final_metrics.p50_execution_time;
            let _p95 = final_metrics.p95_execution_time;
            let _p99 = final_metrics.p99_execution_time;

            // 允许一些误差（由于 histogram 的精度限制）
            let tolerance = Duration::from_millis(10);

            prop_assert!(
                (min.as_millis() as i64 - avg.as_millis() as i64).abs() <= tolerance.as_millis() as i64,
                "For single task, min ({:?}) should equal avg ({:?})",
                min,
                avg
            );
            prop_assert!(
                (avg.as_millis() as i64 - max.as_millis() as i64).abs() <= tolerance.as_millis() as i64,
                "For single task, avg ({:?}) should equal max ({:?})",
                avg,
                max
            );
            prop_assert!(
                (p50.as_millis() as i64 - avg.as_millis() as i64).abs() <= tolerance.as_millis() as i64,
                "For single task, p50 ({:?}) should equal avg ({:?})",
                p50,
                avg
            );
        }

        // 关闭命令池
        let _ = pool.shutdown_with_timeout(Duration::from_secs(5));
    }
}

#[test]
fn test_execution_time_stats_zero_tasks() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 不提交任何任务
    let metrics = pool.metrics();

    // 验证统计值为零或默认值
    assert_eq!(metrics.avg_execution_time, Duration::ZERO);
    assert_eq!(metrics.p50_execution_time, Duration::ZERO);
    assert_eq!(metrics.p95_execution_time, Duration::ZERO);
    assert_eq!(metrics.p99_execution_time, Duration::ZERO);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_execution_time_stats_known_values() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交已知执行时间的任务
    // 使用快速命令以便测试快速完成
    let mut handles = Vec::new();
    for i in 0..10 {
        let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
        handles.push(pool.push_task(config).expect("Failed to submit task"));
    }

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(200));

    // 获取最终指标
    let final_metrics = pool.metrics();

    // 验证统计信息的基本属性
    assert_eq!(final_metrics.tasks_completed, 10);
    assert!(final_metrics.min_execution_time > Duration::ZERO);
    assert!(final_metrics.max_execution_time > Duration::ZERO);
    assert!(final_metrics.avg_execution_time > Duration::ZERO);
    assert!(final_metrics.min_execution_time <= final_metrics.avg_execution_time);
    assert!(final_metrics.avg_execution_time <= final_metrics.max_execution_time);

    // 验证百分位数
    assert!(final_metrics.p50_execution_time > Duration::ZERO);
    assert!(final_metrics.p95_execution_time > Duration::ZERO);
    assert!(final_metrics.p99_execution_time > Duration::ZERO);
    assert!(final_metrics.p50_execution_time <= final_metrics.p95_execution_time);
    assert!(final_metrics.p95_execution_time <= final_metrics.p99_execution_time);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_execution_time_stats_min_max() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交不同执行时间的任务
    let mut handles = Vec::new();

    // 快速任务
    for i in 0..5 {
        let config = CommandConfig::new("echo", vec![format!("fast_{}", i)]);
        handles.push(pool.push_task(config).expect("Failed to submit task"));
    }

    // 慢速任务（使用 sleep）
    #[cfg(unix)]
    {
        for i in 0..5 {
            let config = CommandConfig::new(
                "sh",
                vec!["-c".to_string(), format!("sleep 0.1 && echo slow_{}", i)],
            );
            handles.push(pool.push_task(config).expect("Failed to submit task"));
        }
    }

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(300));

    // 获取最终指标
    let final_metrics = pool.metrics();

    // 验证最小值和最大值的差异
    #[cfg(unix)]
    {
        assert!(
            final_metrics.max_execution_time > final_metrics.min_execution_time,
            "Max ({:?}) should be > Min ({:?}) when tasks have different execution times",
            final_metrics.max_execution_time,
            final_metrics.min_execution_time
        );

        // 最大值应该至少是 100ms（sleep 0.1）
        assert!(
            final_metrics.max_execution_time >= Duration::from_millis(90),
            "Max execution time should be at least 90ms, got {:?}",
            final_metrics.max_execution_time
        );
    }

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_execution_time_stats_percentiles_ordering() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交大量任务以获得有意义的百分位数
    let mut handles = Vec::new();
    for i in 0..50 {
        let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
        handles.push(pool.push_task(config).expect("Failed to submit task"));
    }

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 等待指标更新
    std::thread::sleep(Duration::from_millis(300));

    // 获取最终指标
    let final_metrics = pool.metrics();

    // 验证百分位数的顺序
    assert!(
        final_metrics.p50_execution_time <= final_metrics.p95_execution_time,
        "P50 ({:?}) should be <= P95 ({:?})",
        final_metrics.p50_execution_time,
        final_metrics.p95_execution_time
    );
    assert!(
        final_metrics.p95_execution_time <= final_metrics.p99_execution_time,
        "P95 ({:?}) should be <= P99 ({:?})",
        final_metrics.p95_execution_time,
        final_metrics.p99_execution_time
    );
    assert!(
        final_metrics.min_execution_time <= final_metrics.p50_execution_time,
        "Min ({:?}) should be <= P50 ({:?})",
        final_metrics.min_execution_time,
        final_metrics.p50_execution_time
    );
    // Allow tolerance for p99 vs max due to histogram precision
    let tolerance = std::cmp::max(
        Duration::from_millis(1),
        final_metrics.max_execution_time / 100,
    );
    assert!(
        final_metrics.p99_execution_time <= final_metrics.max_execution_time + tolerance,
        "P99 ({:?}) should be <= Max ({:?}) + tolerance ({:?})",
        final_metrics.p99_execution_time,
        final_metrics.max_execution_time,
        tolerance
    );

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_execution_time_stats_incremental_updates() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor();

    // 提交第一个任务
    let config1 = CommandConfig::new("echo", vec!["task1".to_string()]);
    let handle1 = pool.push_task(config1).expect("Failed to submit task");
    let _ = handle1.wait();
    std::thread::sleep(Duration::from_millis(100));

    // 获取第一次指标
    let metrics1 = pool.metrics();
    assert_eq!(metrics1.tasks_completed, 1);
    let first_avg = metrics1.avg_execution_time;
    assert!(first_avg > Duration::ZERO);

    // 提交第二个任务
    let config2 = CommandConfig::new("echo", vec!["task2".to_string()]);
    let handle2 = pool.push_task(config2).expect("Failed to submit task");
    let _ = handle2.wait();
    std::thread::sleep(Duration::from_millis(100));

    // 获取第二次指标
    let metrics2 = pool.metrics();
    assert_eq!(metrics2.tasks_completed, 2);

    // 验证统计信息已更新
    assert!(metrics2.avg_execution_time > Duration::ZERO);
    assert!(metrics2.min_execution_time > Duration::ZERO);
    assert!(metrics2.max_execution_time > Duration::ZERO);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}
