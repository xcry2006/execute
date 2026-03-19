//! # execute
//!
//! 生产环境就绪的 Rust 命令池库 — 提供可插拔的命令执行器、锁/无锁任务队列、线程池与并发限制策略，
//! 以及完整的可观测性和可靠性特性。
//!
//! ## 快速开始
//!
//! ```rust,no_run
//! use execute::{CommandPool, CommandConfig};
//!
//! // 创建命令池
//! let pool = CommandPool::new();
//!
//! // 提交任务
//! pool.push_task(CommandConfig::new("echo", vec!["Hello, World!".to_string()]));
//!
//! // 启动执行器
//! pool.start_executor();
//!
//! // 优雅关闭
//! pool.shutdown().unwrap();
//! ```
//!
//! ## 主要特性
//!
//! ### 核心功能
//! - **多线程安全的任务队列**：`CommandPool`（基于 `Mutex<VecDeque>`）
//! - **无锁队列变体**：`CommandPoolSeg`（基于 `crossbeam_queue::SegQueue`）
//! - **可扩展执行器接口**：`CommandExecutor`（可集成 tokio / async-std）
//! - **子进程超时与安全等待**：使用 `wait-timeout` 避免额外等待线程
//! - **线程池、并发限制**（信号量）和多种执行模式
//! - **执行器停止机制**：优雅关闭执行器线程
//! - **队列大小限制**：支持有界队列，防止内存无限增长
//! - **批量操作接口**：批量提交任务，提高吞吐量
//! - **任务状态查询**：追踪任务状态（Pending/Running/Completed/Failed）
//! - **任务结果获取**：异步获取任务执行结果（TaskHandle）
//! - **真正的进程池**：常驻子进程池，通过 IPC 通信执行命令
//! - **Pipeline 支持**：命令管道，支持链式执行多个命令
//!
//! ### 生产环境特性
//!
//! #### 可观测性
//! - **结构化日志**：基于 `tracing` 的结构化日志，支持 JSON/Pretty/Compact 格式
//! - **指标收集**：实时收集任务执行指标（成功率、执行时间、百分位数等）
//! - **健康检查**：监控系统健康状态
//! - **性能分析钩子**：在任务执行前后插入自定义逻辑
//!
//! #### 可靠性
//! - **优雅关闭**：确保正在执行的任务完成后再关闭
//! - **错误上下文增强**：详细的错误信息，包含完整执行上下文
//! - **配置参数验证**：在构造时验证所有配置参数
//! - **僵尸进程清理**：自动清理僵尸进程，避免资源泄漏
//!
//! #### 高级功能
//! - **错误重试机制**：支持固定间隔和指数退避重试策略
//! - **超时粒度控制**：分离启动超时和执行超时
//! - **任务取消机制**：支持取消队列中或执行中的任务
//! - **环境变量支持**：为命令设置自定义环境变量
//! - **资源限制**：限制命令输出大小和内存使用
//!
//! ## Feature 标志
//!
//! 本库使用 Cargo features 进行模块化组织：
//!
//! | Feature | 依赖 | 说明 | 默认启用 |
//! |---------|------|------|----------|
//! | `logging` | `tracing`, `tracing-subscriber` | 结构化日志支持 | ✅ |
//! | `metrics` | `hdrhistogram` | 指标收集 | ✅ |
//! | `health` | 无 | 健康检查接口 | ✅ |
//! | `pipeline` | 无 | 命令管道支持 | ✅ |
//! | `minimal` | 无 | 仅核心功能 | ❌ |
//! | `full` | 全部 | 启用所有功能 | ❌ |
//! | `iouring` | `io-uring`, `slab` | io_uring 异步 I/O（Linux 5.1+） | ❌ |
//!
//! ## 示例程序
//!
//! 更多示例请查看 `examples/` 目录：
//!
//! ```bash
//! # 运行日志示例
//! cargo run --example logging_demo
//!
//! # 运行健康检查示例
//! cargo run --example health_check_demo
//!
//! # 运行重试示例
//! cargo run --example retry_execution_demo
//! ```
//!
//! ## 完整文档
//!
//! - [README.md](../README.md) - 详细使用指南
//! - [EXECUTOR_CUSTOM.md](../EXECUTOR_CUSTOM.md) - 自定义执行器指南
//! - [TESTING.md](../TESTING.md) - 测试文档

// 在 docs.rs 上显示 feature 标志
#![cfg_attr(docsrs, feature(doc_cfg))]

mod backend;
mod batch_executor;
mod config;
mod env_optimizer;
mod error;
mod executor;
#[cfg(feature = "health")]
#[cfg_attr(docsrs, doc(cfg(feature = "health")))]
mod health;
mod hooks;
#[cfg(feature = "iouring")]
#[cfg_attr(docsrs, doc(cfg(feature = "iouring")))]
mod iouring_executor;
#[cfg(feature = "logging")]
#[cfg_attr(docsrs, doc(cfg(feature = "logging")))]
mod logging;
#[cfg(feature = "metrics")]
#[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
mod metrics;
#[cfg(feature = "pipeline")]
#[cfg_attr(docsrs, doc(cfg(feature = "pipeline")))]
mod pipeline;
mod pool;
pub mod prelude;
mod process_pool;
mod semaphore;
mod task_handle;
mod task_status;
mod warm_pool;
mod zombie_reaper;

// Re-export 标准库类型（在公共 API 中使用）
pub use std::sync::mpsc::Sender;
pub use std::time::Duration;

// Re-export 外部库类型（在公共 API 中使用）
pub use thiserror::Error;

pub use backend::{ExecutionBackend, ExecutionConfig, ExecutionMode};
pub use batch_executor::{
    BatchConfig, BatchOutput, IndividualOutput, execute_batch_detailed, execute_parallel_batch,
    execute_sequential_batch,
};
pub use config::{
    CommandConfig, EnvConfig, PoolConfig, PoolConfigBuilder, ResourceLimits, RetryPolicy,
    RetryStrategy, ShutdownConfig, TimeoutConfig,
};
pub use env_optimizer::{EnvCache, EnvOptimizer, apply_env_config_optimized};
pub use error::{
    CancelError, CommandError, ConfigError, ErrorContext, ExecuteError, ShutdownError, SubmitError,
};
pub use executor::{
    CommandExecutor, StdCommandExecutor, apply_env_config, execute_command_with_context,
    execute_task_with_hooks, execute_with_retry, execute_with_timeouts,
};
#[cfg(feature = "health")]
#[cfg_attr(docsrs, doc(cfg(feature = "health")))]
pub use health::{HealthCheck, HealthDetails, HealthStatus};
pub use hooks::{ExecutionContext, ExecutionHook, HookTaskResult};
#[cfg(feature = "iouring")]
#[cfg_attr(docsrs, doc(cfg(feature = "iouring")))]
pub use iouring_executor::{IoUringExecutor, execute_batch_iouring};
#[cfg(feature = "logging")]
#[cfg_attr(docsrs, doc(cfg(feature = "logging")))]
pub use logging::{LogConfig, LogFormat, LogLevel, LogTarget};
#[cfg(feature = "metrics")]
#[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
pub use metrics::{Metrics, MetricsSnapshot};
#[cfg(feature = "pipeline")]
#[cfg_attr(docsrs, doc(cfg(feature = "pipeline")))]
pub use pipeline::{Pipeline, PipelineExecutor, PipelineStage};
pub use pool::{CommandPool, TaskItem};
pub use process_pool::ProcessPool;
pub use semaphore::{Semaphore, SemaphoreGuard};
pub use task_handle::{CancellationToken, TaskHandle, TaskResult, TaskState, TaskWithResult};
pub use task_status::{TaskIdGenerator, TaskStatus, TaskStatusTracker};
pub use warm_pool::{WarmExecutor, WarmProcessPool};
pub use zombie_reaper::ZombieReaper;
