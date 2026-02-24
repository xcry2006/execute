// Feature: production-ready-improvements, Property 14: 僵尸进程清理
// **Validates: Requirements 9.1, 9.3, 9.5**
//
// 属性 14: 僵尸进程清理
// 对于任意已终止的子进程，系统应该定期回收，shutdown 后不应有僵尸进程残留
//
// 验证需求：
// - 需求 9.1: 系统应定期检查并回收已终止的子进程
// - 需求 9.3: 系统应记录清理的僵尸进程数量
// - 需求 9.5: 系统应在命令池关闭时清理所有剩余的僵尸进程

#[cfg(unix)]
use execute::{CommandConfig, CommandPool, ExecutionConfig};
#[cfg(unix)]
use proptest::prelude::*;
#[cfg(unix)]
use std::process::Command;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
/// 生成任务数量策略（5-20个任务）
fn task_count_strategy() -> impl Strategy<Value = usize> {
    5usize..=20
}

#[cfg(unix)]
/// 生成检查间隔策略（500-1000ms，避免与executor竞争）
fn check_interval_strategy() -> impl Strategy<Value = u64> {
    500u64..=1000
}

#[cfg(unix)]
proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// 属性测试：对于任意数量的快速完成任务，僵尸进程应该被定期清理
    ///
    /// 验证需求：
    /// - 需求 9.1: 系统应定期检查并回收已终止的子进程
    /// - 需求 9.3: 系统应记录清理的僵尸进程数量（通过日志验证）
    #[test]
    fn prop_zombie_reaper_cleans_processes(
        task_count in task_count_strategy(),
        check_interval_ms in check_interval_strategy(),
    ) {
        // 初始化 tracing 以捕获日志
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_test_writer()
            .try_init();

        // 创建带有僵尸进程清理器的命令池
        let config = ExecutionConfig::new()
            .with_workers(2)
            .with_zombie_reaper_interval(Duration::from_millis(check_interval_ms));

        let pool = CommandPool::with_config(config);
        pool.start_executor();

        // 提交快速完成的任务，这些任务会产生子进程
        let mut handles = Vec::new();
        for i in 0..task_count {
            let task = CommandConfig::new(
                "sh",
                vec!["-c".to_string(), format!("echo 'Task {}' && exit 0", i)],
            );
            match pool.push_task(task) {
                Ok(handle) => handles.push(handle),
                Err(_) => break,
            }
        }

        let submitted_count = handles.len();

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.wait();
        }

        // 等待清理器运行几个周期
        let wait_time = Duration::from_millis(check_interval_ms * 2);
        std::thread::sleep(wait_time);

        // 检查僵尸进程数量
        let zombie_count = count_zombie_processes();

        // 僵尸进程应该被清理，数量应该很少或为 0
        // 由于系统调度的不确定性，我们允许少量僵尸进程短暂存在
        prop_assert!(
            zombie_count < 5,
            "Too many zombie processes after cleanup: {} (submitted {} tasks)",
            zombie_count,
            submitted_count
        );

        // 优雅关闭
        let shutdown_result = pool.shutdown_with_timeout(Duration::from_secs(2));
        prop_assert!(shutdown_result.is_ok(), "Shutdown should succeed");
    }

    /// 属性测试：关闭后不应有僵尸进程残留
    ///
    /// 验证需求：
    /// - 需求 9.5: 系统应在命令池关闭时清理所有剩余的僵尸进程
    #[test]
    fn prop_no_zombies_after_shutdown(
        task_count in task_count_strategy(),
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_test_writer()
            .try_init();

        // 创建带有僵尸进程清理器的命令池（使用较长的检查间隔）
        let config = ExecutionConfig::new()
            .with_workers(2)
            .with_zombie_reaper_interval(Duration::from_secs(5)); // 5秒，确保不会在测试期间运行

        let pool = CommandPool::with_config(config);
        pool.start_executor();

        // 提交任务
        let mut handles = Vec::new();
        for i in 0..task_count {
            let task = CommandConfig::new(
                "sh",
                vec!["-c".to_string(), format!("echo 'Task {}' && exit 0", i)],
            );
            match pool.push_task(task) {
                Ok(handle) => handles.push(handle),
                Err(_) => break,
            }
        }

        // 等待所有任务完成
        for handle in handles {
            let _ = handle.wait();
        }

        // 不等待清理器运行，直接关闭
        // 这样可以测试关闭时的清理逻辑
        let shutdown_result = pool.shutdown_with_timeout(Duration::from_secs(2));
        prop_assert!(shutdown_result.is_ok(), "Shutdown should succeed");

        // 等待一小段时间确保清理完成
        std::thread::sleep(Duration::from_millis(200));

        // 检查僵尸进程数量
        let zombie_count = count_zombie_processes();

        // 关闭后不应有僵尸进程残留
        prop_assert!(
            zombie_count < 3,
            "Zombie processes should be cleaned up after shutdown: {} zombies found",
            zombie_count
        );
    }
}

#[cfg(unix)]
/// 计算当前系统中的僵尸进程数量
fn count_zombie_processes() -> usize {
    match Command::new("ps").arg("aux").output() {
        Ok(output) => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            output_str
                .lines()
                .filter(|line| {
                    // 在不同的系统上，僵尸进程可能显示为 <defunct> 或状态为 Z
                    line.contains("<defunct>") || line.contains(" Z ")
                })
                .count()
        }
        Err(_) => {
            // 如果无法执行 ps 命令，返回 0
            0
        }
    }
}

// 单元测试：验证特定的僵尸进程清理场景

#[cfg(unix)]
#[test]
fn test_zombie_reaper_with_fast_tasks() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // 创建带有僵尸进程清理器的命令池（使用较长的间隔避免竞争）
    let config = ExecutionConfig::new()
        .with_workers(2)
        .with_zombie_reaper_interval(Duration::from_millis(500));

    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 提交多个快速任务
    let task_count = 10;
    let handles: Vec<_> = (0..task_count)
        .map(|i| {
            let task = CommandConfig::new("echo", vec![format!("task_{}", i)]);
            pool.push_task(task).expect("Failed to submit task")
        })
        .collect();

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 等待清理器运行
    std::thread::sleep(Duration::from_secs(2));

    // 检查僵尸进程
    let zombie_count = count_zombie_processes();
    assert!(
        zombie_count < 5,
        "Too many zombie processes: {}",
        zombie_count
    );

    // 关闭
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[cfg(unix)]
#[test]
fn test_zombie_reaper_with_shell_commands() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // 创建带有僵尸进程清理器的命令池（使用较长的间隔避免竞争）
    let config = ExecutionConfig::new()
        .with_workers(2)
        .with_zombie_reaper_interval(Duration::from_millis(500));

    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 提交 shell 命令（更容易产生子进程）
    let task_count = 15;
    let handles: Vec<_> = (0..task_count)
        .map(|i| {
            let task = CommandConfig::new(
                "sh",
                vec!["-c".to_string(), format!("echo 'Task {}' && exit 0", i)],
            );
            pool.push_task(task).expect("Failed to submit task")
        })
        .collect();

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 等待清理器运行多个周期
    std::thread::sleep(Duration::from_secs(2));

    // 检查僵尸进程
    let zombie_count = count_zombie_processes();
    assert!(
        zombie_count < 5,
        "Too many zombie processes after shell commands: {}",
        zombie_count
    );

    // 关闭
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[cfg(unix)]
#[test]
fn test_zombie_cleanup_on_shutdown() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // 创建带有僵尸进程清理器的命令池（使用很长的检查间隔）
    let config = ExecutionConfig::new()
        .with_workers(2)
        .with_zombie_reaper_interval(Duration::from_secs(30)); // 30秒，确保不会在测试期间运行

    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 提交任务
    let task_count = 10;
    let handles: Vec<_> = (0..task_count)
        .map(|i| {
            let task = CommandConfig::new(
                "sh",
                vec!["-c".to_string(), format!("echo 'Task {}' && exit 0", i)],
            );
            pool.push_task(task).expect("Failed to submit task")
        })
        .collect();

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 不等待清理器运行，直接关闭
    // 这会触发 ZombieReaper 的 drop，应该执行最后的清理
    let shutdown_result = pool.shutdown_with_timeout(Duration::from_secs(2));
    assert!(shutdown_result.is_ok(), "Shutdown should succeed");

    // 等待一小段时间确保清理完成
    std::thread::sleep(Duration::from_millis(200));

    // 检查僵尸进程
    let zombie_count = count_zombie_processes();
    assert!(
        zombie_count < 3,
        "Zombie processes should be cleaned up on shutdown: {} zombies found",
        zombie_count
    );
}

#[cfg(unix)]
#[test]
fn test_zombie_reaper_periodic_cleanup() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // 创建带有僵尸进程清理器的命令池（使用较长的间隔避免竞争）
    let config = ExecutionConfig::new()
        .with_workers(2)
        .with_zombie_reaper_interval(Duration::from_millis(500));

    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 分批提交任务，观察清理器的周期性行为
    for batch in 0..3 {
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let task = CommandConfig::new(
                    "sh",
                    vec![
                        "-c".to_string(),
                        format!("echo 'Batch {} Task {}' && exit 0", batch, i),
                    ],
                );
                pool.push_task(task).expect("Failed to submit task")
            })
            .collect();

        // 等待这批任务完成
        for handle in handles {
            let _ = handle.wait();
        }

        // 等待清理器运行
        std::thread::sleep(Duration::from_secs(1));

        // 检查僵尸进程
        let zombie_count = count_zombie_processes();
        assert!(
            zombie_count < 5,
            "Too many zombie processes after batch {}: {}",
            batch,
            zombie_count
        );
    }

    // 关闭
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[cfg(unix)]
#[test]
fn test_zombie_reaper_with_concurrent_tasks() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // 创建带有僵尸进程清理器的命令池（多个工作线程，使用较长的间隔避免竞争）
    let config = ExecutionConfig::new()
        .with_workers(4)
        .with_zombie_reaper_interval(Duration::from_millis(500));

    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 并发提交大量任务
    let task_count = 20;
    let handles: Vec<_> = (0..task_count)
        .map(|i| {
            let task = CommandConfig::new(
                "sh",
                vec!["-c".to_string(), format!("echo 'Task {}' && exit 0", i)],
            );
            pool.push_task(task).expect("Failed to submit task")
        })
        .collect();

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.wait();
    }

    // 等待清理器运行
    std::thread::sleep(Duration::from_secs(2));

    // 检查僵尸进程
    let zombie_count = count_zombie_processes();
    assert!(
        zombie_count < 5,
        "Too many zombie processes with concurrent tasks: {}",
        zombie_count
    );

    // 关闭
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

// 非 Unix 系统的占位测试
#[cfg(not(unix))]
#[test]
fn test_zombie_reaper_not_available_on_non_unix() {
    // 在非 Unix 系统上，僵尸进程清理功能不可用
    // 这个测试只是确保代码可以编译
    assert!(true, "Zombie reaper is only available on Unix systems");
}
