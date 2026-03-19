# 高级功能指南

本文档详细介绍 execute 库的高级功能，包括自定义执行器、健康检查、重试机制、超时控制等。

## 目录

1. [自定义执行器](#自定义执行器)
2. [健康检查](#健康检查)
3. [重试机制](#重试机制)
4. [超时控制](#超时控制)
5. [环境变量](#环境变量)
6. [钩子系统](#钩子系统)
7. [僵尸进程清理](#僵尸进程清理)
8. [指标收集](#指标收集)
9. [任务取消](#任务取消)
10. [资源限制](#资源限制)

---

## 自定义执行器

`CommandExecutor` trait 允许你实现自己的命令执行逻辑，比如集成异步运行时（Tokio）或添加自定义行为。

### 基本接口

```rust
pub trait CommandExecutor: Send + Sync {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError>;
}
```

### 示例 1: 使用标准库执行器

这是默认的执行器，使用 `std::process::Command`：

```rust
use execute::{CommandPool, CommandConfig, StdCommandExecutor};
use std::sync::Arc;

let pool = CommandPool::new();
let executor = Arc::new(StdCommandExecutor);

// 提交任务
pool.push_task(CommandConfig::new("echo", vec!["hello".to_string()]));

// 启动执行器（使用自定义执行器）
pool.start_executor_with_executor(std::time::Duration::from_millis(100), executor);
```

### 示例 2: 集成 Tokio 异步运行时

在同步接口中使用异步功能：

```rust
use execute::{CommandConfig, ExecuteError, CommandExecutor};
use std::process::Output;
use tokio::process::Command;

pub struct TokioExecutor;

impl CommandExecutor for TokioExecutor {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        // 创建运行时来执行异步代码
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| ExecuteError::Io(std::io::Error::new(
                std::io::ErrorKind::Other, e
            )))?;
        
        rt.block_on(async {
            let mut cmd = Command::new(&config.program);
            cmd.args(&config.args);
            
            if let Some(dir) = &config.working_dir {
                cmd.current_dir(dir);
            }
            
            // 设置环境变量
            for (key, value) in &config.env.vars {
                if let Some(val) = value {
                    cmd.env(key, val);
                } else {
                    cmd.env_remove(key);
                }
            }
            
            cmd.output().await.map_err(ExecuteError::Io)
        })
    }
}
```

**使用方式**：

```rust
let pool = CommandPool::new();
let executor = Arc::new(TokioExecutor);

pool.push_task(CommandConfig::new("sleep", vec!["1".to_string()]));
pool.start_executor_with_executor(std::time::Duration::from_millis(100), executor);
```

### 示例 3: 带超时控制的执行器

```rust
use execute::{CommandConfig, ExecuteError, CommandExecutor};
use std::process::Output;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub struct TimeoutExecutor;

impl CommandExecutor for TimeoutExecutor {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| ExecuteError::Io(std::io::Error::new(
                std::io::ErrorKind::Other, e
            )))?;
        
        rt.block_on(async {
            let mut cmd = Command::new(&config.program);
            cmd.args(&config.args);
            
            // 应用超时
            match config.timeout {
                Some(dur) => {
                    timeout(dur, cmd.output()).await
                        .map_err(|_| ExecuteError::Timeout(dur))?
                        .map_err(|e| ExecuteError::Io(e))
                }
                None => {
                    cmd.output().await.map_err(ExecuteError::Io)
                }
            }
        })
    }
}
```

### 性能优化建议

1. **避免重复创建运行时**：使用线程本地存储或全局运行时
  
   ```rust
   use once_cell::sync::Lazy;
   
   static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
       tokio::runtime::Runtime::new().unwrap()
   });
   
   impl CommandExecutor for YourExecutor {
       fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
           RUNTIME.block_on(async { /* ... */ })
       }
   }
   ```

2. **并发限制**：使用信号量防止资源耗尽

   ```rust
   pool.start_executor_with_executor_and_limit(
       Duration::from_millis(100),
       8,  // 8 个工作线程
       4,  // 最多同时执行 4 个外部进程
       executor,
   );
   ```

3. **选择合适的队列**：
   - 高并发场景：使用无锁队列 `CommandPoolSeg`
   - 一般场景：使用标准队列 `CommandPool`

   ```rust
   use execute::CommandPoolSeg;
   
   let pool = CommandPoolSeg::new();
   let executor = Arc::new(YourExecutor);
   pool.start_executor_with_executor(Duration::from_millis(100), executor);
   ```

### 完整示例

```rust
use execute::{CommandPool, CommandConfig, CommandExecutor, ExecuteError};
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::Duration;

struct CustomExecutor;

impl CommandExecutor for CustomExecutor {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        println!("执行：{} {:?}", config.program(), config.args());
        
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
    pool.push_task(CommandConfig::new("date", vec![]));
    
    // 启动执行器：4 个工作线程，最多 2 个并发
    pool.start_executor_with_executor_and_limit(
        Duration::from_millis(100),
        4,
        2,
        executor,
    );
    
    // 等待任务完成
    std::thread::sleep(Duration::from_secs(1));
    
    // 优雅关闭
    pool.shutdown().unwrap();
    
    Ok(())
}
```

运行此示例：
```bash
cargo run --example custom_executor_demo
```

---

## 健康检查

健康检查功能用于监控命令池的运行状态，适合集成到监控系统中。

### 健康状态分类

```rust
pub enum HealthStatus {
    Healthy,                           // 系统健康
    Degraded { issues: Vec<String> },  // 系统降级（部分问题）
    Unhealthy { issues: Vec<String> }, // 系统异常（严重问题）
}
```

### 使用示例

```rust
use execute::{CommandPool, PoolConfigBuilder, HealthStatus};
use std::time::Duration;

// 创建带健康检查的命令池
let pool = PoolConfigBuilder::new()
    .thread_count(4)
    .enable_health_check(true)
    .build()
    .unwrap();

// 执行一些任务
for i in 0..10 {
    pool.push_task(CommandConfig::new("echo", vec![format!("task {}", i)]));
}
pool.start_executor();

// 检查健康状态
let health = pool.health_check();

match health.status {
    HealthStatus::Healthy => {
        println!("✅ 系统健康运行");
    }
    HealthStatus::Degraded { issues } => {
        println!("⚠️  系统性能下降:");
        for issue in issues {
            println!("   - {}", issue);
        }
    }
    HealthStatus::Unhealthy { issues } => {
        println!("❌ 系统异常:");
        for issue in issues {
            println!("   - {}", issue);
        }
    }
}

// 查看详细指标
println!("\n详细指标:");
println!("  活跃工作线程：{}/{}", 
    health.details.workers_alive,
    health.details.workers_total
);
println!("  队列使用率：{:.1}%", 
    health.details.queue_usage * 100.0
);
println!("  长时任务数：{}", 
    health.details.long_running_tasks
);
```

### 健康判定标准

系统会自动检测以下问题：

**降级状态 (Degraded)**:
- 工作线程死亡超过 25%
- 队列使用率超过 80%
- 存在运行超过 5 分钟的任务

**异常状态 (Unhealthy)**:
- 工作线程死亡超过 50%
- 队列已满，无法提交新任务
- 多个关键组件失败

### 集成到监控系统

```rust
use std::thread;
use std::time::Duration;

// 定期健康检查
thread::spawn(move || {
    loop {
        let health = pool.health_check();
        
        match health.status {
            HealthStatus::Unhealthy { issues } => {
                // 发送紧急告警
                send_alert(&format!("命令池异常：{:?}", issues));
            }
            HealthStatus::Degraded { issues } => {
                // 发送警告
                send_warning(&format!("命令池降级：{:?}", issues));
            }
            _ => {}
        }
        
        thread::sleep(Duration::from_secs(30));
    }
});
```

完整示例：`examples/health_check_demo.rs`

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
pool.start_executor();

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
