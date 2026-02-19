use execute::{CommandConfig, CommandPool, CommandPoolSeg, ExecutionConfig, ExecutionMode};

#[test]
fn command_pool_push_pop_and_is_empty_work() {
    let pool = CommandPool::new();
    assert!(pool.is_empty());

    pool.push_task(CommandConfig::new("echo", vec!["hi".to_string()]));
    assert!(!pool.is_empty());

    let task = pool.pop_task().expect("expected a task");
    assert_eq!(task.program(), "echo");
    assert!(pool.is_empty());
}

#[test]
fn command_pool_seg_push_pop_and_is_empty_work() {
    let pool = CommandPoolSeg::new();
    assert!(pool.is_empty());

    pool.push_task(CommandConfig::new("echo", vec!["seg".to_string()]));
    assert!(!pool.is_empty());

    let task = pool.pop_task().expect("expected a task");
    assert_eq!(task.program(), "echo");
    assert!(pool.is_empty());
}

#[test]
fn command_pool_default_execution_mode_is_process() {
    let pool = CommandPool::new();
    assert_eq!(pool.execution_mode(), ExecutionMode::Process);
}

#[test]
fn command_pool_can_use_thread_mode() {
    let config = ExecutionConfig::new().with_mode(ExecutionMode::Thread);
    let pool = CommandPool::with_config(config);
    assert_eq!(pool.execution_mode(), ExecutionMode::Thread);
}

#[test]
fn execution_mode_thread_and_process_are_different() {
    assert_ne!(ExecutionMode::Thread, ExecutionMode::Process);
}

#[test]
fn execution_config_builder_pattern() {
    let config = ExecutionConfig::new()
        .with_mode(ExecutionMode::Thread)
        .with_workers(8);

    assert_eq!(config.mode, ExecutionMode::Thread);
    assert_eq!(config.workers, 8);
}

#[test]
fn command_pool_can_use_process_pool_mode() {
    let config = ExecutionConfig::new().with_mode(ExecutionMode::ProcessPool);
    let pool = CommandPool::with_config(config);
    assert_eq!(pool.execution_mode(), ExecutionMode::ProcessPool);
}

#[test]
fn all_execution_modes_are_different() {
    assert_ne!(ExecutionMode::Process, ExecutionMode::Thread);
    assert_ne!(ExecutionMode::Process, ExecutionMode::ProcessPool);
    assert_ne!(ExecutionMode::Thread, ExecutionMode::ProcessPool);
}

#[test]
fn execution_config_can_create_all_modes() {
    let process_config = ExecutionConfig::new().with_mode(ExecutionMode::Process);
    assert_eq!(process_config.mode, ExecutionMode::Process);

    let thread_config = ExecutionConfig::new().with_mode(ExecutionMode::Thread);
    assert_eq!(thread_config.mode, ExecutionMode::Thread);

    let pool_config = ExecutionConfig::new().with_mode(ExecutionMode::ProcessPool);
    assert_eq!(pool_config.mode, ExecutionMode::ProcessPool);
}

#[test]
fn command_pool_with_queue_limit() {
    let config = ExecutionConfig::new();
    let pool = CommandPool::with_config_and_limit(config, 2);
    
    assert_eq!(pool.max_size(), Some(2));
    assert_eq!(pool.len(), 0);
    
    // 添加任务
    pool.push_task(CommandConfig::new("echo", vec!["1".to_string()]));
    assert_eq!(pool.len(), 1);
    
    pool.push_task(CommandConfig::new("echo", vec!["2".to_string()]));
    assert_eq!(pool.len(), 2);
    
    // 使用 try_push_task 测试队列满的情况
    let result = pool.try_push_task(CommandConfig::new("echo", vec!["3".to_string()]));
    assert!(result.is_err());
}

#[test]
fn command_pool_without_queue_limit() {
    let pool = CommandPool::new();
    
    assert_eq!(pool.max_size(), None);
    
    // 可以添加多个任务
    for i in 0..100 {
        pool.push_task(CommandConfig::new("echo", vec![i.to_string()]));
    }
    
    assert_eq!(pool.len(), 100);
}
