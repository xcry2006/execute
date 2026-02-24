use execute::{CommandConfig, CommandPool, TaskState};

#[test]
fn test_submit_returns_task_handle() {
    let pool = CommandPool::new();

    // Submit a task and get a handle
    let handle = pool
        .push_task(CommandConfig::new("echo", vec!["hello".to_string()]))
        .expect("Failed to submit task");

    // Verify handle has correct initial state
    assert_eq!(handle.state(), TaskState::Queued);
    assert!(!handle.is_cancelled());
}

#[test]
fn test_task_handle_wait_for_result() {
    let pool = CommandPool::new();
    pool.start_executor();

    // Submit a task
    let handle = pool
        .push_task(CommandConfig::new("echo", vec!["test".to_string()]))
        .expect("Failed to submit task");

    // Wait for result
    let result = handle.wait();
    assert!(result.is_ok(), "Task should complete successfully");

    pool.shutdown().expect("Failed to shutdown pool");
}

#[test]
fn test_cancel_queued_task() {
    let pool = CommandPool::new();
    // Don't start executor so task stays queued

    // Submit a task
    let handle = pool
        .push_task(CommandConfig::new("sleep", vec!["10".to_string()]))
        .expect("Failed to submit task");

    // Verify initial state
    assert_eq!(handle.state(), TaskState::Queued);

    // Cancel the task
    let cancel_result = handle.cancel();
    assert!(
        cancel_result.is_ok(),
        "Should be able to cancel queued task"
    );

    // Verify state changed
    assert_eq!(handle.state(), TaskState::Cancelled);
    assert!(handle.is_cancelled());
}

#[test]
fn test_multiple_handles_from_same_pool() {
    let pool = CommandPool::new();
    pool.start_executor();

    // Submit multiple tasks
    let handle1 = pool
        .push_task(CommandConfig::new("echo", vec!["1".to_string()]))
        .expect("Failed to submit task 1");
    let handle2 = pool
        .push_task(CommandConfig::new("echo", vec!["2".to_string()]))
        .expect("Failed to submit task 2");
    let handle3 = pool
        .push_task(CommandConfig::new("echo", vec!["3".to_string()]))
        .expect("Failed to submit task 3");

    // All handles should have different IDs
    assert_ne!(handle1.id(), handle2.id());
    assert_ne!(handle2.id(), handle3.id());
    assert_ne!(handle1.id(), handle3.id());

    // Wait for all tasks
    let result1 = handle1.wait();
    let result2 = handle2.wait();
    let result3 = handle3.wait();

    assert!(result1.is_ok());
    assert!(result2.is_ok());
    assert!(result3.is_ok());

    pool.shutdown().expect("Failed to shutdown pool");
}

#[test]
fn test_task_state_transitions() {
    let pool = CommandPool::new();
    pool.start_executor();

    // Submit a quick task
    let handle = pool
        .push_task(CommandConfig::new("true", vec![]))
        .expect("Failed to submit task");

    // Initial state should be Queued
    assert_eq!(handle.state(), TaskState::Queued);

    // Wait for completion
    let _ = handle.wait();

    // Final state should be Completed
    assert_eq!(handle.state(), TaskState::Completed);

    pool.shutdown().expect("Failed to shutdown pool");
}
