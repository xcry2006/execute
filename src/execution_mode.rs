/// 执行模式枚举 | Execution mode enumeration
///
/// 用户可以选择使用多线程模式或多进程模式来执行命令。
/// Users can choose between multi-threaded or multi-process execution modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// 多线程模式 - 在共享进程内的多个线程中执行任务
    /// Multi-threaded mode - executes tasks in multiple threads within the same process
    Thread,
    /// 多进程模式 - 使用子进程执行每个命令
    /// Multi-process mode - spawns child processes for each command
    Process,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::Process
    }
}

/// 执行模式配置 | Execution mode configuration
///
/// 允许用户配置执行模式的详细参数。
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// 执行模式
    pub mode: ExecutionMode,
    /// 工作线程数（多线程模式）或工作进程数（多进程模式）
    pub workers: usize,
    /// 并发限制（可选）
    pub concurrency_limit: Option<usize>,
}

impl ExecutionConfig {
    /// 创建默认配置（多进程模式）
    pub fn new() -> Self {
        Self {
            mode: ExecutionMode::Process,
            workers: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            concurrency_limit: None,
        }
    }

    /// 设置执行模式
    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置工作线程/进程数
    pub fn with_workers(mut self, workers: usize) -> Self {
        self.workers = workers;
        self
    }

    /// 设置并发限制
    pub fn with_concurrency_limit(mut self, limit: usize) -> Self {
        self.concurrency_limit = Some(limit);
        self
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self::new()
    }
}
