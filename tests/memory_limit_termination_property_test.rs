// Feature: production-ready-improvements, Property 13: 内存限制终止
// **Validates: Requirement 8.4**
//
// 属性 13: 内存限制终止
// 对于任意内存使用超过限制的任务，应该被终止并返回错误
//
// 验证需求：
// - 需求 8.4: WHEN 任务内存使用超过限制时，THE System SHALL 终止任务并返回错误

use execute::{execute_command_with_context, CommandConfig, ResourceLimits};
use proptest::prelude::*;
use std::time::Duration;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意内存限制，超过限制的进程应该被终止
    ///
    /// 验证需求：
    /// - 需求 8.4: WHEN 任务内存使用超过限制时，THE System SHALL 终止任务并返回错误
    ///
    /// 注意：此测试使用一个会快速分配内存的命令来触发内存限制。
    /// 由于内存监控是异步的，测试验证进程被终止（非零退出码或错误）。
    #[test]
    #[cfg(unix)]
    fn prop_memory_limit_terminates_excessive_process(
        limit_mb in 1usize..=10,
    ) {
        // 初始化 tracing 以捕获警告日志
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        let limit = limit_mb * 1024 * 1024;

        // 创建资源限制配置
        let limits = ResourceLimits::new().with_max_memory(limit);

        // 使用 Python 创建一个会快速分配大量内存的进程
        // 分配比限制多得多的内存以确保触发限制
        let memory_to_allocate = limit * 3; // 分配 3 倍的限制
        let memory_mb = memory_to_allocate / (1024 * 1024);

        let config = CommandConfig::new(
            "python3",
            vec![
                "-c".to_string(),
                format!(
                    "import time; data = bytearray({} * 1024 * 1024); time.sleep(2)",
                    memory_mb
                ),
            ],
        )
        .with_resource_limits(limits)
        .with_timeout(Duration::from_secs(10));

        // 执行命令
        let result = execute_command_with_context(&config, 1);

        // 验证命令被终止（可能是错误或非零退出码）
        // 由于内存监控是异步的，进程可能：
        // 1. 被 SIGKILL 终止（返回错误或非零退出码）
        // 2. 在分配内存时失败（返回错误）
        if let Ok(output) = result {
            // 如果命令完成了，它应该有非零退出码（被 SIGKILL 终止）
            prop_assert!(
                !output.status.success(),
                "Process exceeding memory limit should be terminated with non-zero exit code"
            );
        } else {
            // 如果返回错误，这也是预期的（进程被终止）
            prop_assert!(
                result.is_err(),
                "Process exceeding memory limit should result in error"
            );
        }
    }

    /// 属性测试：对于任意内存限制，不超过限制的进程应该正常完成
    ///
    /// 验证需求：
    /// - 需求 8.4: 只有超过限制的任务才被终止
    #[test]
    #[cfg(unix)]
    fn prop_memory_limit_allows_normal_process(
        limit_mb in 50usize..=100,
    ) {
        // 初始化 tracing
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_test_writer()
            .try_init();

        let limit = limit_mb * 1024 * 1024;

        // 创建资源限制配置（限制足够大）
        let limits = ResourceLimits::new().with_max_memory(limit);

        // 使用一个只分配少量内存的命令
        let config = CommandConfig::new(
            "python3",
            vec![
                "-c".to_string(),
                "import time; data = bytearray(1024 * 1024); time.sleep(0.1)".to_string(),
            ],
        )
        .with_resource_limits(limits)
        .with_timeout(Duration::from_secs(5));

        // 执行命令
        let result = execute_command_with_context(&config, 1);

        // 验证命令成功执行
        prop_assert!(
            result.is_ok(),
            "Process within memory limit should execute successfully"
        );

        let output = result.unwrap();

        // 验证进程正常退出
        prop_assert!(
            output.status.success(),
            "Process within memory limit should exit with success status"
        );
    }
}

#[test]
#[cfg(unix)]
fn test_memory_limit_basic_termination() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建一个小的内存限制（5 MB）
    let limits = ResourceLimits::new().with_max_memory(5 * 1024 * 1024);

    // 使用 Python 分配大量内存（50 MB）
    let config = CommandConfig::new(
        "python3",
        vec![
            "-c".to_string(),
            "import time; data = bytearray(50 * 1024 * 1024); time.sleep(2)".to_string(),
        ],
    )
    .with_resource_limits(limits)
    .with_timeout(Duration::from_secs(10));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证进程被终止
    if let Ok(output) = result {
        // 如果命令完成了，它应该有非零退出码
        assert!(
            !output.status.success(),
            "Process should be terminated with non-zero exit code"
        );
    } else {
        // 如果返回错误，这也是预期的
        assert!(result.is_err(), "Process should be terminated with error");
    }
}

#[test]
#[cfg(unix)]
fn test_memory_limit_no_termination_for_small_usage() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建一个大的内存限制（100 MB）
    let limits = ResourceLimits::new().with_max_memory(100 * 1024 * 1024);

    // 使用 Python 分配少量内存（1 MB）
    let config = CommandConfig::new(
        "python3",
        vec![
            "-c".to_string(),
            "import time; data = bytearray(1024 * 1024); time.sleep(0.1)".to_string(),
        ],
    )
    .with_resource_limits(limits)
    .with_timeout(Duration::from_secs(5));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证成功执行
    assert!(result.is_ok(), "Command should execute successfully");

    let output = result.unwrap();

    // 验证进程正常退出
    assert!(
        output.status.success(),
        "Process should exit with success status"
    );
}

#[test]
#[cfg(unix)]
fn test_memory_limit_with_gradual_allocation() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建内存限制（10 MB）
    let limits = ResourceLimits::new().with_max_memory(10 * 1024 * 1024);

    // 使用 Python 逐步分配内存，最终超过限制
    let config = CommandConfig::new(
        "python3",
        vec![
            "-c".to_string(),
            "import time; data = []; \
             for i in range(20): \
                 data.append(bytearray(1024 * 1024)); \
                 time.sleep(0.1)"
                .to_string(),
        ],
    )
    .with_resource_limits(limits)
    .with_timeout(Duration::from_secs(10));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证进程被终止
    if let Ok(output) = result {
        assert!(
            !output.status.success(),
            "Process should be terminated when gradually exceeding memory limit"
        );
    } else {
        assert!(
            result.is_err(),
            "Process should be terminated with error when gradually exceeding memory limit"
        );
    }
}

#[test]
#[cfg(unix)]
fn test_memory_limit_with_echo_command() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建内存限制
    let limits = ResourceLimits::new().with_max_memory(50 * 1024 * 1024);

    // 使用简单的 echo 命令（不会超过内存限制）
    let config = CommandConfig::new("echo", vec!["test".to_string()])
        .with_resource_limits(limits)
        .with_timeout(Duration::from_secs(5));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证成功执行
    assert!(result.is_ok(), "Simple command should execute successfully");

    let output = result.unwrap();
    assert!(
        output.status.success(),
        "Simple command should exit successfully"
    );
}

#[test]
#[cfg(unix)]
fn test_memory_limit_boundary_case() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建内存限制（20 MB - 足够大以容纳 Python 解释器开销）
    let limits = ResourceLimits::new().with_max_memory(20 * 1024 * 1024);

    // 使用 Python 分配较小的内存（2 MB），确保不会超过限制
    let config = CommandConfig::new(
        "python3",
        vec![
            "-c".to_string(),
            "import time; data = bytearray(2 * 1024 * 1024); time.sleep(0.2)".to_string(),
        ],
    )
    .with_resource_limits(limits)
    .with_timeout(Duration::from_secs(5));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证成功执行（接近但未超过限制）
    assert!(
        result.is_ok(),
        "Command near but under memory limit should execute successfully"
    );

    let output = result.unwrap();
    assert!(
        output.status.success(),
        "Command near but under memory limit should exit successfully"
    );
}

#[test]
#[cfg(unix)]
fn test_memory_limit_with_multiple_allocations() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .try_init();

    // 创建内存限制（15 MB）
    let limits = ResourceLimits::new().with_max_memory(15 * 1024 * 1024);

    // 使用 Python 进行多次内存分配，总和超过限制
    let config = CommandConfig::new(
        "python3",
        vec![
            "-c".to_string(),
            "import time; \
             data1 = bytearray(10 * 1024 * 1024); \
             time.sleep(0.1); \
             data2 = bytearray(10 * 1024 * 1024); \
             time.sleep(1)"
                .to_string(),
        ],
    )
    .with_resource_limits(limits)
    .with_timeout(Duration::from_secs(10));

    // 执行命令
    let result = execute_command_with_context(&config, 1);

    // 验证进程被终止
    if let Ok(output) = result {
        assert!(
            !output.status.success(),
            "Process with multiple allocations exceeding limit should be terminated"
        );
    } else {
        assert!(
            result.is_err(),
            "Process with multiple allocations exceeding limit should be terminated with error"
        );
    }
}
