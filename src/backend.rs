use std::process::Output;
use std::sync::Arc;

use crate::config::CommandConfig;
use crate::error::ExecuteError;
use crate::semaphore::Semaphore;

/// 执行后端 trait
pub trait ExecutionBackend: Send + Sync {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError>;
}

/// 执行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionMode {
    #[default]
    Process,
    Thread,
    ProcessPool,
}

/// 执行配置
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub mode: ExecutionMode,
    pub workers: usize,
    pub concurrency_limit: Option<usize>,
    pub zombie_reaper_interval: Option<std::time::Duration>,
}

impl ExecutionConfig {
    pub fn new() -> Self {
        Self {
            mode: ExecutionMode::Process,
            workers: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            concurrency_limit: None,
            zombie_reaper_interval: None,
        }
    }

    pub fn with_concurrency_limit(mut self, limit: usize) -> Self {
        self.concurrency_limit = Some(limit);
        self
    }

    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_workers(mut self, workers: usize) -> Self {
        self.workers = workers;
        self
    }

    pub fn with_zombie_reaper_interval(mut self, interval: std::time::Duration) -> Self {
        self.zombie_reaper_interval = Some(interval);
        self
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// 通用执行后端
pub struct GenericBackend {
    #[allow(dead_code)]
    mode: ExecutionMode,
    semaphore: Option<Semaphore>,
}

impl GenericBackend {
    pub fn new(mode: ExecutionMode) -> Self {
        Self {
            mode,
            semaphore: None,
        }
    }

    pub fn with_concurrency_limit(mode: ExecutionMode, limit: usize) -> Self {
        Self {
            mode,
            semaphore: Some(Semaphore::new(limit)),
        }
    }
}

impl ExecutionBackend for GenericBackend {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        let _guard = self.semaphore.as_ref().map(|s| s.acquire_guard());
        crate::executor::execute_command(config)
    }
}

/// 后端工厂
pub struct BackendFactory;

impl BackendFactory {
    pub fn create(config: &ExecutionConfig) -> Arc<dyn ExecutionBackend> {
        if let Some(limit) = config.concurrency_limit {
            Arc::new(GenericBackend::with_concurrency_limit(config.mode, limit))
        } else {
            Arc::new(GenericBackend::new(config.mode))
        }
    }
}
