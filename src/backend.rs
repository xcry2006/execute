use std::process::Output;
use std::sync::Arc;

use crate::config::CommandConfig;
use crate::error::ExecuteError;

/// 执行后端 trait
///
/// 定义命令执行的抽象接口，支持多种执行策略。
/// 实现此 trait 的后端可以被 CommandPool 使用来执行命令。
pub trait ExecutionBackend: Send + Sync {
    /// 执行单个命令
    ///
    /// # 参数
    /// - `config`: 命令配置，包含程序名、参数、工作目录和超时设置
    ///
    /// # 返回
    /// - `Ok(Output)`: 命令成功执行，返回输出
    /// - `Err(ExecuteError)`: 执行失败，返回错误
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError>;
}

/// 执行模式
///
/// 定义命令池的执行策略，用户可以根据场景选择合适的模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    /// 多进程模式 - 每个命令独立子进程
    ///
    /// 特点：
    /// - 每个命令启动一个新的子进程
    /// - 完全隔离，安全性高
    /// - 进程创建开销较大
    #[default]
    Process,
    /// 多线程模式 - 线程池调度任务
    ///
    /// 特点：
    /// - 在主进程内使用线程池调度
    /// - 任务切换开销小
    /// - 共享内存空间
    Thread,
    /// 进程池模式 - 常驻子进程池
    ///
    /// 特点：
    /// - 预创建一组子进程，复用执行命令
    /// - 减少进程创建开销
    /// - 可以维护状态（未来实现）
    /// - 适合高频短命令场景
    ProcessPool,
}

/// 执行配置
///
/// 用于配置命令池的执行行为，包括执行模式和工作线程数。
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// 执行模式
    pub mode: ExecutionMode,
    /// 工作线程/进程数
    pub workers: usize,
}

impl ExecutionConfig {
    /// 创建默认配置
    ///
    /// 默认使用多进程模式，工作线程数为 CPU 核心数（至少为 4）。
    pub fn new() -> Self {
        Self {
            mode: ExecutionMode::Process,
            workers: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        }
    }

    /// 设置执行模式
    ///
    /// # 参数
    /// - `mode`: 执行模式（Process/Thread/ProcessPool）
    ///
    /// # 示例
    /// ```ignore
    /// use execute::{ExecutionConfig, ExecutionMode};
    ///
    /// let config = ExecutionConfig::new().with_mode(ExecutionMode::Thread);
    /// ```
    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置工作线程/进程数
    ///
    /// # 参数
    /// - `workers`: 工作线程或进程的数量
    ///
    /// # 示例
    /// ```ignore
    /// use execute::ExecutionConfig;
    ///
    /// let config = ExecutionConfig::new().with_workers(8);
    /// ```
    pub fn with_workers(mut self, workers: usize) -> Self {
        self.workers = workers;
        self
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// 多进程后端 - 每个命令独立子进程
///
/// 最简单的执行后端，每次执行命令时都创建一个新的子进程。
/// 适用于命令执行频率不高或需要完全隔离的场景。
pub struct ProcessBackend;

impl ProcessBackend {
    pub fn new() -> Self {
        Self
    }
}

impl ExecutionBackend for ProcessBackend {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        crate::executor::execute_command(config)
    }
}

impl Default for ProcessBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// 多线程后端 - 使用线程池调度任务
///
/// 在主进程内使用多线程调度任务执行。
/// 与多进程模式的区别在于任务调度的机制，命令仍通过子进程执行。
pub struct ThreadBackend {
    #[allow(dead_code)]
    workers: usize,
}

impl ThreadBackend {
    pub fn new(workers: usize) -> Self {
        Self { workers }
    }
}

impl ExecutionBackend for ThreadBackend {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        // 线程模式下也是通过子进程执行命令
        // 区别在于任务调度的机制
        crate::executor::execute_command(config)
    }
}

/// 进程池后端 - 常驻子进程池
///
/// 预创建一组子进程，复用这些进程执行命令。
/// 适用于高频短命令场景，可以显著减少进程创建开销。
///
/// # TODO
/// - 实现真正的进程池逻辑（目前使用简单实现）
/// - 添加 IPC 机制与常驻子进程通信
/// - 支持进程状态保持
pub struct ProcessPoolBackend {
    #[allow(dead_code)]
    pool_size: usize,
}

impl ProcessPoolBackend {
    pub fn new(pool_size: usize) -> Self {
        Self { pool_size }
    }
}

impl ExecutionBackend for ProcessPoolBackend {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        // TODO: 实现进程池逻辑，目前先用简单实现
        // 后续可以通过 IPC 与常驻子进程通信
        crate::executor::execute_command(config)
    }
}

/// 后端工厂
pub struct BackendFactory;

impl BackendFactory {
    pub fn create(config: &ExecutionConfig) -> Arc<dyn ExecutionBackend> {
        match config.mode {
            ExecutionMode::Process => Arc::new(ProcessBackend::new()),
            ExecutionMode::Thread => Arc::new(ThreadBackend::new(config.workers)),
            ExecutionMode::ProcessPool => Arc::new(ProcessPoolBackend::new(config.workers)),
        }
    }
}
