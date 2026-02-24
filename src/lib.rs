mod backend;
mod config;
mod error;
mod executor;
mod health;
mod hooks;
mod logging;
mod metrics;
mod pipeline;
mod pool;
mod pool_seg;
mod process_pool;
mod semaphore;
mod task_handle;
mod task_status;
mod zombie_reaper;

pub use backend::{ExecutionBackend, ExecutionConfig, ExecutionMode};
pub use config::{
    CommandConfig, EnvConfig, PoolConfig, PoolConfigBuilder, ResourceLimits, RetryPolicy,
    RetryStrategy, ShutdownConfig, TimeoutConfig,
};
pub use error::{
    CancelError, CommandError, ConfigError, ErrorContext, ExecuteError, ShutdownError, SubmitError, TimeoutError,
};
pub use executor::{
    CommandExecutor, StdCommandExecutor, apply_env_config, execute_command_with_context,
    execute_task_with_hooks, execute_with_retry, execute_with_timeouts,
};
pub use health::{HealthCheck, HealthDetails, HealthStatus};
pub use hooks::{ExecutionContext, ExecutionHook, HookTaskResult};
pub use logging::{LogConfig, LogFormat, LogLevel, LogTarget};
pub use metrics::{Metrics, MetricsSnapshot};
pub use pipeline::{Pipeline, PipelineExecutor, PipelineStage};
pub use pool::CommandPool;
pub use pool_seg::CommandPoolSeg;
pub use process_pool::ProcessPool;
pub use semaphore::{Semaphore, SemaphoreGuard};
pub use task_handle::{CancellationToken, TaskHandle, TaskResult, TaskState, TaskWithResult};
pub use task_status::{TaskIdGenerator, TaskStatus, TaskStatusTracker};
pub use zombie_reaper::ZombieReaper;
