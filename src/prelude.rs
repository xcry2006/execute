pub use crate::backend::{ExecutionConfig, ExecutionMode};
pub use crate::config::CommandConfig;
pub use crate::config::{EnvConfig, ResourceLimits, RetryPolicy, RetryStrategy, TimeoutConfig};
pub use crate::error::{ExecuteError, ShutdownError, SubmitError};
#[cfg(feature = "metrics")]
pub use crate::metrics::{Metrics, MetricsSnapshot};
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
pub use crate::task_handle::{TaskHandle, TaskState};

pub use std::time::Duration;
