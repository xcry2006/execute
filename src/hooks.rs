use std::time::{Duration, Instant};

/// 执行上下文，包含任务执行前的上下文信息
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// 任务唯一标识符
    pub task_id: u64,
    /// 正在执行的命令
    pub command: String,
    /// 执行任务的 worker 线程 ID
    pub worker_id: usize,
    /// 任务开始执行的时间
    pub start_time: Instant,
}

impl ExecutionContext {
    /// 创建新的执行上下文
    pub fn new(task_id: u64, command: String, worker_id: usize) -> Self {
        Self {
            task_id,
            command,
            worker_id,
            start_time: Instant::now(),
        }
    }
}

/// 钩子任务结果，包含任务执行后的结果信息
#[derive(Debug, Clone)]
pub struct HookTaskResult {
    /// 命令退出码（如果被信号终止则为 None）
    pub exit_code: Option<i32>,
    /// 任务执行时长
    pub duration: Duration,
    /// 命令输出大小（字节）
    pub output_size: usize,
    /// 如果任务失败，包含错误信息
    pub error: Option<String>,
}

impl HookTaskResult {
    /// 创建新的任务结果
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

/// 执行钩子 trait
///
/// 允许在任务执行前后插入自定义逻辑。
/// 适用于性能分析、自定义监控和日志记录。
///
/// # 要求
///
/// - 验证：需求 15.1、15.2、15.5
///
/// # 示例
///
/// ```rust
/// use std::sync::Arc;
/// use execute::{ExecutionHook, ExecutionContext, HookTaskResult};
///
/// struct TimingHook;
///
/// impl ExecutionHook for TimingHook {
///     fn before_execute(&self, ctx: &ExecutionContext) {
///         println!("开始执行任务 {} 在 worker {}", ctx.task_id, ctx.worker_id);
///     }
///     
///     fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult) {
///         println!(
///             "任务 {} 完成，耗时 {:?}，退出码 {:?}",
///             ctx.task_id,
///             result.duration,
///             result.exit_code
///         );
///     }
/// }
/// ```
pub trait ExecutionHook: Send + Sync {
    /// 在任务开始执行前调用
    ///
    /// 此钩子接收执行上下文，包含：
    /// - task_id: 任务的唯一标识符
    /// - command: 正在执行的命令
    /// - worker_id: worker 线程的 ID
    /// - start_time: 执行开始时间
    ///
    /// # 要求
    ///
    /// - 验证：需求 15.1（支持注册 before_execute 钩子）
    /// - 验证：需求 15.5（允许钩子访问任务 ID、命令和执行时长）
    fn before_execute(&self, ctx: &ExecutionContext);

    /// 在任务完成执行后调用
    ///
    /// 此钩子接收执行上下文和任务结果，包含：
    /// - exit_code: 命令退出码
    /// - duration: 任务执行时长
    /// - output_size: 命令输出大小
    /// - error: 如果任务失败，包含错误信息
    ///
    /// # 要求
    ///
    /// - 验证：需求 15.2（支持注册 after_execute 钩子）
    /// - 验证：需求 15.5（允许钩子访问任务 ID、命令和执行时长）
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
