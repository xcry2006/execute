use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::backend::{BackendFactory, ExecutionBackend, ExecutionConfig, ExecutionMode};
use crate::config::CommandConfig;
use crate::error::ExecuteError;
use crate::executor::CommandExecutor;

/// 命令池，支持多线程和多进程两种执行模式
///
/// 提供线程安全的任务队列管理，支持多种执行后端（Process/Thread/ProcessPool）。
/// 可选的队列大小限制可防止内存无限增长。
pub struct CommandPool {
    /// 任务队列和条件变量
    ///
    /// - Mutex<VecDeque<CommandConfig>>: 存储待执行的任务
    /// - Condvar: 用于队列满时的阻塞等待和通知
    tasks: Arc<(Mutex<VecDeque<CommandConfig>>, Condvar)>,

    /// 执行配置
    ///
    /// 包含执行模式、工作线程数、并发限制等配置
    config: ExecutionConfig,

    /// 执行后端
    ///
    /// 具体的命令执行实现（ProcessBackend/ThreadBackend/ProcessPoolBackend）
    backend: Arc<dyn ExecutionBackend>,

    /// 运行状态标志
    ///
    /// 用于控制执行器线程的启动和停止
    running: Arc<AtomicBool>,

    /// 工作线程句柄列表
    ///
    /// 存储所有执行器线程的 JoinHandle，用于优雅关闭
    handles: Arc<Mutex<Vec<JoinHandle<()>>>>,

    /// 队列大小限制
    ///
    /// None 表示无限制，Some(n) 表示最多 n 个任务
    max_size: Option<usize>,
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
            tasks: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            config,
            backend,
            running: Arc::new(AtomicBool::new(false)),
            handles: Arc::new(Mutex::new(Vec::new())),
            max_size: None,
        }
    }

    /// 使用指定配置和队列大小限制创建命令池
    pub fn with_config_and_limit(config: ExecutionConfig, max_size: usize) -> Self {
        let backend = BackendFactory::create(&config);

        Self {
            tasks: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            config,
            backend,
            running: Arc::new(AtomicBool::new(false)),
            handles: Arc::new(Mutex::new(Vec::new())),
            max_size: Some(max_size),
        }
    }

    /// 添加任务（如果设置了队列大小限制，队列满时会阻塞等待）
    pub fn push_task(&self, task: CommandConfig) {
        let (lock, cvar) = &*self.tasks;
        let mut tasks = lock.lock().unwrap();

        // 如果设置了队列大小限制，等待队列有空位
        if let Some(max) = self.max_size {
            while tasks.len() >= max {
                tasks = cvar.wait(tasks).unwrap();
            }
        }

        tasks.push_back(task);
        cvar.notify_one();
    }

    /// 尝试添加任务，如果队列满则返回错误
    pub fn try_push_task(&self, task: CommandConfig) -> Result<(), ExecuteError> {
        let (lock, cvar) = &*self.tasks;
        let mut tasks = lock.lock().unwrap();

        // 如果设置了队列大小限制，检查是否有空位
        if let Some(max) = self.max_size
            && tasks.len() >= max
        {
            return Err(ExecuteError::Io(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "task queue is full",
            )));
        }

        tasks.push_back(task);
        cvar.notify_one();
        Ok(())
    }

    /// 弹出任务
    pub fn pop_task(&self) -> Option<CommandConfig> {
        let (lock, cvar) = &*self.tasks;
        let mut tasks = lock.lock().unwrap();
        let task = tasks.pop_front();
        if task.is_some() {
            cvar.notify_one();
        }
        task
    }

    /// 批量添加任务
    pub fn push_tasks_batch(&self, tasks: Vec<CommandConfig>) -> usize {
        let (lock, cvar) = &*self.tasks;
        let mut queue = lock.lock().unwrap();

        let count = tasks.len();

        for task in tasks {
            // 如果设置了队列大小限制，等待队列有空位
            if let Some(max) = self.max_size {
                while queue.len() >= max {
                    queue = cvar.wait(queue).unwrap();
                }
            }
            queue.push_back(task);
        }

        cvar.notify_all();
        count
    }

    /// 尝试批量添加任务，返回成功添加的数量
    pub fn try_push_tasks_batch(&self, tasks: Vec<CommandConfig>) -> usize {
        let (lock, cvar) = &*self.tasks;
        let mut queue = lock.lock().unwrap();

        let mut count = 0;

        for task in tasks {
            // 如果设置了队列大小限制，检查是否有空位
            if let Some(max) = self.max_size
                && queue.len() >= max
            {
                break;
            }
            queue.push_back(task);
            count += 1;
        }

        if count > 0 {
            cvar.notify_all();
        }
        count
    }

    /// 清空所有任务
    pub fn clear(&self) -> usize {
        let (lock, cvar) = &*self.tasks;
        let mut tasks = lock.lock().unwrap();
        let count = tasks.len();
        tasks.clear();
        cvar.notify_all();
        count
    }

    /// 获取当前队列大小
    pub fn len(&self) -> usize {
        let (lock, _) = &*self.tasks;
        let tasks = lock.lock().unwrap();
        tasks.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 获取队列大小限制
    pub fn max_size(&self) -> Option<usize> {
        self.max_size
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
            max_size: self.max_size,
        }
    }
}

impl Default for CommandPool {
    fn default() -> Self {
        Self::new()
    }
}
