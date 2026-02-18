use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::backend::{BackendFactory, ExecutionBackend, ExecutionConfig, ExecutionMode};
use crate::config::CommandConfig;
use crate::error::ExecuteError;
use crate::executor::CommandExecutor;

/// 命令池，支持多线程和多进程两种执行模式
#[derive(Clone)]
pub struct CommandPool {
    tasks: Arc<Mutex<VecDeque<CommandConfig>>>,
    config: ExecutionConfig,
    backend: Arc<dyn ExecutionBackend>,
}

impl CommandPool {
    /// 创建命令池（默认多进程模式）
    pub fn new() -> Self {
        Self::with_config(ExecutionConfig::default())
    }

    /// 使用指定配置创建命令池
    pub fn with_config(config: ExecutionConfig) -> Self {
        let backend = BackendFactory::create(&config);

        Self {
            tasks: Arc::new(Mutex::new(VecDeque::new())),
            config,
            backend,
        }
    }

    /// 添加任务
    pub fn push_task(&self, task: CommandConfig) {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.push_back(task);
    }

    /// 弹出任务
    pub fn pop_task(&self) -> Option<CommandConfig> {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.pop_front()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        let tasks = self.tasks.lock().unwrap();
        tasks.is_empty()
    }

    /// 获取执行模式
    pub fn execution_mode(&self) -> ExecutionMode {
        self.config.mode
    }

    /// 启动执行器
    pub fn start_executor(&self, interval: Duration) {
        match self.config.mode {
            ExecutionMode::Thread => self.start_thread_executor(interval),
            ExecutionMode::Process => self.start_process_executor(interval),
            ExecutionMode::ProcessPool => self.start_process_pool_executor(interval),
        }
    }

    fn start_thread_executor(&self, interval: Duration) {
        for _ in 0..self.config.workers {
            let pool = self.clone();
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool.pop_task() {
                        let _ = pool.execute_task(&task);
                    }
                    thread::sleep(interval);
                }
            });
        }
    }

    fn start_process_executor(&self, interval: Duration) {
        for _ in 0..self.config.workers {
            let pool = self.clone();
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool.pop_task() {
                        let _ = pool.execute_task(&task);
                    }
                    thread::sleep(interval);
                }
            });
        }
    }

    fn start_process_pool_executor(&self, interval: Duration) {
        // 进程池模式：复用工作线程，但后端使用进程池执行命令
        for _ in 0..self.config.workers {
            let pool = self.clone();
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool.pop_task() {
                        let _ = pool.execute_task(&task);
                    }
                    thread::sleep(interval);
                }
            });
        }
    }

    /// 执行单个任务
    pub fn execute_task(
        &self,
        config: &CommandConfig,
    ) -> Result<std::process::Output, ExecuteError> {
        self.backend.execute(config)
    }

    /// 使用自定义执行器启动（高级用法）
    pub fn start_with_executor<E: CommandExecutor + 'static>(
        &self,
        interval: Duration,
        executor: Arc<E>,
    ) {
        for _ in 0..self.config.workers {
            let pool = self.clone();
            let exec = executor.clone();
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool.pop_task() {
                        let _ = exec.execute(&task);
                    }
                    thread::sleep(interval);
                }
            });
        }
    }
}

impl Default for CommandPool {
    fn default() -> Self {
        Self::new()
    }
}
