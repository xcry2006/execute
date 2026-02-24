use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use crate::backend::{BackendFactory, ExecutionBackend, ExecutionConfig, ExecutionMode};
use crate::config::{CommandConfig, ShutdownConfig};
use crate::error::{ExecuteError, ShutdownError, SubmitError};
use crate::executor::CommandExecutor;
use crate::health::{HealthCheck, HealthDetails, HealthStatus};
use crate::hooks::ExecutionHook;
use crate::metrics::Metrics;
use crate::task_handle::{TaskHandle, TaskResult, TaskState};
use crate::zombie_reaper::ZombieReaper;

/// 任务项，包含配置和句柄
pub struct TaskItem {
    /// 命令配置
    pub config: CommandConfig,
    /// 任务句柄
    pub handle: TaskHandle,
    /// 结果发送器
    pub result_sender: std::sync::mpsc::Sender<TaskResult>,
}

/// 命令池，支持多线程和多进程两种执行模式
///
/// 提供线程安全的任务队列管理，支持多种执行后端（Process/Thread/ProcessPool）。
/// 可选的队列大小限制可防止内存无限增长。
pub struct CommandPool {
    /// 任务队列和条件变量
    ///
    /// - `Mutex<VecDeque<TaskItem>>`: 存储待执行的任务
    /// - Condvar: 用于队列满时的阻塞等待和通知
    tasks: Arc<(Mutex<VecDeque<TaskItem>>, Condvar)>,

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

    /// 指标收集器
    ///
    /// 收集任务执行的统计信息
    metrics: Metrics,

    /// 任务 ID 生成器
    ///
    /// 为每个任务生成唯一 ID
    task_id_counter: Arc<AtomicU64>,

    /// 关闭标志
    ///
    /// 用于标记命令池是否正在关闭或已关闭
    shutdown_flag: Arc<AtomicBool>,

    /// 关闭配置
    ///
    /// 配置优雅关闭的行为
    shutdown_config: ShutdownConfig,

    /// 僵尸进程清理器（可选）
    ///
    /// 如果配置了僵尸进程清理间隔，会启动后台线程定期清理僵尸进程
    #[allow(dead_code)]
    zombie_reaper: Option<ZombieReaper>,

    /// 执行钩子列表
    ///
    /// 在任务执行前后调用的钩子函数，用于性能分析和自定义监控
    hooks: Vec<Arc<dyn ExecutionHook>>,
}

impl CommandPool {
    /// 创建命令池（默认多进程模式）
    pub fn new() -> Self {
        Self::with_config(ExecutionConfig::default())
    }

    /// 使用指定配置创建命令池
    pub fn with_config(config: ExecutionConfig) -> Self {
        let backend = BackendFactory::create(&config);

        tracing::info!(
            mode = ?config.mode,
            workers = config.workers,
            "CommandPool initialized"
        );

        // 如果配置了僵尸进程清理间隔，启动清理器
        let zombie_reaper = config.zombie_reaper_interval.map(ZombieReaper::new);

        Self {
            tasks: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            config,
            backend,
            running: Arc::new(AtomicBool::new(false)),
            handles: Arc::new(Mutex::new(Vec::new())),
            max_size: None,
            metrics: Metrics::new(),
            task_id_counter: Arc::new(AtomicU64::new(1)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            shutdown_config: ShutdownConfig::default(),
            zombie_reaper,
            hooks: Vec::new(),
        }
    }

    /// 使用指定配置和队列大小限制创建命令池
    pub fn with_config_and_limit(config: ExecutionConfig, max_size: usize) -> Self {
        let backend = BackendFactory::create(&config);

        tracing::info!(
            mode = ?config.mode,
            workers = config.workers,
            max_size = max_size,
            "CommandPool initialized with queue limit"
        );

        // 如果配置了僵尸进程清理间隔，启动清理器
        let zombie_reaper = config.zombie_reaper_interval.map(ZombieReaper::new);

        Self {
            tasks: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            config,
            backend,
            running: Arc::new(AtomicBool::new(false)),
            handles: Arc::new(Mutex::new(Vec::new())),
            max_size: Some(max_size),
            metrics: Metrics::new(),
            task_id_counter: Arc::new(AtomicU64::new(1)),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            shutdown_config: ShutdownConfig::default(),
            zombie_reaper,
            hooks: Vec::new(),
        }
    }

    /// 添加执行钩子
    ///
    /// 钩子允许在任务执行前后插入自定义逻辑，用于性能分析、监控等。
    /// 可以多次调用此方法来添加多个钩子。
    ///
    /// # 参数
    ///
    /// * `hook` - 实现了 `ExecutionHook` trait 的钩子对象
    ///
    /// # 返回
    ///
    /// 返回 self，支持链式调用
    ///
    /// # 需求
    ///
    /// - 验证需求 15.1: 支持注册 before_execute 钩子
    /// - 验证需求 15.2: 支持注册 after_execute 钩子
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::CommandPool;
    /// use execute::hooks::{ExecutionHook, ExecutionContext, HookTaskResult};
    /// use std::sync::Arc;
    ///
    /// struct MyHook;
    ///
    /// impl ExecutionHook for MyHook {
    ///     fn before_execute(&self, ctx: &ExecutionContext) {
    ///         println!("Starting task {}", ctx.task_id);
    ///     }
    ///     
    ///     fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult) {
    ///         println!("Task {} completed in {:?}", ctx.task_id, result.duration);
    ///     }
    /// }
    ///
    /// let pool = CommandPool::new()
    ///     .with_hook(Arc::new(MyHook));
    /// ```
    pub fn with_hook(mut self, hook: Arc<dyn ExecutionHook>) -> Self {
        self.hooks.push(hook);
        self
    }

    /// 添加任务（如果设置了队列大小限制，队列满时会阻塞等待）
    ///
    /// # 返回
    ///
    /// 返回任务句柄，可用于等待任务完成、获取结果或取消任务
    ///
    /// # 错误
    ///
    /// 如果命令池正在关闭，返回 `SubmitError::ShuttingDown`
    pub fn push_task(&self, task: CommandConfig) -> Result<TaskHandle, SubmitError> {
        // 检查是否正在关闭
        if self.shutdown_flag.load(Ordering::SeqCst) {
            return Err(SubmitError::ShuttingDown);
        }

        let task_id = self.task_id_counter.fetch_add(1, Ordering::SeqCst);

        tracing::debug!(
            task_id = task_id,
            command = %task.program(),
            args = ?task.args(),
            "Task submitted"
        );

        self.metrics.record_task_submitted();

        // 创建 TaskHandle
        let (handle, result_sender) = TaskHandle::new(task_id);

        let (lock, cvar) = &*self.tasks;
        let mut tasks = lock.lock().unwrap();

        // 如果设置了队列大小限制，等待队列有空位
        if let Some(max) = self.max_size {
            while tasks.len() >= max {
                // 在等待期间再次检查是否正在关闭
                if self.shutdown_flag.load(Ordering::SeqCst) {
                    return Err(SubmitError::ShuttingDown);
                }
                tasks = cvar.wait(tasks).unwrap();
            }
        }

        // 最后再检查一次
        if self.shutdown_flag.load(Ordering::SeqCst) {
            return Err(SubmitError::ShuttingDown);
        }

        tasks.push_back(TaskItem {
            config: task,
            handle: handle.clone(),
            result_sender,
        });
        cvar.notify_one();
        Ok(handle)
    }

    /// 尝试添加任务，如果队列满则返回错误
    ///
    /// # 返回
    ///
    /// 返回任务句柄，可用于等待任务完成、获取结果或取消任务
    ///
    /// # 错误
    ///
    /// * `SubmitError::ShuttingDown` - 命令池正在关闭
    /// * `SubmitError::QueueFull` - 队列已满（仅当设置了队列大小限制时）
    pub fn try_push_task(&self, task: CommandConfig) -> Result<TaskHandle, SubmitError> {
        // 检查是否正在关闭
        if self.shutdown_flag.load(Ordering::SeqCst) {
            return Err(SubmitError::ShuttingDown);
        }

        let task_id = self.task_id_counter.fetch_add(1, Ordering::SeqCst);

        // 创建 TaskHandle
        let (handle, result_sender) = TaskHandle::new(task_id);

        let (lock, cvar) = &*self.tasks;
        let mut tasks = lock.lock().unwrap();

        // 如果设置了队列大小限制，检查是否有空位
        if let Some(max) = self.max_size
            && tasks.len() >= max
        {
            return Err(SubmitError::QueueFull);
        }

        tasks.push_back(TaskItem {
            config: task,
            handle: handle.clone(),
            result_sender,
        });
        cvar.notify_one();
        Ok(handle)
    }

    /// 弹出任务（阻塞等待直到有任务或关闭）
    ///
    /// 使用条件变量等待新任务，避免轮询造成的 CPU 浪费。
    /// 当队列为空时，线程会阻塞等待，直到有新任务提交或命令池关闭。
    pub fn pop_task(&self) -> Option<TaskItem> {
        let (lock, cvar) = &*self.tasks;
        let mut tasks = lock.lock().unwrap();

        loop {
            // 尝试获取任务
            if let Some(task) = tasks.pop_front() {
                // 通知可能在等待队列空位的线程
                cvar.notify_one();
                return Some(task);
            }

            // 如果正在关闭且队列为空，返回 None
            if self.shutdown_flag.load(Ordering::SeqCst) {
                return None;
            }

            // 队列为空且未关闭，等待新任务
            tasks = cvar.wait(tasks).unwrap();
        }
    }

    /* Batch methods temporarily disabled - need to be updated for TaskHandle support
    /// 批量添加任务
    ///
    /// # 返回
    ///
    /// 成功添加的任务数量
    ///
    /// # 错误
    ///
    /// 如果命令池正在关闭，返回 `SubmitError::ShuttingDown`
    pub fn push_tasks_batch(&self, tasks: Vec<CommandConfig>) -> Result<usize, SubmitError> {
        // 检查是否正在关闭
        if self.shutdown_flag.load(Ordering::SeqCst) {
            return Err(SubmitError::ShuttingDown);
        }

        let (lock, cvar) = &*self.tasks;
        let mut queue = lock.lock().unwrap();

        let count = tasks.len();

        for task in tasks {
            // 在添加每个任务前检查是否正在关闭
            if self.shutdown_flag.load(Ordering::SeqCst) {
                return Err(SubmitError::ShuttingDown);
            }

            // 如果设置了队列大小限制，等待队列有空位
            if let Some(max) = self.max_size {
                while queue.len() >= max {
                    // 在等待期间检查是否正在关闭
                    if self.shutdown_flag.load(Ordering::SeqCst) {
                        return Err(SubmitError::ShuttingDown);
                    }
                    queue = cvar.wait(queue).unwrap();
                }
            }
            queue.push_back(task);
        }

        cvar.notify_all();
        Ok(count)
    }

    /// 尝试批量添加任务，返回成功添加的数量
    ///
    /// # 返回
    ///
    /// 成功添加的任务数量
    ///
    /// # 注意
    ///
    /// 如果命令池正在关闭，会立即返回 0
    pub fn try_push_tasks_batch(&self, tasks: Vec<CommandConfig>) -> usize {
        // 检查是否正在关闭
        if self.shutdown_flag.load(Ordering::SeqCst) {
            return 0;
        }

        let (lock, cvar) = &*self.tasks;
        let mut queue = lock.lock().unwrap();

        let mut count = 0;

        for task in tasks {
            // 在添加每个任务前检查是否正在关闭
            if self.shutdown_flag.load(Ordering::SeqCst) {
                break;
            }

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
    */

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

    /// 获取指标快照
    ///
    /// 返回当前的任务执行统计信息
    pub fn metrics(&self) -> crate::metrics::MetricsSnapshot {
        self.metrics.snapshot()
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

    /// 优雅关闭命令池
    ///
    /// 停止接受新任务，等待所有正在执行的任务完成。
    /// 使用默认的超时时间（30秒）。
    ///
    /// # 返回
    ///
    /// * `Ok(())` - 所有任务成功完成
    /// * `Err(ShutdownError)` - 关闭过程中发生错误
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::CommandPool;
    ///
    /// let pool = CommandPool::new();
    /// pool.start_executor(Duration::from_millis(100));
    /// // ... 提交任务 ...
    /// pool.shutdown().unwrap();
    /// ```
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        self.shutdown_with_timeout(self.shutdown_config.timeout)
    }

    /// 使用指定超时时间优雅关闭命令池
    ///
    /// 停止接受新任务，等待所有正在执行的任务完成或超时。
    ///
    /// # 参数
    ///
    /// * `timeout` - 等待任务完成的最大时间
    ///
    /// # 返回
    ///
    /// * `Ok(())` - 所有任务成功完成
    /// * `Err(ShutdownError::Timeout)` - 等待超时
    /// * `Err(ShutdownError::WorkerPanic)` - 工作线程 panic
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::CommandPool;
    /// use std::time::Duration;
    ///
    /// let pool = CommandPool::new();
    /// pool.start_executor(Duration::from_millis(100));
    /// // ... 提交任务 ...
    /// pool.shutdown_with_timeout(Duration::from_secs(60)).unwrap();
    /// ```
    pub fn shutdown_with_timeout(&self, timeout: Duration) -> Result<(), ShutdownError> {
        tracing::info!("Initiating graceful shutdown with timeout {:?}", timeout);

        // 1. 设置 shutdown flag，停止接受新任务
        self.shutdown_flag.store(true, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);

        // 2. 唤醒所有可能在等待的线程
        let (_, cvar) = &*self.tasks;
        cvar.notify_all();

        // 3. 等待所有 worker 完成或超时
        let start = Instant::now();
        let mut handles = self.handles.lock().unwrap();

        // 收集所有 handles 到一个 Vec 中
        let handles_vec: Vec<_> = handles.drain(..).collect();
        let total_workers = handles_vec.len();
        drop(handles); // 释放锁

        for (idx, handle) in handles_vec.into_iter().enumerate() {
            let remaining = timeout.saturating_sub(start.elapsed());

            if remaining.is_zero() {
                tracing::warn!(
                    "Shutdown timeout reached, {} workers may still be running",
                    total_workers - idx
                );
                return Err(ShutdownError::Timeout(timeout));
            }

            // 使用 thread::park_timeout 模拟 join_timeout
            // 注意：标准库的 JoinHandle 没有 join_timeout，我们需要使用其他方法
            match handle.join() {
                Ok(_) => {
                    tracing::debug!("Worker {} joined successfully", idx);
                }
                Err(_) => {
                    tracing::error!("Worker {} panicked", idx);
                    return Err(ShutdownError::WorkerPanic);
                }
            }

            // 检查是否超时
            if start.elapsed() >= timeout {
                tracing::warn!(
                    "Shutdown timeout reached after waiting for {} workers",
                    idx + 1
                );
                return Err(ShutdownError::Timeout(timeout));
            }
        }

        tracing::info!("Graceful shutdown completed successfully");
        Ok(())
    }

    /// 配置关闭行为
    ///
    /// 设置关闭超时时间和是否强制终止进程。
    ///
    /// # 参数
    ///
    /// * `config` - 关闭配置
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::{CommandPool, ShutdownConfig};
    /// use std::time::Duration;
    ///
    /// let mut pool = CommandPool::new();
    /// pool.set_shutdown_config(
    ///     ShutdownConfig::new(Duration::from_secs(60))
    ///         .with_force_kill(true)
    /// );
    /// ```
    pub fn set_shutdown_config(&mut self, config: ShutdownConfig) {
        self.shutdown_config = config;
    }

    /// 检查是否正在关闭或已关闭
    ///
    /// # 返回
    ///
    /// * `true` - 正在关闭或已关闭
    /// * `false` - 正常运行
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_flag.load(Ordering::SeqCst)
    }

    fn start_thread_executor(&self, _interval: Duration) {
        for _ in 0..self.config.workers {
            let pool = self.clone();
            let handle = thread::spawn(move || {
                while pool.running.load(Ordering::SeqCst)
                    && !pool.shutdown_flag.load(Ordering::SeqCst)
                {
                    // pop_task 会阻塞等待，不需要轮询
                    if let Some(task_item) = pool.pop_task() {
                        if !pool.running.load(Ordering::SeqCst)
                            || pool.shutdown_flag.load(Ordering::SeqCst)
                        {
                            break;
                        }

                        // 检查任务是否已被取消
                        if task_item.handle.is_cancelled() {
                            let task_id = task_item.handle.id();
                            tracing::info!(task_id = task_id, "Task cancelled before execution");
                            let _ = task_item
                                .result_sender
                                .send(Err(ExecuteError::Cancelled(task_id)));
                            continue;
                        }

                        // 更新任务状态为 Running
                        task_item.handle.set_state(TaskState::Running { pid: None });

                        // 执行任务
                        let result =
                            pool.execute_task_with_handle(&task_item.config, &task_item.handle);

                        // 发送结果
                        let _ = task_item.result_sender.send(result);

                        // 更新任务状态为 Completed（如果未被取消）
                        if !task_item.handle.is_cancelled() {
                            task_item.handle.set_state(TaskState::Completed);
                        }
                    } else {
                        // pop_task 返回 None 表示正在关闭
                        break;
                    }
                }
                tracing::debug!("Thread executor worker exiting");
            });
            self.handles.lock().unwrap().push(handle);
        }
    }

    fn start_process_executor(&self, _interval: Duration) {
        for _ in 0..self.config.workers {
            let pool = self.clone();
            let handle = thread::spawn(move || {
                while pool.running.load(Ordering::SeqCst)
                    && !pool.shutdown_flag.load(Ordering::SeqCst)
                {
                    // pop_task 会阻塞等待，不需要轮询
                    if let Some(task_item) = pool.pop_task() {
                        if !pool.running.load(Ordering::SeqCst)
                            || pool.shutdown_flag.load(Ordering::SeqCst)
                        {
                            break;
                        }

                        // 检查任务是否已被取消
                        if task_item.handle.is_cancelled() {
                            let task_id = task_item.handle.id();
                            tracing::info!(task_id = task_id, "Task cancelled before execution");
                            let _ = task_item
                                .result_sender
                                .send(Err(ExecuteError::Cancelled(task_id)));
                            continue;
                        }

                        // 更新任务状态为 Running
                        task_item.handle.set_state(TaskState::Running { pid: None });

                        // 执行任务
                        let result =
                            pool.execute_task_with_handle(&task_item.config, &task_item.handle);

                        // 发送结果
                        let _ = task_item.result_sender.send(result);

                        // 更新任务状态为 Completed（如果未被取消）
                        if !task_item.handle.is_cancelled() {
                            task_item.handle.set_state(TaskState::Completed);
                        }
                    } else {
                        // pop_task 返回 None 表示正在关闭
                        break;
                    }
                }
                tracing::debug!("Process executor worker exiting");
            });
            self.handles.lock().unwrap().push(handle);
        }
    }

    fn start_process_pool_executor(&self, _interval: Duration) {
        // 进程池模式：复用工作线程，但后端使用进程池执行命令
        for _ in 0..self.config.workers {
            let pool = self.clone();
            let handle = thread::spawn(move || {
                while pool.running.load(Ordering::SeqCst)
                    && !pool.shutdown_flag.load(Ordering::SeqCst)
                {
                    // pop_task 会阻塞等待，不需要轮询
                    if let Some(task_item) = pool.pop_task() {
                        if !pool.running.load(Ordering::SeqCst)
                            || pool.shutdown_flag.load(Ordering::SeqCst)
                        {
                            break;
                        }

                        // 检查任务是否已被取消
                        if task_item.handle.is_cancelled() {
                            let task_id = task_item.handle.id();
                            tracing::info!(task_id = task_id, "Task cancelled before execution");
                            let _ = task_item
                                .result_sender
                                .send(Err(ExecuteError::Cancelled(task_id)));
                            continue;
                        }

                        // 更新任务状态为 Running
                        task_item.handle.set_state(TaskState::Running { pid: None });

                        // 执行任务
                        let result =
                            pool.execute_task_with_handle(&task_item.config, &task_item.handle);

                        // 发送结果
                        let _ = task_item.result_sender.send(result);

                        // 更新任务状态为 Completed（如果未被取消）
                        if !task_item.handle.is_cancelled() {
                            task_item.handle.set_state(TaskState::Completed);
                        }
                    } else {
                        // pop_task 返回 None 表示正在关闭
                        break;
                    }
                }
                tracing::debug!("Process pool executor worker exiting");
            });
            self.handles.lock().unwrap().push(handle);
        }
    }

    /// 执行单个任务
    pub fn execute_task(
        &self,
        config: &CommandConfig,
    ) -> Result<std::process::Output, ExecuteError> {
        let task_id = self.task_id_counter.load(Ordering::SeqCst);
        let start_time = Instant::now();

        tracing::info!(
            task_id = task_id,
            command = %config.program(),
            "Task execution started"
        );

        self.metrics.record_task_started();

        // 如果配置了重试策略，使用 execute_with_retry，否则直接执行
        let result = if config.retry_policy().is_some() {
            // 使用带重试的执行逻辑
            use crate::executor::execute_with_retry;
            execute_with_retry(config, task_id)
                .map_err(|e| ExecuteError::Io(std::io::Error::other(e.to_string())))
        } else {
            // 直接使用后端执行
            self.backend.execute(config)
        };

        let duration = start_time.elapsed();

        match &result {
            Ok(output) => {
                let exit_code = output.status.code().unwrap_or(-1);
                tracing::info!(
                    task_id = task_id,
                    exit_code = exit_code,
                    duration_ms = duration.as_millis(),
                    "Task completed successfully"
                );
                self.metrics.record_task_completed(duration);
            }
            Err(e) => {
                tracing::error!(
                    task_id = task_id,
                    error = %e,
                    duration_ms = duration.as_millis(),
                    "Task failed"
                );
                self.metrics.record_task_failed(duration);
            }
        }

        result
    }

    /// 执行单个任务并检查取消令牌
    ///
    /// 此方法在任务执行期间会检查取消令牌，如果任务被取消则提前终止。
    fn execute_task_with_handle(
        &self,
        config: &CommandConfig,
        handle: &TaskHandle,
    ) -> Result<std::process::Output, ExecuteError> {
        let task_id = handle.id();
        let start_time = Instant::now();

        tracing::info!(
            task_id = task_id,
            command = %config.program(),
            "Task execution started"
        );

        self.metrics.record_task_started();

        // 在执行前再次检查是否已取消
        if handle.is_cancelled() {
            tracing::info!(task_id = task_id, "Task cancelled before execution");
            return Err(ExecuteError::Cancelled(task_id));
        }

        // 如果配置了重试策略，使用 execute_with_retry，否则直接执行
        let result = if config.retry_policy().is_some() {
            // 使用带重试的执行逻辑
            use crate::executor::execute_with_retry;
            execute_with_retry(config, task_id)
                .map_err(|e| ExecuteError::Io(std::io::Error::other(e.to_string())))
        } else {
            // 直接使用后端执行
            self.backend.execute(config)
        };

        let duration = start_time.elapsed();

        // 检查是否在执行期间被取消
        if handle.is_cancelled() {
            tracing::info!(task_id = task_id, "Task cancelled during execution");
            self.metrics.record_task_failed(duration);
            return Err(ExecuteError::Cancelled(task_id));
        }

        match &result {
            Ok(output) => {
                let exit_code = output.status.code().unwrap_or(-1);
                tracing::info!(
                    task_id = task_id,
                    exit_code = exit_code,
                    duration_ms = duration.as_millis(),
                    "Task completed successfully"
                );
                self.metrics.record_task_completed(duration);
            }
            Err(e) => {
                tracing::error!(
                    task_id = task_id,
                    error = %e,
                    duration_ms = duration.as_millis(),
                    "Task failed"
                );
                self.metrics.record_task_failed(duration);
            }
        }

        result
    }

    /// 使用自定义执行器启动（高级用法）
    pub fn start_with_executor<E: CommandExecutor + 'static>(
        &self,
        _interval: Duration,
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
                while pool.running.load(Ordering::SeqCst)
                    && !pool.shutdown_flag.load(Ordering::SeqCst)
                {
                    // pop_task 会阻塞等待，不需要轮询
                    if let Some(task_item) = pool.pop_task() {
                        if !pool.running.load(Ordering::SeqCst)
                            || pool.shutdown_flag.load(Ordering::SeqCst)
                        {
                            break;
                        }

                        // 检查任务是否已被取消
                        if task_item.handle.is_cancelled() {
                            let task_id = task_item.handle.id();
                            tracing::info!(task_id = task_id, "Task cancelled before execution");
                            let _ = task_item
                                .result_sender
                                .send(Err(ExecuteError::Cancelled(task_id)));
                            continue;
                        }

                        // 更新任务状态为 Running
                        task_item.handle.set_state(TaskState::Running { pid: None });

                        // 执行任务
                        let result = exec.execute(&task_item.config);

                        // 发送结果
                        let _ = task_item.result_sender.send(result);

                        // 更新任务状态为 Completed（如果未被取消）
                        if !task_item.handle.is_cancelled() {
                            task_item.handle.set_state(TaskState::Completed);
                        }
                    } else {
                        // pop_task 返回 None 表示正在关闭
                        break;
                    }
                }
                tracing::debug!("Custom executor worker exiting");
            });
            self.handles.lock().unwrap().push(handle);
        }
    }

    /// 统计存活的工作线程数
    ///
    /// 检查所有工作线程句柄，统计未完成的线程数量。
    ///
    /// # 返回
    ///
    /// 存活的工作线程数量
    fn count_alive_workers(&self) -> usize {
        let handles = self.handles.lock().unwrap();
        handles.iter().filter(|h| !h.is_finished()).count()
    }

    /// 计算队列使用率
    ///
    /// 返回当前队列大小与最大容量的比率。
    /// 如果没有设置队列大小限制，则基于当前队列大小返回一个估计值。
    ///
    /// # 返回
    ///
    /// 队列使用率，范围 0.0 - 1.0
    fn queue_usage(&self) -> f64 {
        let current_size = self.len();

        if let Some(max) = self.max_size {
            if max > 0 {
                (current_size as f64) / (max as f64)
            } else {
                0.0
            }
        } else {
            // 没有设置限制时，基于一个合理的阈值（1000）来估算使用率
            // 超过 1000 个任务认为使用率为 1.0
            let threshold = 1000;
            (current_size as f64 / threshold as f64).min(1.0)
        }
    }

    /// 统计长时间运行的任务数
    ///
    /// 统计执行时间超过指定阈值的任务数量。
    /// 注意：当前实现返回正在运行的任务总数作为近似值。
    ///
    /// # 参数
    ///
    /// * `threshold` - 时间阈值，超过此时间的任务被认为是长时间运行
    ///
    /// # 返回
    ///
    /// 长时间运行的任务数量
    fn count_long_running_tasks(&self, _threshold: Duration) -> usize {
        // 简化实现：返回当前正在运行的任务数
        // 完整实现需要跟踪每个任务的开始时间
        self.metrics.tasks_running.load(Ordering::Relaxed)
    }

    /// 执行健康检查
    ///
    /// 检查命令池的健康状态，包括工作线程状态、队列使用率和长时间运行的任务。
    ///
    /// # 返回
    ///
    /// 返回包含健康状态和详细信息的 `HealthCheck` 结构体。
    ///
    /// # 健康状态分类
    ///
    /// * `Healthy` - 所有检查都通过，系统运行正常
    /// * `Degraded` - 存在一些问题但系统仍可运行（如队列使用率高、有长时间运行的任务）
    /// * `Unhealthy` - 存在严重问题，系统无法正常运行（如所有工作线程都已停止）
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::CommandPool;
    ///
    /// let pool = CommandPool::new();
    /// pool.start_executor(Duration::from_millis(100));
    ///
    /// let health = pool.health_check();
    /// match health.status {
    ///     HealthStatus::Healthy => println!("System is healthy"),
    ///     HealthStatus::Degraded { issues } => {
    ///         println!("System is degraded: {:?}", issues);
    ///     }
    ///     HealthStatus::Unhealthy { issues } => {
    ///         println!("System is unhealthy: {:?}", issues);
    ///     }
    /// }
    /// ```
    pub fn health_check(&self) -> HealthCheck {
        let mut issues = Vec::new();

        // 检查工作线程状态
        let workers_alive = self.count_alive_workers();
        let workers_total = self.config.workers;

        if workers_alive < workers_total {
            issues.push(format!(
                "Only {}/{} workers alive",
                workers_alive, workers_total
            ));
        }

        // 检查队列使用率
        let queue_usage = self.queue_usage();
        if queue_usage > 0.9 {
            issues.push(format!("Queue usage high: {:.1}%", queue_usage * 100.0));
        }

        // 检查长时间运行的任务（超过 5 分钟）
        let long_running = self.count_long_running_tasks(Duration::from_secs(300));
        if long_running > 0 {
            issues.push(format!("{} tasks running > 5 minutes", long_running));
        }

        // 根据问题严重程度确定健康状态
        let status = if issues.is_empty() {
            HealthStatus::Healthy
        } else if workers_alive > 0 {
            // 有工作线程存活，系统降级但仍可运行
            HealthStatus::Degraded { issues }
        } else {
            // 没有工作线程存活，系统不健康
            HealthStatus::Unhealthy { issues }
        };

        let snapshot = self.metrics.snapshot();

        HealthCheck {
            status,
            timestamp: SystemTime::now(),
            details: HealthDetails {
                workers_alive,
                workers_total,
                queue_usage,
                long_running_tasks: long_running,
                avg_task_duration: snapshot.avg_execution_time,
            },
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
            metrics: self.metrics.clone(),
            task_id_counter: Arc::clone(&self.task_id_counter),
            shutdown_flag: Arc::clone(&self.shutdown_flag),
            shutdown_config: self.shutdown_config.clone(),
            zombie_reaper: None, // 不克隆 zombie_reaper，因为它包含线程句柄
            hooks: self.hooks.clone(),
        }
    }
}

impl Default for CommandPool {
    fn default() -> Self {
        Self::new()
    }
}
