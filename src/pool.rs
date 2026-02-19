use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::backend::{BackendFactory, ExecutionBackend, ExecutionConfig, ExecutionMode};
use crate::config::CommandConfig;
use crate::error::ExecuteError;
use crate::executor::CommandExecutor;

/// 命令池，支持多线程和多进程两种执行模式
pub struct CommandPool {
    tasks: Arc<Mutex<VecDeque<CommandConfig>>>,
    config: ExecutionConfig,
    backend: Arc<dyn ExecutionBackend>,
    running: Arc<AtomicBool>,
    handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
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
            running: Arc::new(AtomicBool::new(false)),
            handles: Arc::new(Mutex::new(Vec::new())),
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
        // 如果已经在运行，先停止
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        
        self.running.store(true, Ordering::SeqCst);
        
        match self.config.mode {
            ExecutionMode::Thread => self.start_thread_executor(interval),
            ExecutionMode::Process => self.start_process_executor(interval),
            ExecutionMode::ProcessPool => self.start_process_pool_executor(interval),
        }
    }

    /// 停止执行器
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        
        // 等待所有线程结束
        let mut handles = self.handles.lock().unwrap();
        for handle in handles.drain(..) {
            let _ = handle.join();
        }
    }

    /// 检查执行器是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn start_thread_executor(&self, interval: Duration) {
        for _ in 0..self.config.workers {
            let pool = self.clone();
            let handle = thread::spawn(move || {
                while pool.running.load(Ordering::SeqCst) {
                    while let Some(task) = pool.pop_task() {
                        if !pool.running.load(Ordering::SeqCst) {
                            break;
                        }
                        let _ = pool.execute_task(&task);
                    }
                    thread::sleep(interval);
                }
            });
            self.handles.lock().unwrap().push(handle);
        }
    }

    fn start_process_executor(&self, interval: Duration) {
        for _ in 0..self.config.workers {
            let pool = self.clone();
            let handle = thread::spawn(move || {
                while pool.running.load(Ordering::SeqCst) {
                    while let Some(task) = pool.pop_task() {
                        if !pool.running.load(Ordering::SeqCst) {
                            break;
                        }
                        let _ = pool.execute_task(&task);
                    }
                    thread::sleep(interval);
                }
            });
            self.handles.lock().unwrap().push(handle);
        }
    }

    fn start_process_pool_executor(&self, interval: Duration) {
        // 进程池模式：复用工作线程，但后端使用进程池执行命令
        for _ in 0..self.config.workers {
            let pool = self.clone();
            let handle = thread::spawn(move || {
                while pool.running.load(Ordering::SeqCst) {
                    while let Some(task) = pool.pop_task() {
                        if !pool.running.load(Ordering::SeqCst) {
                            break;
                        }
                        let _ = pool.execute_task(&task);
                    }
                    thread::sleep(interval);
                }
            });
            self.handles.lock().unwrap().push(handle);
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
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        
        self.running.store(true, Ordering::SeqCst);
        
        for _ in 0..self.config.workers {
            let pool = self.clone();
            let exec = executor.clone();
            let handle = thread::spawn(move || {
                while pool.running.load(Ordering::SeqCst) {
                    while let Some(task) = pool.pop_task() {
                        if !pool.running.load(Ordering::SeqCst) {
                            break;
                        }
                        let _ = exec.execute(&task);
                    }
                    thread::sleep(interval);
                }
            });
            self.handles.lock().unwrap().push(handle);
        }
    }
}

impl Clone for CommandPool {
    fn clone(&self) -> Self {
        Self {
            tasks: Arc::clone(&self.tasks),
            config: self.config.clone(),
            backend: Arc::clone(&self.backend),
            running: Arc::clone(&self.running),
            handles: Arc::clone(&self.handles),
        }
    }
}

impl Default for CommandPool {
    fn default() -> Self {
        Self::new()
    }
}
