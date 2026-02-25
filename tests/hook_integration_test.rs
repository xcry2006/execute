use execute::{CommandPool, ExecutionConfig, ExecutionContext, ExecutionHook, HookTaskResult};
use std::sync::{Arc, Mutex};

// 类型别名简化复杂类型
type CallVec = Arc<Mutex<Vec<u64>>>;

/// Test hook that records calls
struct TestHook {
    before_calls: CallVec,
    after_calls: CallVec,
}

impl TestHook {
    fn new() -> (Self, CallVec, CallVec) {
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
fn test_with_hook_method_exists() {
    // Validates Requirement 15.1: 支持注册 before_execute 钩子
    // Validates Requirement 15.2: 支持注册 after_execute 钩子

    let (hook, _before, _after) = TestHook::new();

    // Test that with_hook method exists and can be called
    let pool = CommandPool::with_config(ExecutionConfig::default()).with_hook(Arc::new(hook));

    // Verify pool was created successfully
    assert_eq!(pool.len(), 0);
}

#[test]
fn test_with_hook_chaining() {
    // Test that with_hook supports method chaining
    let (hook1, _before1, _after1) = TestHook::new();
    let (hook2, _before2, _after2) = TestHook::new();

    let pool = CommandPool::with_config(ExecutionConfig::default())
        .with_hook(Arc::new(hook1))
        .with_hook(Arc::new(hook2));

    // Verify pool was created successfully
    assert_eq!(pool.len(), 0);
}

#[test]
fn test_pool_without_hooks() {
    // Test that pool can be created without hooks
    let pool = CommandPool::with_config(ExecutionConfig::default());

    // Verify pool was created successfully
    assert_eq!(pool.len(), 0);
}
