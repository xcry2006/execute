# execute

![crates.io](https://img.shields.io/crates/v/execute?label=crate)
![license](https://img.shields.io/badge/license-MIT-blue)

生产环境就绪的 Rust 命令池库 — 提供可插拔的命令执行器、锁/无锁任务队列、线程池与并发限制策略，以及完整的可观测性和可靠性特性。

## 主要特性

### 核心功能
 - 多线程安全的任务队列：`CommandPool`（基于 `Mutex<VecDeque>`）
 - 无锁队列变体：`CommandPoolSeg`（基于 `crossbeam_queue::SegQueue`）
 - 可扩展执行器接口：`CommandExecutor`（可集成 `tokio` / `async-std`）
 - 子进程超时与安全等待：使用 `wait-timeout` 避免额外等待线程
 - 线程池、并发限制（信号量）和多种执行模式
 - **执行器停止机制**：优雅关闭执行器线程
 - **队列大小限制**：支持有界队列，防止内存无限增长
 - **批量操作接口**：批量提交任务，提高吞吐量
 - **任务状态查询**：追踪任务状态（Pending/Running/Completed/Failed）
 - **任务结果获取**：异步获取任务执行结果（TaskHandle）
 - **真正的进程池**：常驻子进程池，通过 IPC 通信执行命令
 - **Pipeline 支持**：命令管道，支持链式执行多个命令

### 生产环境特性（新增）

#### 可观测性
 - **结构化日志**：基于 `tracing` 的结构化日志，支持 JSON/Pretty/Compact 格式
 - **指标收集**：实时收集任务执行指标（成功率、执行时间、百分位数等）
 - **健康检查**：提供健康检查接口，监控系统状态
 - **性能分析钩子**：在任务执行前后插入自定义逻辑

#### 可靠性
 - **优雅关闭**：确保正在执行的任务完成后再关闭
 - **错误上下文增强**：详细的错误信息，包含完整执行上下文
 - **配置参数验证**：在构造时验证所有配置参数
 - **僵尸进程清理**：自动清理僵尸进程，避免资源泄漏

#### 高级功能
 - **错误重试机制**：支持固定间隔和指数退避重试策略
 - **超时粒度控制**：分离启动超时和执行超时
 - **任务取消机制**：支持取消队列中或执行中的任务
 - **环境变量支持**：为命令设置自定义环境变量
 - **资源限制**：限制命令输出大小和内存使用

## 快速开始

### 安装

添加依赖（`Cargo.toml`）:

```toml
[dependencies]
execute = "0.1"
```

如果需要使用特定的功能，可以启用相应的 feature（参见 [Cargo.toml](Cargo.toml) 中的 feature 定义）。

### 基础使用

```rust
use execute::{CommandPool, CommandConfig};
use std::time::Duration;

let pool = CommandPool::new();
pool.push_task(CommandConfig::new("echo", vec!["hello".to_string()]));
pool.start_executor();
```

### 使用配置构建器

```rust
use execute::{CommandPool, PoolConfigBuilder};
use std::time::Duration;

let pool = PoolConfigBuilder::new()
    .thread_count(4)
    .queue_capacity(100)
    .enable_metrics(true)
    .enable_health_check(true)
    .build()
    .unwrap();
```

## 功能使用指南

### 1. 结构化日志

使用 `tracing` 库记录任务执行的完整生命周期：

```rust
use execute::{CommandPool, LogConfig, LogLevel, LogFormat, LogTarget};

// 配置日志
let log_config = LogConfig {
    level: LogLevel::Info,
    format: LogFormat::Pretty,  // 或 Json、Compact
    target: LogTarget::Stdout,  // 或 Stderr、File(path)
};

let pool = PoolConfigBuilder::new()
    .with_log_config(log_config)
    .build()
    .unwrap();

// 日志会自动记录：
// - 任务提交：task_id、command、timestamp
// - 任务开始：task_id、worker_id、start_time
// - 任务完成：task_id、exit_code、duration
// - 错误信息：task_id、error、context
```

示例输出（Pretty 格式）：
```
2024-01-15T10:30:45.123Z INFO execute::pool: Task submitted task_id=1 command="echo hello"
2024-01-15T10:30:45.124Z INFO execute::worker: Task execution started task_id=1 worker_id=0
2024-01-15T10:30:45.234Z INFO execute::worker: Task completed task_id=1 exit_code=0 duration_ms=110
```

完整示例：`examples/logging_demo.rs`

### 2. 优雅关闭

确保正在执行的任务完成后再关闭：

```rust
use execute::CommandPool;
use std::time::Duration;

let pool = CommandPool::new();

// 提交任务
pool.push_task(CommandConfig::new("sleep", vec!["5".to_string()]));

// 优雅关闭（默认 30 秒超时）
pool.shutdown().unwrap();

// 或指定超时时间
pool.shutdown_with_timeout(Duration::from_secs(60)).unwrap();
```

关闭行为：
- 停止接受新任务
- 等待所有正在执行的任务完成
- 超时后强制终止剩余任务
- 清理所有资源

完整示例：`examples/graceful_shutdown.rs`

### 3. 错误上下文增强

所有错误都包含详细的执行上下文：

```rust
use execute::{CommandPool, CommandConfig, CommandError};

let pool = CommandPool::new();
let config = CommandConfig::new("nonexistent_command", vec![]);

match pool.execute(&config) {
    Err(CommandError::ExecutionFailed { context, source }) => {
        println!("Task ID: {}", context.task_id);
        println!("Command: {}", context.command);
        println!("Working dir: {:?}", context.working_dir);
        println!("Timestamp: {:?}", context.timestamp);
        println!("Worker ID: {:?}", context.worker_id);
        println!("Error: {}", source);
    }
    Err(CommandError::Timeout { context, configured_timeout, actual_duration }) => {
        println!("Task {} timed out", context.task_id);
        println!("Configured: {:?}, Actual: {:?}", configured_timeout, actual_duration);
    }
    _ => {}
}
```

完整示例：`examples/error_context_demo.rs`

### 4. 指标收集

实时收集和查询任务执行指标：

```rust
use execute::{CommandPool, PoolConfigBuilder};

let pool = PoolConfigBuilder::new()
    .enable_metrics(true)
    .build()
    .unwrap();

// 执行一些任务...

// 获取指标快照
let metrics = pool.metrics();
println!("Tasks submitted: {}", metrics.tasks_submitted);
println!("Tasks completed: {}", metrics.tasks_completed);
println!("Tasks failed: {}", metrics.tasks_failed);
println!("Success rate: {:.2}%", metrics.success_rate * 100.0);
println!("Avg execution time: {:?}", metrics.avg_execution_time);
println!("P95 execution time: {:?}", metrics.p95_execution_time);
println!("P99 execution time: {:?}", metrics.p99_execution_time);
println!("Tasks queued: {}", metrics.tasks_queued);
println!("Tasks running: {}", metrics.tasks_running);
```

可用指标：
- 任务计数：submitted、completed、failed、cancelled
- 当前状态：queued、running
- 执行时间统计：avg、min、max、p50、p95、p99
- 成功率

### 5. 健康检查

监控系统健康状态：

```rust
use execute::{CommandPool, HealthStatus};

let pool = PoolConfigBuilder::new()
    .enable_health_check(true)
    .build()
    .unwrap();

let health = pool.health_check();

match health.status {
    HealthStatus::Healthy => {
        println!("System is healthy");
    }
    HealthStatus::Degraded { issues } => {
        println!("System is degraded:");
        for issue in issues {
            println!("  - {}", issue);
        }
    }
    HealthStatus::Unhealthy { issues } => {
        println!("System is unhealthy:");
        for issue in issues {
            println!("  - {}", issue);
        }
    }
}

// 查看详细信息
println!("Workers alive: {}/{}", health.details.workers_alive, health.details.workers_total);
println!("Queue usage: {:.1}%", health.details.queue_usage * 100.0);
println!("Long running tasks: {}", health.details.long_running_tasks);
```

完整示例：`examples/health_check_demo.rs`

### 6. 错误重试机制

自动重试失败的任务：

```rust
use execute::{CommandConfig, RetryPolicy, RetryStrategy};
use std::time::Duration;

// 固定间隔重试
let retry_policy = RetryPolicy {
    max_attempts: 3,
    strategy: RetryStrategy::FixedInterval(Duration::from_secs(1)),
};

let config = CommandConfig::new("flaky_command", vec![])
    .with_retry(retry_policy);

// 指数退避重试
let retry_policy = RetryPolicy {
    max_attempts: 5,
    strategy: RetryStrategy::ExponentialBackoff {
        initial: Duration::from_millis(100),
        max: Duration::from_secs(10),
        multiplier: 2.0,
    },
};

let config = CommandConfig::new("flaky_command", vec![])
    .with_retry(retry_policy);
```

重试行为：
- 任务失败时自动重试
- 记录每次重试的日志
- 达到最大次数后返回最终错误
- 支持固定间隔和指数退避策略

完整示例：`examples/retry_execution_demo.rs`、`examples/retry_strategy_demo.rs`

### 7. 超时粒度控制

分别控制启动超时和执行超时：

```rust
use execute::{CommandConfig, TimeoutConfig};
use std::time::Duration;

let timeout_config = TimeoutConfig {
    spawn_timeout: Some(Duration::from_secs(5)),      // 启动超时
    execution_timeout: Some(Duration::from_secs(30)), // 执行超时
};

let config = CommandConfig::new("long_running_command", vec![])
    .with_timeouts(timeout_config);
```

超时类型：
- **启动超时**：命令启动过程的超时（spawn）
- **执行超时**：命令执行过程的超时（wait）
- 错误信息会明确区分超时类型

完整示例：`examples/separated_timeout_demo.rs`、`examples/timeout_config_demo.rs`

### 8. 任务取消机制

取消已提交的任务：

```rust
use execute::{CommandPool, CommandConfig};

let pool = CommandPool::new();

// 提交任务并获取句柄
let handle = pool.submit(CommandConfig::new("sleep", vec!["60".to_string()])).unwrap();

// 取消任务
handle.cancel().unwrap();

// 检查取消状态
if handle.is_cancelled() {
    println!("Task was cancelled");
}

// 等待任务完成或取消
match handle.wait() {
    Ok(output) => println!("Task completed"),
    Err(TaskError::Cancelled) => println!("Task was cancelled"),
    Err(e) => println!("Task failed: {}", e),
}
```

取消行为：
- 队列中的任务：从队列中移除
- 执行中的任务：终止进程
- 返回 `Cancelled` 错误

完整示例：`examples/task_cancellation_demo.rs`、`examples/submit_with_handle_demo.rs`

### 9. 环境变量支持

为命令设置自定义环境变量：

```rust
use execute::{CommandConfig, EnvConfig};

// 设置环境变量
let env = EnvConfig::new()
    .set("MY_VAR", "my_value")
    .set("ANOTHER_VAR", "another_value");

let config = CommandConfig::new("my_command", vec![])
    .with_env(env);

// 清除特定环境变量
let env = EnvConfig::new()
    .remove("PATH");

// 不继承父进程环境变量
let env = EnvConfig::new()
    .no_inherit()
    .set("ONLY_VAR", "value");

let config = CommandConfig::new("my_command", vec![])
    .with_env(env);
```

完整示例：`examples/env_config_demo.rs`

### 10. 资源限制

限制命令的资源使用：

```rust
use execute::{CommandConfig, ResourceLimits};

let limits = ResourceLimits {
    max_output_size: Some(1024 * 1024),  // 1 MB
    max_memory: Some(100 * 1024 * 1024), // 100 MB
};

let config = CommandConfig::new("memory_hungry_command", vec![])
    .with_resource_limits(limits);
```

资源限制：
- **输出大小限制**：超过限制时截断输出并记录警告
- **内存限制**：超过限制时终止进程并返回错误

完整示例：`examples/resource_limits_demo.rs`

### 11. 僵尸进程清理

自动清理僵尸进程：

```rust
use execute::PoolConfigBuilder;
use std::time::Duration;

let pool = PoolConfigBuilder::new()
    .zombie_reaper_interval(Duration::from_secs(10))  // 每 10 秒检查一次
    .build()
    .unwrap();

// 僵尸进程会自动清理
// 关闭时也会清理所有剩余的僵尸进程
```

完整示例：`examples/zombie_reaper_demo.rs`

### 12. 性能分析钩子

在任务执行前后插入自定义逻辑：

```rust
use execute::{CommandPool, ExecutionHook, ExecutionContext, TaskResult};
use std::sync::Arc;

struct MyHook;

impl ExecutionHook for MyHook {
    fn before_execute(&self, ctx: &ExecutionContext) {
        println!("Task {} starting: {}", ctx.task_id, ctx.command);
    }
    
    fn after_execute(&self, ctx: &ExecutionContext, result: &TaskResult) {
        println!("Task {} finished in {:?}", ctx.task_id, result.duration);
        if let Some(error) = &result.error {
            println!("Error: {}", error);
        }
    }
}

let pool = CommandPool::new()
    .with_hook(Arc::new(MyHook));
```

钩子用途：
- 性能分析
- 自定义监控
- 日志增强
- 指标收集

完整示例：`examples/hooks_demo.rs`、`examples/hook_demo.rs`

## 配置示例

### 完整配置示例

```rust
use execute::{
    PoolConfigBuilder, LogConfig, LogLevel, LogFormat, LogTarget,
    ShutdownConfig, RetryPolicy, RetryStrategy, TimeoutConfig,
    ResourceLimits, EnvConfig
};
use std::time::Duration;

// 构建命令池配置
let pool = PoolConfigBuilder::new()
    // 基础配置
    .thread_count(8)
    .queue_capacity(1000)
    
    // 日志配置
    .with_log_config(LogConfig {
        level: LogLevel::Info,
        format: LogFormat::Json,
        target: LogTarget::Stdout,
    })
    
    // 关闭配置
    .with_shutdown_config(ShutdownConfig {
        timeout: Duration::from_secs(60),
        force_kill: true,
    })
    
    // 启用功能
    .enable_metrics(true)
    .enable_health_check(true)
    .zombie_reaper_interval(Duration::from_secs(30))
    
    .build()
    .unwrap();

// 配置命令
let config = CommandConfig::new("my_command", vec!["arg1".to_string()])
    // 超时配置
    .with_timeouts(TimeoutConfig {
        spawn_timeout: Some(Duration::from_secs(5)),
        execution_timeout: Some(Duration::from_secs(300)),
    })
    
    // 重试配置
    .with_retry(RetryPolicy {
        max_attempts: 3,
        strategy: RetryStrategy::ExponentialBackoff {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(10),
            multiplier: 2.0,
        },
    })
    
    // 资源限制
    .with_resource_limits(ResourceLimits {
        max_output_size: Some(10 * 1024 * 1024),  // 10 MB
        max_memory: Some(500 * 1024 * 1024),      // 500 MB
    })
    
    // 环境变量
    .with_env(EnvConfig::new()
        .set("LOG_LEVEL", "debug")
        .set("APP_ENV", "production")
    );

// 提交任务
let handle = pool.submit(config).unwrap();
```

### 最小配置示例

```rust
use execute::CommandPool;

// 使用默认配置
let pool = CommandPool::new();
```

### 生产环境推荐配置

```rust
use execute::{PoolConfigBuilder, LogConfig, LogLevel, LogFormat, LogTarget};
use std::time::Duration;

let pool = PoolConfigBuilder::new()
    .thread_count(num_cpus::get())
    .queue_capacity(10000)
    .with_log_config(LogConfig {
        level: LogLevel::Info,
        format: LogFormat::Json,  // 便于日志聚合
        target: LogTarget::Stdout,
    })
    .enable_metrics(true)
    .enable_health_check(true)
    .zombie_reaper_interval(Duration::from_secs(60))
    .build()
    .unwrap();
```

## 迁移指南

### 从 0.1.x 迁移到 0.2.x

#### 1. 构造函数变化

**旧版本**：
```rust
let pool = CommandPool::new();
```

**新版本（兼容）**：
```rust
// 方式 1：使用旧 API（仍然支持）
let pool = CommandPool::new();

// 方式 2：使用新的 builder API（推荐）
let pool = PoolConfigBuilder::new()
    .thread_count(4)
    .build()
    .unwrap();
```

#### 2. 错误处理变化

**旧版本**：
```rust
match pool.execute(&config) {
    Err(e) => println!("Error: {}", e),
    _ => {}
}
```

**新版本**：
```rust
use execute::CommandError;

match pool.execute(&config) {
    Err(CommandError::ExecutionFailed { context, source }) => {
        println!("Task {} failed: {}", context.task_id, source);
        println!("Command: {}", context.command);
    }
    Err(CommandError::Timeout { context, configured_timeout, actual_duration }) => {
        println!("Task {} timed out", context.task_id);
    }
    _ => {}
}
```

#### 3. 关闭机制变化

**旧版本**：
```rust
// 没有优雅关闭机制
drop(pool);
```

**新版本**：
```rust
// 优雅关闭
pool.shutdown().unwrap();

// 或指定超时
pool.shutdown_with_timeout(Duration::from_secs(30)).unwrap();
```

#### 4. 新增功能（可选）

以下功能是新增的，不影响现有代码：

```rust
// 启用指标收集
let pool = PoolConfigBuilder::new()
    .enable_metrics(true)
    .build()
    .unwrap();

let metrics = pool.metrics();

// 启用健康检查
let pool = PoolConfigBuilder::new()
    .enable_health_check(true)
    .build()
    .unwrap();

let health = pool.health_check();

// 使用任务句柄
let handle = pool.submit(config).unwrap();
handle.cancel().unwrap();
```

#### 5. 配置验证

新版本会在构造时验证配置参数：

```rust
// 这会返回错误而不是 panic
let result = PoolConfigBuilder::new()
    .thread_count(0)  // 无效
    .build();

match result {
    Err(ConfigError::InvalidThreadCount(0)) => {
        println!("Invalid thread count");
    }
    _ => {}
}
```

### 向后兼容性

所有旧 API 都保持兼容，新功能通过以下方式添加：

- **可选配置**：新功能默认禁用
- **Builder 模式**：新配置通过 builder 添加
- **保留旧 API**：`CommandPool::new()` 等方法仍然可用
- **错误类型扩展**：使用 `#[non_exhaustive]` 确保兼容性

### 性能影响

- **未启用功能**：零运行时开销（编译时优化）
- **日志**：使用 `tracing` 的零成本抽象
- **指标**：使用原子操作，开销极小（< 1%）
- **健康检查**：按需调用，不影响任务执行
- **钩子**：仅在配置时有开销

### 推荐升级步骤

1. **更新依赖**：
   ```toml
   [dependencies]
   execute = "0.2"
   tracing = "0.1"
   tracing-subscriber = "0.3"
   ```

2. **运行测试**：确保现有代码正常工作

3. **逐步启用新功能**：
   - 先启用日志和错误上下文
   - 然后启用指标和健康检查
   - 最后根据需要启用高级功能

4. **更新错误处理**：利用新的错误上下文信息

5. **添加监控**：集成指标和健康检查到监控系统

## 更多示例与文档

### 项目文档

- **[EXECUTOR_CUSTOM.md](EXECUTOR_CUSTOM.md)** - 详细使用指南和高级功能说明
- **[TESTING.md](TESTING.md)** - 测试文档，包含测试分类和运行方式
- **[CONTRIBUTING.md](CONTRIBUTING.md)** - 贡献指南
- **[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)** - 行为准则
- **[SECURITY.md](SECURITY.md)** - 安全政策

### 示例程序

所有示例都在 `examples/` 目录下：

#### 核心功能
- **Tokio 集成**：`examples/tokio_integration.rs`
- **自定义执行器**：见 [EXECUTOR_CUSTOM.md](EXECUTOR_CUSTOM.md)

#### 可观测性
- **结构化日志**：`examples/logging_demo.rs`
- **指标收集**：查看 `CommandPool::metrics()` 使用
- **健康检查**：`examples/health_check_demo.rs`
- **性能分析钩子**：`examples/hooks_demo.rs`、`examples/hook_demo.rs`

#### 可靠性
- **优雅关闭**：`examples/graceful_shutdown.rs`
- **错误上下文**：`examples/error_context_demo.rs`
- **僵尸进程清理**：`examples/zombie_reaper_demo.rs`

#### 高级功能
- **错误重试**：`examples/retry_execution_demo.rs`、`examples/retry_strategy_demo.rs`、`examples/retry_integration_demo.rs`
- **超时控制**：`examples/separated_timeout_demo.rs`、`examples/timeout_config_demo.rs`
- **任务取消**：`examples/task_cancellation_demo.rs`、`examples/submit_with_handle_demo.rs`
- **环境变量**：`examples/env_config_demo.rs`
- **资源限制**：`examples/resource_limits_demo.rs`

### 运行示例

```bash
# 运行日志示例
cargo run --example logging_demo

# 运行健康检查示例
cargo run --example health_check_demo

# 运行重试示例
cargo run --example retry_execution_demo
```

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                      CommandPool API                         │
│  (submit, shutdown, health_check, metrics)                   │
└────────────────────┬────────────────────────────────────────┘
                     │
        ┌────────────┴────────────┐
        │                         │
┌───────▼────────┐       ┌───────▼────────┐
│  CommandPool   │       │ CommandPoolSeg │
│  (Mutex-based) │       │ (Lock-free)    │
└───────┬────────┘       └───────┬────────┘
        │                        │
        └────────────┬───────────┘
                     │
        ┌────────────┴───────────────────────────────────┐
        │                                                │
┌───────▼────────┐  ┌──────────────┐  ┌───────────────▼──┐
│  Task Queue    │  │   Metrics    │  │  Health Monitor  │
│                │  │  Collector   │  │                  │
└───────┬────────┘  └──────┬───────┘  └───────┬──────────┘
        │                  │                   │
┌───────▼──────────────────▼───────────────────▼──────────┐
│              Worker Thread Pool                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ Worker 1 │  │ Worker 2 │  │ Worker N │              │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘              │
└───────┼─────────────┼─────────────┼────────────────────┘
        │             │             │
┌───────▼─────────────▼─────────────▼────────────────────┐
│              Tracing & Logging Layer                    │
│  (structured logs, spans, events)                       │
└─────────────────────────────────────────────────────────┘
```

### 核心组件

1. **CommandPool/CommandPoolSeg**：主入口，负责任务调度和生命周期管理
2. **Task Queue**：存储待执行任务，支持优先级和取消
3. **Worker Thread Pool**：执行任务的工作线程池
4. **Metrics Collector**：收集和聚合运行时指标
5. **Health Monitor**：监控系统健康状态
6. **Tracing Layer**：提供结构化日志和追踪

## 最佳实践

### 1. 生产环境配置

```rust
use execute::{PoolConfigBuilder, LogConfig, LogLevel, LogFormat, LogTarget};
use std::time::Duration;

let pool = PoolConfigBuilder::new()
    // 使用系统 CPU 核心数
    .thread_count(num_cpus::get())
    
    // 设置合理的队列容量
    .queue_capacity(10000)
    
    // 使用 JSON 格式日志便于聚合
    .with_log_config(LogConfig {
        level: LogLevel::Info,
        format: LogFormat::Json,
        target: LogTarget::Stdout,
    })
    
    // 启用监控功能
    .enable_metrics(true)
    .enable_health_check(true)
    
    // 定期清理僵尸进程
    .zombie_reaper_interval(Duration::from_secs(60))
    
    .build()
    .unwrap();
```

### 2. 错误处理

```rust
use execute::{CommandError, TaskError};

// 详细的错误处理
match pool.submit(config) {
    Ok(handle) => {
        match handle.wait() {
            Ok(output) => {
                // 处理成功结果
            }
            Err(TaskError::Cancelled) => {
                // 任务被取消
            }
            Err(TaskError::Failed(CommandError::Timeout { context, .. })) => {
                // 超时处理
                log::error!("Task {} timed out", context.task_id);
            }
            Err(e) => {
                // 其他错误
                log::error!("Task failed: {}", e);
            }
        }
    }
    Err(e) => {
        // 提交失败
        log::error!("Failed to submit task: {}", e);
    }
}
```

### 3. 监控集成

```rust
use std::time::Duration;
use std::thread;

// 定期收集指标
thread::spawn(move || {
    loop {
        let metrics = pool.metrics();
        
        // 发送到监控系统（Prometheus、Datadog 等）
        send_metric("tasks.submitted", metrics.tasks_submitted);
        send_metric("tasks.completed", metrics.tasks_completed);
        send_metric("tasks.failed", metrics.tasks_failed);
        send_metric("tasks.success_rate", metrics.success_rate);
        send_metric("tasks.avg_duration_ms", metrics.avg_execution_time.as_millis());
        send_metric("tasks.p95_duration_ms", metrics.p95_execution_time.as_millis());
        
        thread::sleep(Duration::from_secs(10));
    }
});

// 定期健康检查
thread::spawn(move || {
    loop {
        let health = pool.health_check();
        
        match health.status {
            HealthStatus::Unhealthy { .. } => {
                // 发送告警
                send_alert("CommandPool is unhealthy");
            }
            HealthStatus::Degraded { .. } => {
                // 发送警告
                send_warning("CommandPool is degraded");
            }
            _ => {}
        }
        
        thread::sleep(Duration::from_secs(30));
    }
});
```

### 4. 资源管理

```rust
use execute::{CommandConfig, ResourceLimits, TimeoutConfig};
use std::time::Duration;

// 为不受信任的命令设置严格限制
let config = CommandConfig::new("untrusted_command", vec![])
    .with_resource_limits(ResourceLimits {
        max_output_size: Some(1024 * 1024),      // 1 MB
        max_memory: Some(100 * 1024 * 1024),     // 100 MB
    })
    .with_timeouts(TimeoutConfig {
        spawn_timeout: Some(Duration::from_secs(5)),
        execution_timeout: Some(Duration::from_secs(30)),
    });
```

### 5. 优雅关闭

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

let pool = Arc::new(CommandPool::new());
let shutdown_flag = Arc::new(AtomicBool::new(false));

// 注册信号处理器
let pool_clone = pool.clone();
let shutdown_flag_clone = shutdown_flag.clone();
ctrlc::set_handler(move || {
    println!("Received shutdown signal");
    shutdown_flag_clone.store(true, Ordering::SeqCst);
    
    // 优雅关闭
    if let Err(e) = pool_clone.shutdown_with_timeout(Duration::from_secs(60)) {
        eprintln!("Shutdown error: {}", e);
    }
}).unwrap();

// 主循环
while !shutdown_flag.load(Ordering::SeqCst) {
    // 处理任务...
}
```

## 性能优化

### 1. 选择合适的队列实现

```rust
// 高并发场景：使用无锁队列
use execute::CommandPoolSeg;
let pool = CommandPoolSeg::new(num_cpus::get());

// 一般场景：使用标准队列
use execute::CommandPool;
let pool = CommandPool::new();
```

### 2. 调整线程数

```rust
// CPU 密集型任务
let pool = PoolConfigBuilder::new()
    .thread_count(num_cpus::get())
    .build()
    .unwrap();

// I/O 密集型任务
let pool = PoolConfigBuilder::new()
    .thread_count(num_cpus::get() * 2)
    .build()
    .unwrap();
```

### 3. 批量操作

```rust
// 批量提交任务
let tasks: Vec<_> = (0..1000)
    .map(|i| CommandConfig::new("echo", vec![format!("task{}", i)]))
    .collect();

let count = pool.push_tasks_batch(tasks);
```

### 4. 禁用不需要的功能

```rust
// 最小配置，最佳性能
let pool = CommandPool::new();

// 仅启用需要的功能
let pool = PoolConfigBuilder::new()
    .enable_metrics(true)  // 仅启用指标
    .build()
    .unwrap();
```

## 安全考虑

### 1. 输入验证

```rust
// 验证命令参数
fn validate_command(cmd: &str) -> Result<(), ValidationError> {
    if cmd.contains("..") || cmd.contains(";") {
        return Err(ValidationError::UnsafeCommand);
    }
    Ok(())
}
```

### 2. 资源限制

```rust
// 为所有外部命令设置资源限制
let config = CommandConfig::new("external_command", vec![])
    .with_resource_limits(ResourceLimits {
        max_output_size: Some(10 * 1024 * 1024),
        max_memory: Some(500 * 1024 * 1024),
    })
    .with_timeouts(TimeoutConfig {
        spawn_timeout: Some(Duration::from_secs(5)),
        execution_timeout: Some(Duration::from_secs(300)),
    });
```

### 3. 日志脱敏

```rust
// 避免记录敏感信息
let config = CommandConfig::new("mysql", vec![
    "-u", "user",
    "-p", "****",  // 不记录密码
]);
```

## 故障排查

### 1. 任务执行缓慢

检查指标：
```rust
let metrics = pool.metrics();
println!("Queue size: {}", metrics.tasks_queued);
println!("Running tasks: {}", metrics.tasks_running);
println!("P95 duration: {:?}", metrics.p95_execution_time);
```

可能原因：
- 线程数不足
- 任务执行时间过长
- 队列积压

### 2. 内存使用过高

检查：
- 队列容量是否过大
- 是否有内存泄漏
- 输出大小是否受限

解决方案：
```rust
let pool = PoolConfigBuilder::new()
    .queue_capacity(1000)  // 限制队列大小
    .build()
    .unwrap();

let config = CommandConfig::new("command", vec![])
    .with_resource_limits(ResourceLimits {
        max_output_size: Some(1024 * 1024),  // 限制输出
        max_memory: Some(100 * 1024 * 1024), // 限制内存
    });
```

### 3. 僵尸进程

启用自动清理：
```rust
let pool = PoolConfigBuilder::new()
    .zombie_reaper_interval(Duration::from_secs(30))
    .build()
    .unwrap();
```

### 4. 健康检查失败

```rust
let health = pool.health_check();
match health.status {
    HealthStatus::Degraded { issues } | HealthStatus::Unhealthy { issues } => {
        for issue in issues {
            println!("Issue: {}", issue);
        }
    }
    _ => {}
}
```

## 自定义执行器

实现 `CommandExecutor` trait 即可将自定义执行器注入到命令池：

```rust
use execute::{CommandExecutor, CommandConfig, ExecuteError};
use std::process::Output;

struct MyExecutor;

impl CommandExecutor for MyExecutor {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        // 自定义执行逻辑
        std::process::Command::new(&config.program)
            .args(&config.args)
            .output()
            .map_err(ExecuteError::Io)
    }
}
```

详细示例与指南请见 [自定义执行器文档](EXECUTOR_CUSTOM.md)。

## 贡献
----

欢迎提交 issue/PR。请确保代码风格一致并包括必要的测试用例。

## 许可
---

该仓库采用 MIT 许可证，详见 `LICENSE`。
