use execute::{CommandConfig, CommandPool, CommandPoolSeg};

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
