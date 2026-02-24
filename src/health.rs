use std::time::{Duration, SystemTime};

/// 健康状态
///
/// 表示系统的健康状况，包括健康、降级和不健康三种状态。
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    /// 系统健康，所有检查都通过
    Healthy,

    /// 系统降级，存在一些问题但仍可运行
    Degraded {
        /// 检测到的问题列表
        issues: Vec<String>,
    },

    /// 系统不健康，无法正常运行
    Unhealthy {
        /// 检测到的严重问题列表
        issues: Vec<String>,
    },
}

/// 健康检查结果
///
/// 包含健康状态、时间戳和详细信息。
/// 用于监控系统健康状态，可集成到监控系统中。
///
/// # 示例
///
/// ```ignore
/// use execute::CommandPool;
///
/// let pool = CommandPool::new(4);
/// let health = pool.health_check();
///
/// match health.status {
///     HealthStatus::Healthy => println!("System is healthy"),
///     HealthStatus::Degraded { ref issues } => {
///         println!("System is degraded:");
///         for issue in issues {
///             println!("  - {}", issue);
///         }
///     }
///     HealthStatus::Unhealthy { ref issues } => {
///         println!("System is unhealthy:");
///         for issue in issues {
///             println!("  - {}", issue);
///         }
///     }
/// }
///
/// println!("Workers: {}/{}", health.details.workers_alive, health.details.workers_total);
/// println!("Queue usage: {:.1}%", health.details.queue_usage * 100.0);
/// ```
#[derive(Debug, Clone)]
pub struct HealthCheck {
    /// 健康状态
    pub status: HealthStatus,

    /// 检查时间戳
    pub timestamp: SystemTime,

    /// 详细信息
    pub details: HealthDetails,
}

/// 健康检查详细信息
///
/// 包含系统运行状态的详细指标。
/// 这些指标用于判断系统健康状态。
///
/// # 字段
///
/// * `workers_alive` - 存活的工作线程数
/// * `workers_total` - 总工作线程数
/// * `queue_usage` - 队列使用率（0.0 - 1.0），1.0 表示队列已满
/// * `long_running_tasks` - 长时间运行的任务数（超过阈值）
/// * `avg_task_duration` - 平均任务执行时长
///
/// # 示例
///
/// ```ignore
/// use execute::CommandPool;
///
/// let pool = CommandPool::new(4);
/// let health = pool.health_check();
/// let details = &health.details;
///
/// println!("Workers: {}/{}", details.workers_alive, details.workers_total);
/// println!("Queue usage: {:.1}%", details.queue_usage * 100.0);
/// println!("Long running tasks: {}", details.long_running_tasks);
/// println!("Average task duration: {:?}", details.avg_task_duration);
/// ```
#[derive(Debug, Clone)]
pub struct HealthDetails {
    /// 存活的工作线程数
    pub workers_alive: usize,

    /// 总工作线程数
    pub workers_total: usize,

    /// 队列使用率（0.0 - 1.0）
    pub queue_usage: f64,

    /// 长时间运行的任务数
    pub long_running_tasks: usize,

    /// 平均任务执行时长
    pub avg_task_duration: Duration,
}
