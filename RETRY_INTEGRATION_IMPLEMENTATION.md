# 重试机制集成实现文档

## 概述

本文档描述了任务 14.4 的实现：将重试机制集成到命令执行流程中。此实现确保重试逻辑在 CommandPool 和 CommandPoolSeg 中正确工作，同时保持指标的准确性。

## 实现的需求

- **需求 11.4**: 任务失败且未达到最大重试次数时，系统应自动重试任务
- **需求 11.5**: 任务重试时，系统应记录重试次数和原因
- **需求 11.6**: 达到最大重试次数后仍失败时，系统应返回最终错误

## 设计决策

### 1. 重试逻辑的位置

重试逻辑集成在任务执行层面，而不是在队列层面：

- **CommandPool**: 在 `execute_task()` 方法中检查是否配置了重试策略
- **CommandPoolSeg**: 添加 `execute_task_with_retry()` 辅助方法处理重试

这种设计确保：
- 重试对调用者透明
- 指标准确记录（每个任务只计数一次，无论重试多少次）
- 重试日志正确记录在 `execute_with_retry()` 函数中

### 2. 重试的错误类型

重试机制只重试**执行错误**，不重试**命令失败**：

- **执行错误**（会重试）：
  - 进程启动失败 (SpawnFailed)
  - 执行超时 (Timeout)
  - I/O 错误 (ExecutionFailed)

- **命令失败**（不会重试）：
  - 非零退出码（如 `false` 命令返回 1）
  - 这些是成功的执行，只是命令本身失败了

这种区分很重要，因为：
- 执行错误通常是临时性的（网络问题、资源不足）
- 命令失败通常是确定性的（逻辑错误、参数错误）

### 3. 指标准确性

重试不影响指标的准确性：

- 每个任务只计数一次（提交时 +1）
- 最终结果只记录一次（成功或失败）
- 重试过程中的中间失败不计入指标
- 执行时间包括所有重试的总时间

## 实现细节

### CommandPool 集成

```rust
pub fn execute_task(
    &self,
    config: &CommandConfig,
) -> Result<std::process::Output, ExecuteError> {
    let task_id = self.task_id_counter.load(Ordering::SeqCst);
    let start_time = Instant::now();

    // 记录任务开始
    tracing::info!(task_id = task_id, command = %config.program(), "Task execution started");
    self.metrics.record_task_started();

    // 如果配置了重试策略，使用 execute_with_retry
    let result = if config.retry_policy().is_some() {
        use crate::executor::execute_with_retry;
        execute_with_retry(config, task_id)
            .map_err(|e| ExecuteError::Io(std::io::Error::other(e.to_string())))
    } else {
        // 否则直接使用后端执行
        self.backend.execute(config)
    };
    
    let duration = start_time.elapsed();

    // 记录最终结果
    match &result {
        Ok(output) => {
            tracing::info!(task_id = task_id, exit_code = output.status.code().unwrap_or(-1), 
                          duration_ms = duration.as_millis(), "Task completed successfully");
            self.metrics.record_task_completed(duration);
        }
        Err(e) => {
            tracing::error!(task_id = task_id, error = %e, 
                           duration_ms = duration.as_millis(), "Task failed");
            self.metrics.record_task_failed(duration);
        }
    }

    result
}
```

### CommandPoolSeg 集成

为 CommandPoolSeg 添加了 `task_id_counter` 字段和 `execute_task_with_retry()` 方法：

```rust
pub struct CommandPoolSeg {
    tasks: Arc<SegQueue<CommandConfig>>,
    stop_flag: Arc<AtomicBool>,
    task_id_counter: Arc<AtomicU64>,  // 新增
}

fn execute_task_with_retry(&self, task: &CommandConfig) -> Result<Output, ExecuteError> {
    let task_id = self.task_id_counter.fetch_add(1, Ordering::SeqCst);
    
    if task.retry_policy().is_some() {
        use crate::executor::execute_with_retry;
        execute_with_retry(task, task_id)
            .map_err(|e| ExecuteError::Io(std::io::Error::other(e.to_string())))
    } else {
        execute_command(task)
    }
}
```

所有 worker 线程方法都更新为使用 `execute_task_with_retry()`：
- `start_executor_with_workers()`
- `start_executor_with_workers_and_limit()`

自定义执行器方法保持不变，因为它们使用用户提供的执行器。

## 测试验证

### 测试用例

1. **test_commandpool_retry_on_timeout**: 验证超时错误会触发重试
2. **test_commandpool_no_retry_on_nonzero_exit**: 验证非零退出码不会触发重试
3. **test_commandpool_retry_success_after_retry**: 验证成功的命令不会触发重试
4. **test_commandpoolseg_retry_on_timeout**: 验证 CommandPoolSeg 的重试功能
5. **test_retry_without_retry_policy**: 验证没有重试策略时的行为
6. **test_metrics_accuracy_with_retry**: 验证重试不影响指标准确性

### 测试结果

所有测试通过，验证了：
- ✅ 重试逻辑正确集成到执行流程
- ✅ 重试日志正确记录（attempt、max_attempts、error）
- ✅ 指标准确性不受重试影响
- ✅ 超时错误会触发重试
- ✅ 非零退出码不会触发重试

## 使用示例

### 基本用法

```rust
use execute::{CommandPool, CommandConfig, RetryPolicy, RetryStrategy};
use std::time::Duration;

let pool = CommandPool::new();
pool.start_executor(Duration::from_millis(100));

// 配置重试策略
let retry_policy = RetryPolicy::new(
    3, // 最多重试 3 次
    RetryStrategy::FixedInterval(Duration::from_millis(100)),
);

// 创建可能超时的命令
let config = CommandConfig::new("curl", vec!["https://example.com".to_string()])
    .with_timeout(Duration::from_secs(5))
    .with_retry(retry_policy);

// 提交任务 - 如果超时会自动重试
pool.push_task(config).unwrap();
```

### 指数退避重试

```rust
let retry_policy = RetryPolicy::new(
    5,
    RetryStrategy::ExponentialBackoff {
        initial: Duration::from_millis(100),
        max: Duration::from_secs(10),
        multiplier: 2.0,
    },
);

let config = CommandConfig::new("flaky-command", vec![])
    .with_retry(retry_policy);

pool.push_task(config).unwrap();
```

## 日志输出

重试过程会产生详细的日志：

```
INFO  Task execution started task_id=1 command=sleep
WARN  Command execution failed task_id=1 attempt=1 max_attempts=3 error=Command timeout...
INFO  Retrying command after failure task_id=1 attempt=1 max_attempts=2 command="sleep 2"
WARN  Command execution failed task_id=1 attempt=2 max_attempts=3 error=Command timeout...
INFO  Retrying command after failure task_id=1 attempt=2 max_attempts=2 command="sleep 2"
WARN  Command execution failed task_id=1 attempt=3 max_attempts=3 error=Command timeout...
ERROR Command failed after all retry attempts task_id=1 attempts=3
ERROR Task failed task_id=1 error=io error: Command timeout... duration_ms=351
```

## 性能影响

- **无重试策略**: 零开销，直接执行
- **有重试策略**: 
  - 每次重试增加配置的延迟时间
  - 重试日志记录的开销（可忽略）
  - 总执行时间 = 所有尝试的时间 + 重试延迟

## 向后兼容性

- ✅ 完全向后兼容
- ✅ 不配置重试策略时行为不变
- ✅ 现有代码无需修改
- ✅ 重试是可选功能

## 相关文件

- `src/pool.rs`: CommandPool 的重试集成
- `src/pool_seg.rs`: CommandPoolSeg 的重试集成
- `src/executor.rs`: execute_with_retry() 实现（任务 14.3）
- `tests/retry_integration_test.rs`: 集成测试
- `examples/retry_integration_demo.rs`: 使用示例

## 下一步

任务 14.4 已完成。下一个任务是 14.5: 编写重试行为属性测试（可选）。
