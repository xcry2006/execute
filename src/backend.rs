use std::process::Output;
use std::sync::Arc;

use crate::config::CommandConfig;
use crate::error::ExecuteError;

/// 执行后端 trait | Execution backend trait
///
/// 定义命令执行的不同后端实现，用户可以根据需求选择合适的后端。
/// 这是中间层抽象，允许自由选择执行策略。
pub trait ExecutionBackend: Send + Sync {
    /// 执行单个命令
    ///
    /// # 参数
    /// - `config`: 命令配置
    ///
    /// # 返回
    /// - `Ok(Output)`: 命令执行成功
    /// - `Err(ExecuteError)`: 执行失败
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError>;

    /// 获取后端名称
    fn name(&self) -> &'static str;

    /// 启动后端（如果需要）
    fn start(&self) -> Result<(), ExecuteError> {
        Ok(())
    }

    /// 停止后端（如果需要）
    fn stop(&self) -> Result<(), ExecuteError> {
        Ok(())
    }
}

/// 后端类型枚举 | Backend type enumeration
///
/// 预定义的后端类型，方便用户快速选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendType {
    /// 多进程后端 - 每个命令独立子进程
    /// 特点：完全隔离，资源独立，启动开销较大
    #[default]
    Process,

    /// 线程池后端 - 常驻子进程+内部多线程
    /// 特点：通过工作进程池复用，减少启动开销
    ThreadPool,

    /// 进程池后端 - 预创建子进程池
    /// 特点：预创建进程，快速响应，有状态保持能力
    ProcessPool,

    /// 内联后端 - 在同一线程直接执行
    /// 特点：无额外开销，适合轻量命令或测试
    Inline,
}

/// 后端配置 | Backend configuration
#[derive(Debug, Clone)]
pub struct BackendConfig {
    /// 后端类型
    pub backend_type: BackendType,
    /// 工作线程/进程数
    pub workers: usize,
    /// 进程池大小（仅 ProcessPool 使用）
    pub pool_size: Option<usize>,
    /// 并发限制
    pub concurrency_limit: Option<usize>,
}

impl BackendConfig {
    /// 创建默认配置
    pub fn new() -> Self {
        Self {
            backend_type: BackendType::Process,
            workers: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            pool_size: None,
            concurrency_limit: None,
        }
    }

    /// 设置后端类型
    pub fn with_backend_type(mut self, backend_type: BackendType) -> Self {
        self.backend_type = backend_type;
        self
    }

    /// 设置工作线程/进程数
    pub fn with_workers(mut self, workers: usize) -> Self {
        self.workers = workers;
        self
    }

    /// 设置进程池大小
    pub fn with_pool_size(mut self, pool_size: usize) -> Self {
        self.pool_size = Some(pool_size);
        self
    }

    /// 设置并发限制
    pub fn with_concurrency_limit(mut self, limit: usize) -> Self {
        self.concurrency_limit = Some(limit);
        self
    }
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// 后端工厂 | Backend factory
///
/// 根据配置创建对应的后端实例。
pub struct BackendFactory;

impl BackendFactory {
    /// 根据配置创建后端
    pub fn create(config: &BackendConfig) -> Arc<dyn ExecutionBackend> {
        match config.backend_type {
            BackendType::Process => Arc::new(ProcessBackend::new(config)),
            BackendType::ThreadPool => Arc::new(ThreadPoolBackend::new(config)),
            BackendType::ProcessPool => Arc::new(ProcessPoolBackend::new(config)),
            BackendType::Inline => Arc::new(InlineBackend::new()),
        }
    }
}

// ============================================================================
// 具体后端实现
// ============================================================================

/// 多进程后端 | Process backend
///
/// 每个命令启动一个独立的子进程，执行完成后退出。
/// 特点：完全隔离，无状态共享，启动开销较大。
pub struct ProcessBackend {
    #[allow(dead_code)]
    config: BackendConfig,
}

impl ProcessBackend {
    /// 创建新的多进程后端
    pub fn new(config: &BackendConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }
}

impl ExecutionBackend for ProcessBackend {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        crate::executor::execute_command(config)
    }

    fn name(&self) -> &'static str {
        "ProcessBackend"
    }
}

/// 线程池后端 | Thread pool backend
///
/// 在主进程内使用线程池调度任务，每个任务启动子进程执行。
/// 特点：任务调度更高效，但每个命令仍是独立子进程。
pub struct ThreadPoolBackend {
    #[allow(dead_code)]
    config: BackendConfig,
}

impl ThreadPoolBackend {
    /// 创建新的线程池后端
    pub fn new(config: &BackendConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }
}

impl ExecutionBackend for ThreadPoolBackend {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        // 线程池后端也是通过子进程执行命令
        // 区别在于任务调度的机制
        crate::executor::execute_command(config)
    }

    fn name(&self) -> &'static str {
        "ThreadPoolBackend"
    }
}

/// 进程池后端 | Process pool backend
///
/// 预创建一组子进程，复用这些进程执行命令。
/// 特点：减少进程创建开销，可以维护状态。
pub struct ProcessPoolBackend {
    #[allow(dead_code)]
    config: BackendConfig,
}

impl ProcessPoolBackend {
    /// 创建新的进程池后端
    pub fn new(config: &BackendConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }
}

impl ExecutionBackend for ProcessPoolBackend {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        // TODO: 实现进程池逻辑
        // 目前先使用简单实现
        crate::executor::execute_command(config)
    }

    fn name(&self) -> &'static str {
        "ProcessPoolBackend"
    }

    fn start(&self) -> Result<(), ExecuteError> {
        // TODO: 预创建进程池
        Ok(())
    }

    fn stop(&self) -> Result<(), ExecuteError> {
        // TODO: 清理进程池
        Ok(())
    }
}

/// 内联后端 | Inline backend
///
/// 在同一线程直接执行命令，不创建额外线程。
/// 特点：最简单，无并发，适合测试或单任务场景。
pub struct InlineBackend;

impl InlineBackend {
    /// 创建新的内联后端
    pub fn new() -> Self {
        Self
    }
}

impl ExecutionBackend for InlineBackend {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        crate::executor::execute_command(config)
    }

    fn name(&self) -> &'static str {
        "InlineBackend"
    }
}

impl Default for InlineBackend {
    fn default() -> Self {
        Self::new()
    }
}
