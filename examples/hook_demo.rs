use execute::{CommandPool, ExecutionConfig, ExecutionContext, ExecutionHook, HookTaskResult};
use std::sync::Arc;

/// Example hook that logs task execution
struct LoggingHook;

impl ExecutionHook for LoggingHook {
    fn before_execute(&self, ctx: &ExecutionContext) {
        println!(
            "[Hook] Task {} starting: {} (worker {})",
            ctx.task_id, ctx.command, ctx.worker_id
        );
    }

    fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult) {
        println!(
            "[Hook] Task {} completed: exit_code={:?}, duration={:?}, output_size={}",
            ctx.task_id, result.exit_code, result.duration, result.output_size
        );
        if let Some(error) = &result.error {
            println!("[Hook] Task {} error: {}", ctx.task_id, error);
        }
    }
}

/// Example hook that tracks performance
struct PerformanceHook;

impl ExecutionHook for PerformanceHook {
    fn before_execute(&self, ctx: &ExecutionContext) {
        println!(
            "[Perf] Starting task {} at {:?}",
            ctx.task_id, ctx.start_time
        );
    }

    fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult) {
        let duration_ms = result.duration.as_millis();
        if duration_ms > 1000 {
            println!(
                "[Perf] WARNING: Task {} took {}ms (slow!)",
                ctx.task_id, duration_ms
            );
        } else {
            println!("[Perf] Task {} completed in {}ms", ctx.task_id, duration_ms);
        }
    }
}

fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== CommandPool Hook Demo ===\n");

    // Create a command pool with multiple hooks
    let pool = CommandPool::with_config(ExecutionConfig::default())
        .with_hook(Arc::new(LoggingHook))
        .with_hook(Arc::new(PerformanceHook));

    println!("Created CommandPool with 2 hooks\n");

    // Note: The hooks will be called when task execution is implemented in task 18.3
    // For now, this demonstrates that the with_hook() method works correctly

    println!("\nPool created successfully!");
    println!("Queue size: {}", pool.len());
    println!("Max size: {:?}", pool.max_size());

    println!("\n=== Demo Complete ===");
    println!("Note: Hook execution will be demonstrated in task 18.3");
}
