# execute

![crates.io](https://img.shields.io/crates/v/execute?label=crate)
![license](https://img.shields.io/badge/license-MIT-blue)

轻量的命令池库（Rust） — 提供可插拔的命令执行器、锁/无锁任务队列、线程池与并发限制策略。

主要特性
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

快速开始
---------

添加依赖（`Cargo.toml`）:

```toml
[dependencies]
execute = "0.1"
crossbeam-queue = "0.3"
wait-timeout = "0.2"
thiserror = "2.0.17"
```

示例（使用标准库执行器）:

```rust
use execute::{CommandPool, CommandConfig};
use std::time::Duration;

let pool = CommandPool::new();
pool.push_task(CommandConfig::new("echo", vec!["hello".to_string()]));
pool.start_executor(Duration::from_millis(100));
```

示例（带队列大小限制）:

```rust
use execute::{CommandPool, CommandConfig, ExecutionConfig};

let config = ExecutionConfig::new();
let pool = CommandPool::with_config_and_limit(config, 100); // 最多 100 个任务
pool.push_task(CommandConfig::new("echo", vec!["task1".to_string()]));
```

示例（批量提交任务）:

```rust
use execute::{CommandPool, CommandConfig};

let pool = CommandPool::new();
let tasks: Vec<_> = (0..10)
    .map(|i| CommandConfig::new("echo", vec![format!("task{}", i)]))
    .collect();
let count = pool.push_tasks_batch(tasks);
```

示例（使用任务状态追踪）:

```rust
use execute::{TaskStatusTracker, TaskStatus, TaskIdGenerator};

let tracker = TaskStatusTracker::new();
let id_gen = TaskIdGenerator::new();

let task_id = id_gen.next_id();
tracker.register(task_id);
tracker.update(task_id, TaskStatus::Running);

let status = tracker.get(task_id);
let pending_count = tracker.count_by_status(TaskStatus::Pending);
```

示例（使用任务结果获取）:

```rust
use execute::{TaskHandle, TaskWithResult};

let (task, handle) = TaskWithResult::new(1);
// 在另一个线程中执行任务并发送结果
task.send_result(Ok(output));

// 等待结果
let result = handle.wait();
// 或尝试非阻塞获取
if let Ok(Some(output)) = handle.try_get() {
    // 任务已完成
}
```

示例（使用进程池）:

```rust
use execute::ProcessPool;

let pool = ProcessPool::new(4).unwrap(); // 4 个工作进程
let output = pool.execute(&CommandConfig::new("echo", vec!["hello".to_string()])).unwrap();
```

更多示例
--------

- Tokio 集成与超时控制示例：见 `examples/tokio_integration.rs`

自定义执行器
--------------

实现 `CommandExecutor` trait 即可将自定义执行器注入到命令池（例如在 `tokio` 里创建异步执行逻辑并在同步 trait 中 `block_on`，或在专用线程中运行运行时）。

详细示例与指南请见 [自定义执行器文档](EXECUTOR_CUSTOM.md)。

贡献
----

欢迎提交 issue/PR。请确保代码风格一致并包括必要的测试用例。

许可
---

该仓库采用 MIT 许可证，详见 `LICENSE`。
