use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::config::CommandConfig;
use crate::error::ExecuteError;
use crate::execution_mode::{ExecutionConfig, ExecutionMode};
use crate::executor::{CommandExecutor, execute_command};
use crate::semaphore::Semaphore;
use crate::thread_executor::ThreadModeExecutor;

/// 命令池，基于 Mutex<VecDeque> 实现，支持多线程和多进程两种模式。
#[derive(Clone)]
pub struct CommandPool {
    tasks: Arc<Mutex<VecDeque<CommandConfig>>>,
    /// 执行模式配置
    exec_config: ExecutionConfig,
    /// 线程模式执行器（仅在 Thread 模式下使用）
    thread_executor: Option<Arc<ThreadModeExecutor>>,
}

/// `CommandPool` 是一个简单的命令队列，支持多线程生产任务并由后台执行器消费执行。
///
/// 默认使用 `StdCommandExecutor`。如需使用其他执行器，可创建并传入自定义实现。
///
/// 使用示例：
/// ```ignore
/// use execute::{CommandPool, CommandConfig};
/// use std::time::Duration;
///
/// let pool = CommandPool::new();
/// pool.push_task(CommandConfig::new("echo", vec!["hi".to_string()]));
/// pool.start_executor(Duration::from_secs(1));
/// ```
impl CommandPool {
    /// # 创建一个CommandPool命令池（默认多进程模式）
    pub fn new() -> Self {
        Self::with_config(ExecutionConfig::default())
    }

    /// # 使用指定执行模式创建命令池
    ///
    /// # 参数
    /// - `config`: 执行模式配置，包含模式类型、工作线程/进程数等
    ///
    /// # 示例
    /// ```ignore
    /// use execute::{CommandPool, ExecutionConfig, ExecutionMode};
    ///
    /// // 创建多线程模式的命令池
    /// let config = ExecutionConfig::new().with_mode(ExecutionMode::Thread);
    /// let pool = CommandPool::with_config(config);
    /// ```
    pub fn with_config(config: ExecutionConfig) -> Self {
        let thread_executor = if config.mode == ExecutionMode::Thread {
            Some(Arc::new(ThreadModeExecutor::new(config.workers)))
        } else {
            None
        };

        Self {
            tasks: Arc::new(Mutex::new(VecDeque::new())),
            exec_config: config,
            thread_executor,
        }
    }

    /// # 获取当前执行模式
    pub fn execution_mode(&self) -> ExecutionMode {
        self.exec_config.mode
    }

    /// # 获取执行配置
    pub fn execution_config(&self) -> &ExecutionConfig {
        &self.exec_config
    }

    /// # 添加任务到命令池
    ///
    /// 将给定的 `CommandConfig` 推入命令池的队尾，等待执行器轮询时被取出执行。
    ///
    /// # 参数
    /// - `task`: 要添加到池中的 `CommandConfig` 实例。
    ///
    /// # 示例
    /// ```ignore
    /// use execute::{CommandPool, CommandConfig};
    ///
    /// let pool = CommandPool::new();
    /// pool.push_task(CommandConfig::new("echo", vec!["hi".to_string()]));
    /// ```
    pub fn push_task(&self, task: CommandConfig) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.push_back(task);
    }

    /// # 从命令池弹出任务
    ///
    /// 从队列头部弹出一个任务并返回，若池为空则返回 `None`。
    ///
    /// # 返回
    /// - `Some(CommandConfig)`: 成功弹出任务。
    /// - `None`: 池为空。
    pub fn pop_task(&self) -> Option<CommandConfig> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.pop_front()
    }

    /// # 池是否为空
    ///
    /// 返回当前命令池是否没有待处理任务。
    pub fn is_empty(&self) -> bool {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.is_empty()
    }

    /// # 启动定时执行器
    ///
    /// 在后台线程中启动轮询执行器，按指定 `interval` 轮询命令池并执行任务。
    /// 根据配置的执行模式（多线程或多进程）选择相应的执行策略。
    ///
    /// # 参数
    /// - `interval`: 两次轮询之间的间隔时间。
    ///
    /// # 示例
    /// ```ignore
    /// use execute::CommandPool;
    /// use std::time::Duration;
    ///
    /// let pool = CommandPool::new();
    /// pool.start_executor(Duration::from_secs(1));
    /// ```
    pub fn start_executor(&self, interval: Duration) {
        match self.exec_config.mode {
            ExecutionMode::Thread => {
                self.start_thread_executor(interval);
            }
            ExecutionMode::Process => {
                self.start_process_executor(interval, self.exec_config.workers);
            }
        }
    }

    /// 启动线程模式执行器
    fn start_thread_executor(&self, interval: Duration) {
        if let Some(ref executor) = self.thread_executor {
            // 启动线程执行器
            executor.start(interval);

            // 启动任务转发线程，将队列中的任务提交给线程执行器
            let pool_clone = self.clone();
            let executor_clone = executor.clone();
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool_clone.pop_task() {
                        executor_clone.submit(task);
                    }
                    thread::sleep(interval);
                }
            });
        }
    }

    /// 启动进程模式执行器
    fn start_process_executor(&self, interval: Duration, workers: usize) {
        if let Some(limit) = self.exec_config.concurrency_limit {
            self.start_executor_with_workers_and_limit(interval, workers, limit);
        } else {
            self.start_executor_with_workers(interval, workers);
        }
    }

    /// 启动具有固定工作线程数的执行器以复用线程并发执行任务。
    pub fn start_executor_with_workers(&self, interval: Duration, workers: usize) {
        for _ in 0..workers {
            let pool_clone = self.clone();
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool_clone.pop_task() {
                        let _ = pool_clone.execute_task(&task);
                    }
                    thread::sleep(interval);
                }
            });
        }
    }

    /// 与 `start_executor_with_workers` 类似，但限制同时执行的外部进程数量为 `limit`。
    pub fn start_executor_with_workers_and_limit(
        &self,
        interval: Duration,
        workers: usize,
        limit: usize,
    ) {
        let sem = Arc::new(Semaphore::new(limit));
        for _ in 0..workers {
            let pool_clone = self.clone();
            let sem = sem.clone();
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool_clone.pop_task() {
                        let _permit = sem.acquire_guard();
                        let _ = pool_clone.execute_task(&task);
                    }
                    thread::sleep(interval);
                }
            });
        }
    }

    /// Execute a single task.
    ///
    /// 启动子进程并等待完成；若设置了超时，会在超时后尝试终止子进程并返回 `ExecuteError::Timeout`。
    ///
    /// # 参数
    /// - `config`: 要执行的命令配置引用。
    ///
    /// # 返回
    /// - `Ok(Output)`: 子进程正常退出并返回输出。
    /// - `Err(ExecuteError)`: 启动进程、等待或超时等错误情况。
    pub fn execute_task(
        &self,
        config: &CommandConfig,
    ) -> Result<std::process::Output, ExecuteError> {
        execute_command(config)
    }

    /// 使用自定义执行器启动执行器 | Start executor with custom executor
    ///
    /// 允许用户指定自己的 CommandExecutor 实现，从而支持不同的运行时。
    /// Allows users to provide a custom CommandExecutor implementation for different runtimes.
    pub fn start_executor_with_executor<E: CommandExecutor + 'static>(
        &self,
        interval: Duration,
        executor: Arc<E>,
    ) {
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        self.start_executor_with_workers_and_executor(interval, workers, executor);
    }

    /// 使用自定义执行器和工作线程数启动执行器 | Start executor with custom executor and worker count
    pub fn start_executor_with_workers_and_executor<E: CommandExecutor + 'static>(
        &self,
        interval: Duration,
        workers: usize,
        executor: Arc<E>,
    ) {
        for _ in 0..workers {
            let pool_clone = self.clone();
            let executor = executor.clone();
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool_clone.pop_task() {
                        let _ = executor.execute(&task);
                    }
                    thread::sleep(interval);
                }
            });
        }
    }

    /// 使用自定义执行器和并发限制启动执行器 | Start executor with custom executor and concurrency limit
    pub fn start_executor_with_executor_and_limit<E: CommandExecutor + 'static>(
        &self,
        interval: Duration,
        workers: usize,
        limit: usize,
        executor: Arc<E>,
    ) {
        let sem = Arc::new(Semaphore::new(limit));
        for _ in 0..workers {
            let pool_clone = self.clone();
            let executor = executor.clone();
            let sem = sem.clone();
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool_clone.pop_task() {
                        let _permit = sem.acquire_guard();
                        let _ = executor.execute(&task);
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
