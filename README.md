# execute

轻量的命令池库（Rust）

- 支持多线程生产/消费命令队列（`CommandPool`）
- 支持无锁队列变体（`CommandPoolSeg`，基于 `crossbeam_queue::SegQueue`）
- 支持超时、安全的子进程等待（使用 `wait-timeout`）
- 可扩展的执行器接口 `CommandExecutor`，可集成 `tokio` 等异步运行时
- 提供锁/无锁、线程池、并发限制等多种执行模式

Usage
-----

快速示例（使用标准库执行器）：

```rust
use execute::{CommandPool, CommandConfig};
use std::time::Duration;

let pool = CommandPool::new();
pool.push_task(CommandConfig::new("echo", vec!["hello".to_string()]));
pool.start_executor(Duration::from_millis(100));
```

自定义执行器
--------------

库提供 `CommandExecutor` trait，你可以实现它，将自定义执行器（例如基于 `tokio` 的异步执行器）注入到池中：

```rust
// 伪代码示例
struct MyExecutor; // impl CommandExecutor for MyExecutor { ... }
let exec = std::sync::Arc::new(MyExecutor);
pool.start_executor_with_executor(Duration::from_millis(100), exec);
```

文档
----

已生成本 crate 的文档： `target/doc/execute/index.html`。

License
-------

（在此处填写许可证信息，例如 MIT/Apache-2.0）
