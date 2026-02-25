// 在 docs.rs 上显示 feature 标志
#![cfg_attr(docsrs, feature(doc_cfg))]

mod backend;
mod batch_executor;
mod config;
mod env_optimizer;
mod error;
mod executor;
mod warm_pool;
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
mod zombie_reaper;

// Re-export 标准库类型（在公共 API 中使用）
pub use std::sync::mpsc::Sender;
pub use std::time::Duration;

// Re-export 外部库类型（在公共 API 中使用）
pub use thiserror::Error;

pub use backend::{ExecutionBackend, ExecutionConfig, ExecutionMode};
pub use batch_executor::{
    execute_batch_detailed, execute_parallel_batch, execute_sequential_batch, BatchConfig,
    BatchOutput, IndividualOutput,
};
pub use env_optimizer::{apply_env_config_optimized, EnvCache, EnvOptimizer};
pub use warm_pool::{WarmExecutor, WarmProcessPool};
pub use config::{
    CommandConfig, EnvConfig, PoolConfig, PoolConfigBuilder, ResourceLimits, RetryPolicy,
    RetryStrategy, ShutdownConfig, TimeoutConfig,
};
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
#[cfg(feature = "logging")]
#[cfg_attr(docsrs, doc(cfg(feature = "logging")))]
pub use logging::{LogConfig, LogFormat, LogLevel, LogTarget};
#[cfg(feature = "metrics")]
#[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
pub use metrics::{Metrics, MetricsSnapshot};
#[cfg(feature = "iouring")]
#[cfg_attr(docsrs, doc(cfg(feature = "iouring")))]
pub use iouring_executor::{execute_batch_iouring, IoUringExecutor};
#[cfg(feature = "pipeline")]
#[cfg_attr(docsrs, doc(cfg(feature = "pipeline")))]
pub use pipeline::{Pipeline, PipelineExecutor, PipelineStage};
pub use pool::{CommandPool, TaskItem};
pub use process_pool::ProcessPool;
pub use semaphore::{Semaphore, SemaphoreGuard};
pub use task_handle::{CancellationToken, TaskHandle, TaskResult, TaskState, TaskWithResult};
pub use task_status::{TaskIdGenerator, TaskStatus, TaskStatusTracker};
pub use zombie_reaper::ZombieReaper;
