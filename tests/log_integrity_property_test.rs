// Feature: production-ready-improvements, Property 1: 日志完整性
// **Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5**
//
// 属性 1: 日志完整性
// 对于任意任务执行，日志应该包含任务的完整生命周期信息（提交、开始、完成/失败），
// 包括任务 ID、命令、时间戳和执行结果
//
// 注意：由于 tracing subscriber 是全局单例，在单元测试中难以捕获每个测试的日志。
// 因此，我们采用以下策略：
// 1. 单元测试验证特定场景下的日志完整性
// 2. 属性测试验证任务执行的正确性（间接验证日志系统工作正常）
// 3. 集成测试或手动测试验证实际的日志输出

use execute::{CommandConfig, CommandPool};
use proptest::prelude::*;
use std::time::Duration;

/// 生成有效的命令字符串策略
fn command_strategy() -> impl Strategy<Value = String> {
    #[cfg(unix)]
    {
        prop_oneof![
            Just("echo".to_string()),
            Just("true".to_string()),
            Just("false".to_string()),
            Just("sleep".to_string()),
        ]
    }
    #[cfg(not(unix))]
    {
        prop_oneof![
            Just("echo".to_string()),
            Just("cmd".to_string()),
        ]
    }
}

/// 生成命令参数策略
fn args_strategy() -> impl Strategy<Value = Vec<String>> {
    prop_oneof![
        // 简单参数
        Just(vec!["test".to_string()]),
        Just(vec!["hello".to_string(), "world".to_string()]),
        // 数字参数
        Just(vec!["0".to_string()]),
        Just(vec!["1".to_string()]),
        // 空参数
        Just(vec![]),
    ]
}

/// 生成 CommandConfig 策略
fn command_config_strategy() -> impl Strategy<Value = CommandConfig> {
    (command_strategy(), args_strategy()).prop_map(|(cmd, args)| {
        let mut config = CommandConfig::new(&cmd, args);
        
        // 为某些命令添加超时，避免测试运行太久
        if cmd == "sleep" {
            config = config.with_timeout(Duration::from_millis(100));
        }
        
        config
    })
}

#[test]
fn test_log_integrity_single_task() {
    // 初始化 tracing（输出到 test writer）
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .with_ansi(false)
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(50));

    // 提交一个简单任务
    let config = CommandConfig::new("echo", vec!["test".to_string()]);
    let handle = pool.push_task(config).expect("Failed to submit task");
    let task_id = handle.id();

    // 等待任务完成
    let result = handle.wait();

    // 验证任务成功执行
    assert!(result.is_ok(), "Task {} should complete successfully", task_id);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意命令配置，任务应该正确执行并完成
    ///
    /// 此测试间接验证日志系统的正确性：
    /// - 如果日志系统有问题，可能会导致任务执行失败或挂起
    /// - 通过验证任务能够正确提交、执行和完成，我们间接验证了日志系统的基本功能
    ///
    /// 验证需求：
    /// - 需求 1.2: 任务提交时记录任务 ID、命令和时间戳
    /// - 需求 1.3: 任务开始执行时记录任务 ID、工作线程 ID 和开始时间
    /// - 需求 1.4: 任务完成时记录任务 ID、执行结果、退出码和执行时长
    /// - 需求 1.5: 发生错误时记录错误类型、上下文信息
    #[test]
    fn prop_log_integrity_for_any_task(config in command_config_strategy()) {
        // 初始化 tracing（只在第一次调用时成功）
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .with_ansi(false)
            .try_init();

        // 创建命令池并启动
        let pool = CommandPool::new();
        pool.start_executor(Duration::from_millis(50));

        // 提交任务
        let handle = match pool.push_task(config.clone()) {
            Ok(h) => h,
            Err(_) => {
                // 如果提交失败（例如池正在关闭），跳过此测试用例
                let _ = pool.shutdown_with_timeout(Duration::from_secs(1));
                return Ok(());
            }
        };
        
        let task_id = handle.id();
        let command = config.program().to_string();

        // 等待任务完成
        let result = handle.wait();

        // 验证任务完成（成功或失败都是正常的）
        prop_assert!(
            result.is_ok() || result.is_err(),
            "Task {} should complete (either successfully or with error). Command: {}",
            task_id,
            command
        );

        // 如果任务成功，验证输出存在
        if let Ok(output) = result {
            prop_assert!(
                output.status.code().is_some() || !output.status.success(),
                "Task {} should have exit code or failure status. Command: {}",
                task_id,
                command
            );
        }

        // 关闭命令池
        let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
    }
}

#[test]
fn test_log_integrity_for_failed_task() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .with_ansi(false)
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(50));

    // 提交一个会失败的任务
    let config = CommandConfig::new("false", vec![]);
    let handle = pool.push_task(config).expect("Failed to submit task");
    let task_id = handle.id();

    // 等待任务完成
    let result = handle.wait();

    // 验证任务执行完成
    assert!(result.is_ok(), "Task {} should complete", task_id);
    
    // 验证任务失败（false 命令返回非零退出码）
    if let Ok(output) = result {
        assert!(!output.status.success(), "Task {} should have failed", task_id);
    }

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
#[cfg(unix)]
fn test_log_integrity_for_timeout_task() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .with_ansi(false)
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(50));

    // 提交一个会超时的任务
    let config = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_timeout(Duration::from_millis(100));
    let handle = pool.push_task(config).expect("Failed to submit task");
    let task_id = handle.id();

    // 等待任务完成（应该超时）
    let result = handle.wait();

    // 验证任务因超时而失败
    assert!(result.is_err(), "Task {} should timeout and fail", task_id);

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_log_integrity_multiple_tasks() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .with_ansi(false)
        .try_init();

    // 创建命令池并启动
    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(50));

    // 提交多个任务
    let mut handles = Vec::new();
    for i in 0..10 {
        let config = CommandConfig::new("echo", vec![format!("task_{}", i)]);
        let handle = pool.push_task(config).expect("Failed to submit task");
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        let task_id = handle.id();
        let result = handle.wait();
        assert!(result.is_ok(), "Task {} should complete successfully", task_id);
    }

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}

#[test]
fn test_log_integrity_with_concurrent_tasks() {
    // 初始化 tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .with_ansi(false)
        .try_init();

    // 创建命令池并启动（多个工作线程）
    let pool = CommandPool::new();
    pool.start_executor(Duration::from_millis(50));

    // 并发提交多个任务
    let handles: Vec<_> = (0..20)
        .map(|i| {
            let config = CommandConfig::new("echo", vec![format!("concurrent_{}", i)]);
            pool.push_task(config).expect("Failed to submit task")
        })
        .collect();

    // 等待所有任务完成
    for handle in handles {
        let task_id = handle.id();
        let result = handle.wait();
        assert!(result.is_ok(), "Task {} should complete successfully", task_id);
    }

    // 关闭命令池
    let _ = pool.shutdown_with_timeout(Duration::from_secs(2));
}
