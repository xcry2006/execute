/// 常用类型预导入模块
///
/// 此模块包含最常用的类型，方便快速开始。
///
/// # 示例
///
/// ```ignore
/// use execute::prelude::*;
///
/// let pool = CommandPool::new();
/// pool.start_executor();
/// ```
pub use crate::pool::CommandPool;
pub use crate::config::CommandConfig;
pub use crate::config::{RetryPolicy, RetryStrategy, TimeoutConfig, EnvConfig, ResourceLimits};
pub use crate::error::{ExecuteError, SubmitError, ShutdownError};
pub use crate::task_handle::{TaskHandle, TaskState};
#[cfg(feature = "metrics")]
pub use crate::metrics::{Metrics, MetricsSnapshot};
pub use crate::backend::{ExecutionConfig, ExecutionMode};

pub use std::time::Duration;
