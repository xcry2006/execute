# 高级功能指南 | Advanced Features Guide

本文档详细介绍 execute 库的高级功能，包括自定义执行器、健康检查、重试机制、超时控制、环境变量、钩子系统、僵尸进程清理等。

## 目录

1. [自定义执行器](#自定义执行器--custom-executor)
2. [健康检查](#健康检查--health-check)
3. [重试机制](#重试机制--retry-mechanism)
4. [分离超时控制](#分离超时控制--separated-timeout)
5. [环境变量配置](#环境变量配置--environment-variables)
6. [钩子系统](#钩子系统--execution-hooks)
7. [僵尸进程清理](#僵尸进程清理--zombie-reaper)
8. [指标收集](#指标收集--metrics)
9. [任务取消](#任务取消--task-cancellation)
10. [资源限制](#资源限制--resource-limits)

---

## 自定义执行器 | Custom Executor

`CommandExecutor` trait 定义了命令执行的标准接口，允许用户实现自己的执行策略：

```rust
pub trait CommandExecutor: Send + Sync {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError>;
}
```

## 标准库执行器 | Standard Executor

默认提供的 `StdCommandExecutor` 使用标准库 `std::process::Command`：

```rust
use execute::{CommandPool, CommandConfig, StdCommandExecutor};
use std::sync::Arc;
use std::time::Duration;

let pool = CommandPool::new();
let executor = Arc::new(StdCommandExecutor);

pool.push_task(CommandConfig::new("echo", vec!["hello".to_string()]));
pool.start_executor_with_executor(Duration::from_secs(1), executor);
```

## 自定义执行器实现 | Custom Executor Implementation

### 示例：Tokio 异步执行器

```rust
use execute::{CommandConfig, ExecuteError, CommandExecutor};
use std::process::Output;
use tokio::process::Command;

pub struct TokioCommandExecutor;

impl CommandExecutor for TokioCommandExecutor {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        // 注意：此处为同步 trait，需要使用 block_on 或在异步上下文中调用
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| ExecuteError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e
            )))?;
        
        rt.block_on(async {
            let mut cmd = Command::new(&config.program);
            cmd.args(&config.args);
            
            if let Some(dir) = &config.working_dir {
                cmd.current_dir(dir);
            }
            
            cmd.output().await
                .map_err(|e| ExecuteError::Io(e))
        })
    }
}
```

### 示例：带超时的异步执行器

```rust
use execute::{CommandConfig, ExecuteError, CommandExecutor};
use std::process::Output;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub struct TokioWithTimeoutExecutor;

impl CommandExecutor for TokioWithTimeoutExecutor {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| ExecuteError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e
            )))?;
        
        rt.block_on(async {
            let mut cmd = Command::new(&config.program);
            cmd.args(&config.args);
            
            if let Some(dir) = &config.working_dir {
                cmd.current_dir(dir);
            }
            
            match config.timeout {
                Some(dur) => {
                    timeout(dur, cmd.output()).await
                        .map_err(|_| ExecuteError::Timeout(dur))?
                        .map_err(|e| ExecuteError::Io(e))
                }
                None => {
                    cmd.output().await
                        .map_err(|e| ExecuteError::Io(e))
                }
            }
        })
    }
}
```

## 使用自定义执行器 | Using Custom Executors

### 基础用法 | Basic Usage

```rust
use execute::{CommandPool, CommandConfig};
use std::sync::Arc;
use std::time::Duration;

let pool = CommandPool::new();
let executor = Arc::new(YourCustomExecutor);

// 启动执行器
pool.start_executor_with_executor(Duration::from_millis(100), executor);

// 添加任务
pool.push_task(CommandConfig::new("your_command", vec![]));
```

### 使用工作线程数 | With Worker Count

```rust
let pool = CommandPool::new();
let executor = Arc::new(YourCustomExecutor);

// 使用 8 个工作线程
pool.start_executor_with_workers_and_executor(
    Duration::from_millis(100),
    8,
    executor,
);
```

### 使用并发限制 | With Concurrency Limit

```rust
let pool = CommandPool::new();
let executor = Arc::new(YourCustomExecutor);

// 8 个工作线程，同时执行最多 4 个外部进程
pool.start_executor_with_executor_and_limit(
    Duration::from_millis(100),
    8,
    4,
    executor,
);
```

## 无锁队列变体 | Lock-free Queue Variant

`CommandPoolSeg` 提供相同的自定义执行器支持，但使用无锁队列实现：

```rust
use execute::{CommandPoolSeg, CommandConfig};
use std::sync::Arc;
use std::time::Duration;

let pool = CommandPoolSeg::new();
let executor = Arc::new(YourCustomExecutor);

pool.push_task(CommandConfig::new("command", vec![]));
pool.start_executor_with_executor(Duration::from_millis(100), executor);
```

## 性能注意事项 | Performance Considerations

1. **同步 trait 的异步实现**：如果使用异步运行时，需要在同步 trait 方法中创建运行时。考虑使用线程本地存储或全局运行时以避免重复创建。

2. **并发限制**：使用信号量限制并发可以防止资源耗尽，特别是处理大量 I/O 密集型命令时。

3. **工作线程数**：根据 CPU 核心数和任务类型调整工作线程数。I/O 密集型任务可以使用更多线程。

## 完整示例 | Complete Example

```rust
use execute::{CommandPool, CommandConfig, CommandExecutor, ExecuteError};
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::Duration;

// 自定义执行器
struct CustomExecutor;

impl CommandExecutor for CustomExecutor {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        // 实现您的执行逻辑：这里给出一个最简同步实现示例
        println!("Executing: {} {:?}", config.program(), config.args());

        let mut cmd = Command::new(config.program());
        cmd.args(config.args());
        if let Some(dir) = config.working_dir() {
            cmd.current_dir(dir);
        }

        cmd.output().map_err(ExecuteError::Io)
    }
}
 
fn main() -> Result<(), ExecuteError> {
    let pool = CommandPool::new();
    let executor = Arc::new(CustomExecutor);

    // 添加任务
    pool.push_task(CommandConfig::new("echo", vec!["hello".to_string()]));
    pool.push_task(CommandConfig::new("ls", vec!["-la".to_string()]));

    // 启动执行器：4 个工作线程，最多 2 个并发
    pool.start_executor_with_executor_and_limit(
        Duration::from_millis(100),
        4,
        2,
        executor,
    );

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(1));
    Ok(())
}
```

---

## 健康检查 | Health Check

健康检查功能允许你监控命令池的运行状态。

### 健康状态类型

```rust
pub enum HealthStatus {
    Healthy,
    Degraded { issues: Vec<String> },
    Unhealthy { issues: Vec<String> },
}
```

### 使用示例

```rust
use execute::{CommandPool, ExecutionConfig, HealthStatus};
use std::time::Duration;

let pool = CommandPool::with_config(ExecutionConfig {
    workers: 4,
    ..Default::default()
});

pool.start_executor(Duration::from_millis(100));

let health = pool.health_check();

match health.status {
    HealthStatus::Healthy => {
        println!("系统健康");
    }
    HealthStatus::Degraded { issues } => {
        println!("系统降级: {:?}", issues);
    }
    HealthStatus::Unhealthy { issues } => {
        println!("系统不健康: {:?}", issues);
    }
}

println!("工作线程: {}/{}", 
    health.details.workers_alive,
    health.details.workers_total
);
```

### 健康指标

- **workers_alive**: 活跃的工作线程数
- **workers_total**: 配置的总工作线程数
- **queue_usage**: 队列使用率 (0.0 - 1.0)
- **long_running_tasks**: 运行超过 5 分钟的任务数
- **avg_task_duration**: 平均任务执行时间

---

## 重试机制 | Retry Mechanism

自动重试失败的任务，支持固定间隔和指数退避策略。

### 重试策略

```rust
pub enum RetryStrategy {
    FixedInterval(Duration),
    ExponentialBackoff {
        initial: Duration,
        max: Duration,
        multiplier: f64,
    },
}
```

### 使用示例

```rust
use execute::{CommandConfig, RetryPolicy, RetryStrategy, execute_with_retry};
use std::time::Duration;

// 配置重试策略 - 指数退避
let policy = RetryPolicy::new(
    3,  // 最多重试 3 次
    RetryStrategy::ExponentialBackoff {
        initial: Duration::from_millis(100),
        max: Duration::from_secs(10),
        multiplier: 2.0,
    }
);

let config = CommandConfig::new("curl", vec!["https://example.com".to_string()])
    .with_retry(policy);

match execute_with_retry(&config, 1) {
    Ok(output) => println!("成功: {}", String::from_utf8_lossy(&output.stdout)),
    Err(e) => eprintln!("重试后仍失败: {}", e),
}
```

### 固定间隔重试

```rust
let policy = RetryPolicy::new(
    3,
    RetryStrategy::FixedInterval(Duration::from_secs(1))
);
```

---

## 分离超时控制 | Separated Timeout

精细控制命令启动超时和执行超时，便于错误诊断。

### TimeoutConfig

```rust
pub struct TimeoutConfig {
    pub spawn_timeout: Option<Duration>,    // 进程创建超时
    pub execution_timeout: Option<Duration>, // 执行超时
}
```

### 使用示例

```rust
use execute::{CommandConfig, TimeoutConfig, execute_with_timeouts};
use std::time::Duration;

let timeout_config = TimeoutConfig::new()
    .with_spawn_timeout(Duration::from_secs(5))      // 5秒启动超时
    .with_execution_timeout(Duration::from_secs(30)); // 30秒执行超时

let config = CommandConfig::new("my-command", vec![])
    .with_timeouts(timeout_config);

match execute_with_timeouts(&config, 1) {
    Ok(output) => println!("成功: {:?}", output),
    Err(CommandError::Timeout { context, configured_timeout, actual_duration }) => {
        eprintln!("超时: 配置={:?}, 实际={:?}", configured_timeout, actual_duration);
    }
    Err(e) => eprintln!("错误: {}", e),
}
```

### 与重试结合使用

```rust
let timeout_config = TimeoutConfig::new()
    .with_execution_timeout(Duration::from_secs(10));

let retry_policy = RetryPolicy::new(
    3,
    RetryStrategy::FixedInterval(Duration::from_secs(1))
);

let config = CommandConfig::new("flaky-command", vec![])
    .with_timeouts(timeout_config)
    .with_retry(retry_policy);

let result = execute_with_retry(&config, 1);
```

---

## 环境变量配置 | Environment Variables

为命令配置自定义环境变量，支持继承、覆盖和清除。

### EnvConfig

```rust
pub struct EnvConfig {
    pub inherit_parent: bool,           // 是否继承父进程环境变量
    pub vars: HashMap<String, Option<String>>, // 环境变量映射
}
```

### 使用示例

```rust
use execute::{CommandConfig, EnvConfig, execute_command_with_context};

// 设置环境变量
let env = EnvConfig::new()
    .set("MY_VAR", "my_value")
    .set("ANOTHER_VAR", "42");

let config = CommandConfig::new("printenv", vec!["MY_VAR".to_string()])
    .with_env(env);

let result = execute_command_with_context(&config, 1)?;

// 清除特定环境变量
let env = EnvConfig::new()
    .remove("TEMP_VAR");

// 不继承父进程环境变量
let env = EnvConfig::new()
    .no_inherit()
    .set("ONLY_VAR", "only_value");
```

---

## 钩子系统 | Execution Hooks

在任务执行前后插入自定义逻辑，用于性能分析、监控等。

### ExecutionHook Trait

```rust
pub trait ExecutionHook: Send + Sync {
    fn before_execute(&self, ctx: &ExecutionContext);
    fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult);
}
```

### 使用示例

```rust
use execute::{CommandConfig, execute_task_with_hooks};
use execute::{ExecutionHook, ExecutionContext, HookTaskResult};
use std::sync::Arc;

struct TimingHook;

impl ExecutionHook for TimingHook {
    fn before_execute(&self, ctx: &ExecutionContext) {
        println!("开始执行任务 {}", ctx.task_id);
    }
    
    fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult) {
        println!("任务 {} 完成，耗时 {:?}", ctx.task_id, result.duration);
    }
}

let config = CommandConfig::new("echo", vec!["hello".to_string()]);
let hooks = vec![Arc::new(TimingHook) as Arc<dyn ExecutionHook>];
let result = execute_task_with_hooks(&config, 1, 0, &hooks);
```

### 钩子隔离性

钩子 panic 不会影响任务执行，系统会捕获 panic 并记录警告日志。

---

## 僵尸进程清理 | Zombie Reaper

自动清理已终止的子进程，防止僵尸进程累积。

### 使用示例

```rust
use execute::{CommandPool, ExecutionConfig};
use std::time::Duration;

// 创建带僵尸进程清理的池（每5秒检查一次）
let config = ExecutionConfig::new()
    .with_workers(4)
    .with_zombie_reaper_interval(Duration::from_secs(5));

let pool = CommandPool::with_config(config);
pool.start_executor(Duration::from_millis(100));

// 僵尸进程会自动清理
```

### 平台支持

- **Unix**: 使用 `waitpid(-1, WNOHANG)` 回收僵尸进程
- **非 Unix**: 无操作实现

---

## 指标收集 | Metrics

实时收集任务执行指标，用于监控和性能分析。

### 使用示例

```rust
use execute::CommandPool;

let pool = CommandPool::new();
// ... 执行任务 ...

let metrics = pool.metrics();
println!("总任务数: {}", metrics.total_tasks);
println!("成功任务: {}", metrics.successful_tasks);
println!("失败任务: {}", metrics.failed_tasks);
println!("成功率: {:.2}%", metrics.success_rate * 100.0);
println!("平均执行时间: {:?}", metrics.avg_execution_time);
```

### 可用指标

- 任务计数（总数/成功/失败）
- 成功率
- 执行时间统计（最小/最大/平均/百分位数）
- 队列等待时间

---

## 任务取消 | Task Cancellation

支持取消队列中或执行中的任务。

### 使用示例

```rust
use execute::{CommandPool, CommandConfig};
use std::time::Duration;

let pool = CommandPool::new();

// 提交任务并获取句柄
let config = CommandConfig::new("long_running_command", vec![]);
let handle = pool.submit_task(config)?;

// 取消任务
match handle.cancel() {
    Ok(()) => println!("任务已取消"),
    Err(e) => println!("取消失败: {}", e),
}
```

---

## 资源限制 | Resource Limits

限制命令的输出大小和内存使用。

### 使用示例

```rust
use execute::{CommandConfig, ResourceLimits};

let limits = ResourceLimits::new()
    .with_max_output_size(1024 * 1024)  // 1MB 输出限制
    .with_max_memory(100 * 1024 * 1024); // 100MB 内存限制

let config = CommandConfig::new("command", vec![])
    .with_resource_limits(limits);
```

---

## 总结

execute 库提供了丰富的高级功能：

| 功能 | 用途 |
|------|------|
| 自定义执行器 | 集成不同运行时（Tokio 等） |
| 健康检查 | 监控命令池状态 |
| 重试机制 | 自动重试失败任务 |
| 分离超时 | 精细控制启动和执行超时 |
| 环境变量 | 配置命令执行环境 |
| 钩子系统 | 性能分析和监控 |
| 僵尸进程清理 | 防止资源泄漏 |
| 指标收集 | 实时监控和统计 |
| 任务取消 | 灵活的任务管理 |
| 资源限制 | 防止资源耗尽 |

更多示例请参考 `examples/` 目录。
