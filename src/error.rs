use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use thiserror::Error;

/// ExecuteError 表示在启动或等待子进程过程中可能遇到的错误。
///
/// 常见变体包括 IO 错误、超时错误以及子进程状态异常等。
#[derive(Error, Debug)]
pub enum ExecuteError {
    /// IO 错误
    ///
    /// 当系统调用失败时返回，如进程创建失败、管道创建失败等。
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// 命令执行超时
    ///
    /// 当命令执行时间超过设定的超时时间时返回。
    /// 包含实际的超时时长。
    #[error("command execution timed out after {0:?}")]
    Timeout(Duration),

    /// 子进程错误
    ///
    /// 当子进程返回非零退出码或其他异常状态时返回。
    /// 包含错误描述信息。
    #[error("child process error: {0}")]
    Child(String),

    /// 任务已取消
    ///
    /// 当任务被用户取消时返回。
    /// 包含任务 ID。
    #[error("task {0} was cancelled")]
    Cancelled(u64),
}

/// 错误上下文，包含命令执行失败时的详细信息
///
/// 此结构体提供了丰富的上下文信息，帮助快速定位和解决问题。
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// 任务 ID
    pub task_id: u64,
    /// 完整的命令字符串
    pub command: String,
    /// 工作目录
    pub working_dir: PathBuf,
    /// 失败时间戳
    pub timestamp: SystemTime,
    /// 工作线程 ID（如果适用）
    pub worker_id: Option<usize>,
}

impl ErrorContext {
    /// 创建新的错误上下文
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务 ID
    /// * `command` - 命令字符串
    /// * `working_dir` - 工作目录路径
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    /// use execute::ErrorContext;
    ///
    /// let ctx = ErrorContext::new(1, "ls -la", Path::new("/tmp"));
    /// assert_eq!(ctx.task_id, 1);
    /// assert_eq!(ctx.command, "ls -la");
    /// ```
    pub fn new(task_id: u64, command: &str, working_dir: &std::path::Path) -> Self {
        Self {
            task_id,
            command: command.to_string(),
            working_dir: working_dir.to_path_buf(),
            timestamp: SystemTime::now(),
            worker_id: None,
        }
    }

    /// 设置工作线程 ID
    pub fn with_worker_id(mut self, worker_id: usize) -> Self {
        self.worker_id = Some(worker_id);
        self
    }
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "task_id={}, command='{}', working_dir='{}', timestamp={:?}",
            self.task_id,
            self.command,
            self.working_dir.display(),
            self.timestamp
        )?;
        if let Some(worker_id) = self.worker_id {
            write!(f, ", worker_id={}", worker_id)?;
        }
        Ok(())
    }
}

/// 命令执行错误，包含详细的上下文信息
#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Command execution failed: {context}, source: {source}")]
    ExecutionFailed {
        context: ErrorContext,
        #[source]
        source: std::io::Error,
    },

    #[error("Command timeout: {context}, configured_timeout={configured_timeout:?}, actual_duration={actual_duration:?}")]
    Timeout {
        context: ErrorContext,
        configured_timeout: Duration,
        actual_duration: Duration,
    },

    #[error("Spawn failed: {context}, source: {source}")]
    SpawnFailed {
        context: ErrorContext,
        #[source]
        source: std::io::Error,
    },
}

impl CommandError {
    pub fn from_execute_error(error: ExecuteError, context: ErrorContext) -> Self {
        match error {
            ExecuteError::Io(e) => CommandError::ExecutionFailed { context, source: e },
            ExecuteError::Timeout(timeout) => CommandError::Timeout {
                context,
                configured_timeout: timeout,
                actual_duration: timeout,
            },
            ExecuteError::Child(msg) => CommandError::ExecutionFailed {
                context,
                source: std::io::Error::other(msg),
            },
            ExecuteError::Cancelled(task_id) => CommandError::ExecutionFailed {
                context,
                source: std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    format!("Task {} was cancelled", task_id),
                ),
            },
        }
    }
}

/// 配置错误类型
///
/// 此枚举表示在创建或验证命令池配置时可能遇到的各种错误。
/// 每个错误变体都提供清晰的错误消息，帮助用户快速定位配置问题。
#[derive(Error, Debug)]
pub enum ConfigError {
    /// 无效的线程数
    ///
    /// 当线程数小于 1 时返回此错误。
    /// 线程数必须至少为 1 才能执行任务。
    #[error("Invalid thread count: {0}, must be >= 1")]
    InvalidThreadCount(usize),

    /// 无效的队列容量
    ///
    /// 当队列容量小于 1 时返回此错误。
    /// 队列容量必须至少为 1 才能存储任务。
    #[error("Invalid queue capacity: {0}, must be >= 1")]
    InvalidQueueCapacity(usize),

    /// 无效的超时时间
    ///
    /// 当超时时间为零或负数时返回此错误。
    /// 超时时间必须为正数。
    #[error("Invalid timeout: {0:?}, must be positive")]
    InvalidTimeout(Duration),

    /// 无效的轮询间隔
    ///
    /// 当轮询间隔为零或负数时返回此错误。
    /// 轮询间隔必须为正数。
    #[error("Invalid poll interval: {0:?}, must be positive")]
    InvalidPollInterval(Duration),

    /// 线程数超过系统限制
    ///
    /// 当请求的线程数超过系统允许的最大线程数时返回此错误。
    /// 第一个参数是请求的线程数，第二个参数是系统限制。
    #[error("Thread count {0} exceeds system limit {1}")]
    ThreadCountExceedsLimit(usize, usize),
}

/// 关闭错误类型
///
/// 此枚举表示在关闭命令池时可能遇到的错误。
#[derive(Error, Debug)]
pub enum ShutdownError {
    /// 关闭超时
    ///
    /// 当等待任务完成超过配置的超时时间时返回此错误。
    #[error("Shutdown timeout after {0:?}")]
    Timeout(Duration),

    /// 工作线程 panic
    ///
    /// 当工作线程在关闭过程中 panic 时返回此错误。
    #[error("Worker thread panicked")]
    WorkerPanic,
}

/// 提交错误类型
///
/// 此枚举表示在提交任务到命令池时可能遇到的错误。
#[derive(Error, Debug)]
pub enum SubmitError {
    /// 队列已满
    ///
    /// 当队列达到最大容量且无法接受新任务时返回此错误。
    #[error("Queue is full")]
    QueueFull,

    /// 命令池正在关闭
    ///
    /// 当命令池已开始关闭流程时尝试提交任务会返回此错误。
    #[error("Pool is shutting down")]
    ShuttingDown,

    /// 命令池已停止
    ///
    /// 当命令池已完全停止时尝试提交任务会返回此错误。
    #[error("Pool is stopped")]
    Stopped,
}

/// 取消错误类型
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CancelError {
    #[error("Task already completed")]
    AlreadyCompleted,

    #[error("Task already cancelled")]
    AlreadyCancelled,

    #[error("Invalid task state for cancellation")]
    InvalidState,

    #[error("Failed to kill process: {0}")]
    KillFailed(String),
}
