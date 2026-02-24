use std::time::{Duration, Instant};

/// Context information available to execution hooks before task execution
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Unique identifier for the task
    pub task_id: u64,
    /// The command being executed
    pub command: String,
    /// ID of the worker thread executing the task
    pub worker_id: usize,
    /// Time when the task execution started
    pub start_time: Instant,
}

impl ExecutionContext {
    /// Creates a new execution context
    pub fn new(task_id: u64, command: String, worker_id: usize) -> Self {
        Self {
            task_id,
            command,
            worker_id,
            start_time: Instant::now(),
        }
    }
}

/// Result information available to execution hooks after task execution
#[derive(Debug, Clone)]
pub struct HookTaskResult {
    /// Exit code of the command (None if terminated by signal)
    pub exit_code: Option<i32>,
    /// Duration of the task execution
    pub duration: Duration,
    /// Size of the command output in bytes
    pub output_size: usize,
    /// Error message if the task failed
    pub error: Option<String>,
}

impl HookTaskResult {
    /// Creates a new task result
    pub fn new(
        exit_code: Option<i32>,
        duration: Duration,
        output_size: usize,
        error: Option<String>,
    ) -> Self {
        Self {
            exit_code,
            duration,
            output_size,
            error,
        }
    }
}

/// Trait for implementing execution hooks
///
/// Hooks allow custom logic to be executed before and after task execution.
/// This is useful for performance analysis, custom monitoring, and logging.
///
/// # Requirements
///
/// - Validates: Requirements 15.1, 15.2, 15.5
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use command_pool::hooks::{ExecutionHook, ExecutionContext, HookTaskResult};
///
/// struct TimingHook;
///
/// impl ExecutionHook for TimingHook {
///     fn before_execute(&self, ctx: &ExecutionContext) {
///         println!("Starting task {} on worker {}", ctx.task_id, ctx.worker_id);
///     }
///     
///     fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult) {
///         println!(
///             "Task {} completed in {:?} with exit code {:?}",
///             ctx.task_id,
///             result.duration,
///             result.exit_code
///         );
///     }
/// }
/// ```
pub trait ExecutionHook: Send + Sync {
    /// Called before a task begins execution
    ///
    /// This hook receives the execution context containing:
    /// - task_id: Unique identifier for the task
    /// - command: The command being executed
    /// - worker_id: ID of the worker thread
    /// - start_time: When execution started
    ///
    /// # Requirements
    ///
    /// - Validates: Requirement 15.1 (支持注册 before_execute 钩子)
    /// - Validates: Requirement 15.5 (允许钩子访问任务 ID、命令和执行时长)
    fn before_execute(&self, ctx: &ExecutionContext);

    /// Called after a task completes execution
    ///
    /// This hook receives both the execution context and the task result containing:
    /// - exit_code: Exit code of the command
    /// - duration: How long the task took to execute
    /// - output_size: Size of the command output
    /// - error: Error message if the task failed
    ///
    /// # Requirements
    ///
    /// - Validates: Requirement 15.2 (支持注册 after_execute 钩子)
    /// - Validates: Requirement 15.5 (允许钩子访问任务 ID、命令和执行时长)
    fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct TestHook {
        before_calls: Arc<Mutex<Vec<u64>>>,
        after_calls: Arc<Mutex<Vec<u64>>>,
    }

    impl TestHook {
        fn new() -> (Self, Arc<Mutex<Vec<u64>>>, Arc<Mutex<Vec<u64>>>) {
            let before = Arc::new(Mutex::new(Vec::new()));
            let after = Arc::new(Mutex::new(Vec::new()));
            let hook = Self {
                before_calls: before.clone(),
                after_calls: after.clone(),
            };
            (hook, before, after)
        }
    }

    impl ExecutionHook for TestHook {
        fn before_execute(&self, ctx: &ExecutionContext) {
            self.before_calls.lock().unwrap().push(ctx.task_id);
        }

        fn after_execute(&self, ctx: &ExecutionContext, _result: &HookTaskResult) {
            self.after_calls.lock().unwrap().push(ctx.task_id);
        }
    }

    #[test]
    fn test_execution_context_creation() {
        let ctx = ExecutionContext::new(42, "echo test".to_string(), 1);
        assert_eq!(ctx.task_id, 42);
        assert_eq!(ctx.command, "echo test");
        assert_eq!(ctx.worker_id, 1);
    }

    #[test]
    fn test_task_result_creation() {
        let result = HookTaskResult::new(Some(0), Duration::from_secs(1), 100, None);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.duration, Duration::from_secs(1));
        assert_eq!(result.output_size, 100);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_hook_trait_implementation() {
        let (hook, before_calls, after_calls) = TestHook::new();

        let ctx = ExecutionContext::new(1, "test".to_string(), 0);
        hook.before_execute(&ctx);

        let result = HookTaskResult::new(Some(0), Duration::from_secs(1), 0, None);
        hook.after_execute(&ctx, &result);

        assert_eq!(*before_calls.lock().unwrap(), vec![1]);
        assert_eq!(*after_calls.lock().unwrap(), vec![1]);
    }

    #[test]
    fn test_hook_can_access_task_info() {
        // Validates Requirement 15.5: 允许钩子访问任务 ID、命令和执行时长
        let ctx = ExecutionContext::new(123, "echo hello".to_string(), 2);

        // Hook can access task_id
        assert_eq!(ctx.task_id, 123);

        // Hook can access command
        assert_eq!(ctx.command, "echo hello");

        // Hook can access execution duration through HookTaskResult
        let result = HookTaskResult::new(Some(0), Duration::from_millis(500), 10, None);
        assert_eq!(result.duration, Duration::from_millis(500));
    }
}
