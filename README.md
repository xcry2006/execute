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

快速开始
---------

添加依赖（`Cargo.toml`）:

```toml
[dependencies]
crossbeam-queue = "0.3"
wait-timeout = "0.2"
thiserror = "1.0"
```

示例（使用标准库执行器）:

```rust
use execute::{CommandPool, CommandConfig};
use std::time::Duration;

let pool = CommandPool::new();
pool.push_task(CommandConfig::new("echo", vec!["hello".to_string()]));
pool.start_executor(Duration::from_millis(100));
```

自定义执行器
--------------

实现 `CommandExecutor` trait 即可将自定义执行器注入到命令池（例如在 `tokio` 里创建异步执行逻辑并在同步 trait 中 `block_on`，或在专用线程中运行运行时）。示例请见 `EXECUTOR_CUSTOM.md`。

文档
----

本地生成：

```bash
cargo doc --no-deps
xdg-open target/doc/execute/index.html
```

贡献
----

欢迎提交 issue/PR。请确保代码风格一致并包括必要的测试用例。

许可
---

该仓库采用 MIT 许可证，详见 `LICENSE`。

----

如果你希望在发布到 crates.io 前我帮你生成 `Cargo.toml` 的 badge、添加 GitHub Actions、或写一个发布说明，我可以继续帮你。
