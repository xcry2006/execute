use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use crossbeam_queue::SegQueue;

use crate::config::CommandConfig;
use crate::executor::{CommandExecutor, execute_command};
use crate::semaphore::Semaphore;

/// 基于无锁队列（SegQueue）的命令池 | Lock-free command pool using SegQueue
///
/// 相比 CommandPool 的 Mutex-based 实现，SegQueue 提供更高的并发吞吐量。
/// 特别是在多生产者场景下性能更优（避免了锁竞争）。
#[derive(Clone)]
pub struct CommandPoolSeg {
    tasks: Arc<SegQueue<CommandConfig>>,
    stop_flag: Arc<AtomicBool>,
    task_id_counter: Arc<AtomicU64>,
}

impl CommandPoolSeg {
    /// 创建一个新的无锁命令池 | Create a new lock-free command pool
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(SegQueue::new()),
            stop_flag: Arc::new(AtomicBool::new(false)),
            task_id_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    /// 无阻塞地推入任务 | Push a task without blocking (lock-free)
    ///
    /// # 返回值
    ///
    /// 如果命令池已停止，返回 `Err(SubmitError::Stopped)`；否则返回 `Ok(())`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use execute::{CommandPoolSeg, CommandConfig};
    ///
    /// let pool = CommandPoolSeg::new();
    /// let config = CommandConfig::new("echo", vec!["hello".to_string()]);
    ///
    /// pool.push_task(config).unwrap();
    ///
    /// pool.stop();
    ///
    /// let config2 = CommandConfig::new("echo", vec!["world".to_string()]);
    /// assert!(pool.push_task(config2).is_err());
    /// ```
    pub fn push_task(&self, task: CommandConfig) -> Result<(), crate::error::SubmitError> {
        if self.is_stopped() {
            return Err(crate::error::SubmitError::Stopped);
        }
        self.tasks.push(task);
        Ok(())
    }

    /// 无阻塞地尝试弹出任务 | Try to pop a task without blocking
    pub fn pop_task(&self) -> Option<CommandConfig> {
        self.tasks.pop()
    }

    /// 返回队列是否为空
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// 停止命令池，不再接受新任务 | Stop the command pool from accepting new tasks
    ///
    /// 调用此方法后，命令池将停止接受新任务提交，但会继续执行队列中已有的任务。
    /// 工作线程会在处理完队列中的所有任务后自动退出。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use execute::CommandPoolSeg;
    /// use std::time::Duration;
    ///
    /// let pool = CommandPoolSeg::new();
    /// pool.start_executor(Duration::from_millis(100));
    ///
    /// // ... 提交一些任务 ...
    ///
    /// // 停止接受新任务
    /// pool.stop();
    /// ```
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// 查询命令池是否已停止 | Check if the command pool is stopped
    ///
    /// # 返回值
    ///
    /// 如果命令池已调用 `stop()` 方法，返回 `true`；否则返回 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::CommandPoolSeg;
    ///
    /// let pool = CommandPoolSeg::new();
    /// assert!(!pool.is_stopped());
    ///
    /// pool.stop();
    /// assert!(pool.is_stopped());
    /// ```
    pub fn is_stopped(&self) -> bool {
        self.stop_flag.load(Ordering::SeqCst)
    }

    /// 执行单个任务，支持重试
    ///
    /// 如果任务配置了重试策略，会自动重试失败的任务。
    /// 重试过程中会记录日志，但不影响指标的准确性。
    ///
    /// # 参数
    ///
    /// * `task` - 要执行的任务配置
    ///
    /// # 返回
    ///
    /// 成功时返回命令输出，失败时返回错误
    fn execute_task_with_retry(
        &self,
        task: &CommandConfig,
    ) -> Result<std::process::Output, crate::error::ExecuteError> {
        let task_id = self.task_id_counter.fetch_add(1, Ordering::SeqCst);

        // 如果配置了重试策略，使用 execute_with_retry
        if task.retry_policy().is_some() {
            use crate::executor::execute_with_retry;
            execute_with_retry(task, task_id)
                .map_err(|e| crate::error::ExecuteError::Io(std::io::Error::other(e.to_string())))
        } else {
            // 否则直接执行
            execute_command(task)
        }
    }

    /// 启动自动调节工作线程数的执行器 | Start executor with auto-detected worker count
    ///
    /// 根据 CPU 核心数自动调节工作线程数，默认最少 4 个线程。
    pub fn start_executor(&self, interval: Duration) {
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        self.start_executor_with_workers(interval, workers);
    }

    /// 启动具有固定工作线程数的执行器 | Start executor with fixed worker thread count
    ///
    /// 使用固定数量的线程复用，避免频繁创建销毁线程的开销。
    pub fn start_executor_with_workers(&self, interval: Duration, workers: usize) {
        for _ in 0..workers {
            let pool = self.clone();
            thread::spawn(move || {
                loop {
                    // 检查停止标志
                    if pool.is_stopped() && pool.is_empty() {
                        break;
                    }

                    while let Some(task) = pool.pop_task() {
                        let _ = pool.execute_task_with_retry(&task);

                        // 在处理任务后再次检查停止标志
                        if pool.is_stopped() && pool.is_empty() {
                            break;
                        }
                    }

                    // 如果已停止且队列为空，退出
                    if pool.is_stopped() && pool.is_empty() {
                        break;
                    }

                    thread::sleep(interval);
                }
            });
        }
    }

    /// 限制同时执行的外部进程数量为 `limit` 的工作线程启动函数。
    pub fn start_executor_with_workers_and_limit(
        &self,
        interval: Duration,
        workers: usize,
        limit: usize,
    ) {
        let sem = Arc::new(Semaphore::new(limit));
        for _ in 0..workers {
            let pool = self.clone();
            let sem = sem.clone();
            thread::spawn(move || {
                loop {
                    // 检查停止标志
                    if pool.is_stopped() && pool.is_empty() {
                        break;
                    }

                    while let Some(task) = pool.pop_task() {
                        let _permit = sem.acquire_guard();
                        let _ = pool.execute_task_with_retry(&task);

                        // 在处理任务后再次检查停止标志
                        if pool.is_stopped() && pool.is_empty() {
                            break;
                        }
                    }

                    // 如果已停止且队列为空，退出
                    if pool.is_stopped() && pool.is_empty() {
                        break;
                    }

                    thread::sleep(interval);
                }
            });
        }
    }

    /// 使用自定义执行器启动执行器 | Start executor with custom executor
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
            let pool = self.clone();
            let executor = executor.clone();
            thread::spawn(move || {
                loop {
                    // 检查停止标志
                    if pool.is_stopped() && pool.is_empty() {
                        break;
                    }

                    while let Some(task) = pool.pop_task() {
                        let _ = executor.execute(&task);

                        // 在处理任务后再次检查停止标志
                        if pool.is_stopped() && pool.is_empty() {
                            break;
                        }
                    }

                    // 如果已停止且队列为空，退出
                    if pool.is_stopped() && pool.is_empty() {
                        break;
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
            let pool = self.clone();
            let executor = executor.clone();
            let sem = sem.clone();
            thread::spawn(move || {
                loop {
                    // 检查停止标志
                    if pool.is_stopped() && pool.is_empty() {
                        break;
                    }

                    while let Some(task) = pool.pop_task() {
                        let _permit = sem.acquire_guard();
                        let _ = executor.execute(&task);

                        // 在处理任务后再次检查停止标志
                        if pool.is_stopped() && pool.is_empty() {
                            break;
                        }
                    }

                    // 如果已停止且队列为空，退出
                    if pool.is_stopped() && pool.is_empty() {
                        break;
                    }

                    thread::sleep(interval);
                }
            });
        }
    }
}

impl Default for CommandPoolSeg {
    fn default() -> Self {
        Self::new()
    }
}
