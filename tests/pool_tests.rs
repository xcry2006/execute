use execute::{
    CommandConfig, CommandPool, CommandPoolSeg, ExecutionConfig, ExecutionMode,
};

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
    // 单线程环境下，pop 之后应为空
    assert!(pool.is_empty());
}

#[test]
fn command_pool_default_execution_mode_is_process() {
    let pool = CommandPool::new();
    assert_eq!(pool.execution_mode(), ExecutionMode::Process);
}

#[test]
fn command_pool_can_be_created_with_thread_mode() {
    let config = ExecutionConfig::new().with_mode(ExecutionMode::Thread);
    let pool = CommandPool::with_config(config);
    assert_eq!(pool.execution_mode(), ExecutionMode::Thread);
}

#[test]
fn command_pool_can_switch_between_modes() {
    // 多进程模式
    let process_config = ExecutionConfig::new()
        .with_mode(ExecutionMode::Process)
        .with_workers(2);
    let process_pool = CommandPool::with_config(process_config);
    assert_eq!(process_pool.execution_mode(), ExecutionMode::Process);
    assert_eq!(process_pool.execution_config().workers, 2);

    // 多线程模式
    let thread_config = ExecutionConfig::new()
        .with_mode(ExecutionMode::Thread)
        .with_workers(4)
        .with_concurrency_limit(8);
    let thread_pool = CommandPool::with_config(thread_config);
    assert_eq!(thread_pool.execution_mode(), ExecutionMode::Thread);
    assert_eq!(thread_pool.execution_config().workers, 4);
    assert_eq!(thread_pool.execution_config().concurrency_limit, Some(8));
}

#[test]
fn execution_mode_thread_and_process_are_different() {
    assert_ne!(ExecutionMode::Thread, ExecutionMode::Process);
}

#[test]
fn execution_config_default_values() {
    let config = ExecutionConfig::new();
    assert_eq!(config.mode, ExecutionMode::Process);
    assert!(config.concurrency_limit.is_none());
    assert!(config.workers > 0);
}

#[test]
fn execution_config_builder_pattern() {
    let config = ExecutionConfig::new()
        .with_mode(ExecutionMode::Thread)
        .with_workers(8)
        .with_concurrency_limit(16);

    assert_eq!(config.mode, ExecutionMode::Thread);
    assert_eq!(config.workers, 8);
    assert_eq!(config.concurrency_limit, Some(16));
}
