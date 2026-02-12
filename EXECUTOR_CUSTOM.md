# 自定义执行器指南 | Custom Executor Guide

本文档介绍如何为命令池创建和使用自定义执行器，实现对不同运行时的支持。

## 概述 | Overview

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
