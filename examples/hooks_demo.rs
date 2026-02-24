use execute::{CommandConfig, execute_task_with_hooks};
use execute::{ExecutionContext, ExecutionHook, HookTaskResult};
use std::sync::Arc;
use std::time::Duration;

/// 简单的计时钩子，记录任务执行时间
struct TimingHook;

impl ExecutionHook for TimingHook {
    fn before_execute(&self, ctx: &ExecutionContext) {
        println!(
            "[TimingHook] Task {} starting on worker {}: {}",
            ctx.task_id, ctx.worker_id, ctx.command
        );
    }

    fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult) {
        println!(
            "[TimingHook] Task {} completed in {:?} with exit code {:?}",
            ctx.task_id, result.duration, result.exit_code
        );
    }
}

/// 性能分析钩子，记录详细的性能信息
struct PerformanceHook;

impl ExecutionHook for PerformanceHook {
    fn before_execute(&self, ctx: &ExecutionContext) {
        println!(
            "[PerformanceHook] Starting performance analysis for task {}",
            ctx.task_id
        );
    }

    fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult) {
        println!(
            "[PerformanceHook] Performance report for task {}:",
            ctx.task_id
        );
        println!("  - Duration: {:?}", result.duration);
        println!("  - Output size: {} bytes", result.output_size);
        println!("  - Exit code: {:?}", result.exit_code);

        if let Some(error) = &result.error {
            println!("  - Error: {}", error);
        }

        // 性能分析
        if result.duration > Duration::from_secs(1) {
            println!("  ⚠️  Warning: Task took longer than 1 second");
        }

        if result.output_size > 1024 * 1024 {
            println!("  ⚠️  Warning: Output size exceeds 1MB");
        }
    }
}

/// 日志钩子，记录任务执行日志
struct LoggingHook;

impl ExecutionHook for LoggingHook {
    fn before_execute(&self, ctx: &ExecutionContext) {
        println!(
            "[LoggingHook] Task {} | Worker {} | Command: {}",
            ctx.task_id, ctx.worker_id, ctx.command
        );
    }

    fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult) {
        let status = if result.error.is_some() {
            "FAILED"
        } else if result.exit_code == Some(0) {
            "SUCCESS"
        } else {
            "ERROR"
        };

        println!(
            "[LoggingHook] Task {} | Status: {} | Duration: {:?}",
            ctx.task_id, status, result.duration
        );
    }
}

fn main() {
    println!("=== Execution Hooks Demo ===\n");

    // 创建钩子
    let timing_hook: Arc<dyn ExecutionHook> = Arc::new(TimingHook);
    let perf_hook: Arc<dyn ExecutionHook> = Arc::new(PerformanceHook);
    let log_hook: Arc<dyn ExecutionHook> = Arc::new(LoggingHook);

    let hooks = vec![timing_hook, perf_hook, log_hook];

    // 示例 1: 成功的命令
    println!("--- Example 1: Successful command ---");
    let config1 = CommandConfig::new("echo", vec!["Hello, World!".to_string()]);
    match execute_task_with_hooks(&config1, 1, 0, &hooks) {
        Ok(output) => {
            println!("✓ Command succeeded");
            println!("  Output: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("✗ Command failed: {}", e);
        }
    }
    println!();

    // 示例 2: 失败的命令
    println!("--- Example 2: Failed command ---");
    let config2 = CommandConfig::new("false", vec![]);
    match execute_task_with_hooks(&config2, 2, 0, &hooks) {
        Ok(output) => {
            if output.status.success() {
                println!("✓ Command succeeded");
            } else {
                println!(
                    "✗ Command returned non-zero exit code: {:?}",
                    output.status.code()
                );
            }
        }
        Err(e) => {
            println!("✗ Command failed: {}", e);
        }
    }
    println!();

    // 示例 3: 带超时的命令
    println!("--- Example 3: Command with timeout ---");
    let config3 =
        CommandConfig::new("sleep", vec!["2".to_string()]).with_timeout(Duration::from_millis(500));
    match execute_task_with_hooks(&config3, 3, 1, &hooks) {
        Ok(output) => {
            println!("✓ Command succeeded: {:?}", output.status);
        }
        Err(e) => {
            println!("✗ Command failed (expected timeout): {}", e);
        }
    }
    println!();

    // 示例 4: 生成大量输出的命令
    println!("--- Example 4: Command with large output ---");
    let config4 = CommandConfig::new("seq", vec!["1".to_string(), "1000".to_string()]);
    match execute_task_with_hooks(&config4, 4, 1, &hooks) {
        Ok(output) => {
            println!("✓ Command succeeded");
            println!(
                "  Output lines: {}",
                output.stdout.split(|&b| b == b'\n').count()
            );
        }
        Err(e) => {
            println!("✗ Command failed: {}", e);
        }
    }
    println!();

    // 示例 5: 使用单个钩子
    println!("--- Example 5: Single hook ---");
    let single_hook = vec![Arc::new(TimingHook) as Arc<dyn ExecutionHook>];
    let config5 = CommandConfig::new("date", vec![]);
    match execute_task_with_hooks(&config5, 5, 2, &single_hook) {
        Ok(output) => {
            println!("✓ Command succeeded");
            println!(
                "  Output: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            );
        }
        Err(e) => {
            println!("✗ Command failed: {}", e);
        }
    }

    println!("\n=== Demo Complete ===");
}
