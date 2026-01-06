use std::collections::VecDeque;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex, mpsc, Condvar};
use std::thread;
use std::time::Duration;
use thiserror::Error;
use crossbeam_queue::SegQueue;

/// CommandConfig 表示要执行的外部命令及其执行参数。
///
/// 字段：
/// - `program`: 可执行程序名或路径。
/// - `args`: 传递给程序的参数列表。
/// - `working_dir`: 可选的工作目录，若为 `None` 则使用当前目录。
/// - `timeout`: 可选的超时时间，超时后会尝试终止子进程。
///
/// 示例（构造一个带超时的命令配置）：
/// ```ignore
/// let cfg = CommandConfig::new("sleep", vec!["5".to_string()]).timeout(Duration::from_secs(2));
/// ```
#[derive(Debug, Clone)]
pub struct CommandConfig {
    program: String,
    args: Vec<String>,
    working_dir: Option<String>,
    timeout: Option<Duration>,
}

/// ExecuteError 表示在启动或等待子进程过程中可能遇到的错误。
///
/// 常见变体包括 IO 错误、通道接收错误以及超时错误。
#[derive(Error, Debug)]
pub enum ExecuteError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("channel receive error: {0}")]
    Recv(#[from] mpsc::RecvError),

    #[error("channel recv_timeout error: {0}")]
    RecvTimeout(#[from] mpsc::RecvTimeoutError),

    #[error("command execution timed out after {0:?}")]
    Timeout(Duration),

    #[error("child process error: {0}")]
    Child(String),
}

impl CommandConfig {
    /// # 创建一个CommandConfig结构体
    ///
    /// # 参数
    /// - `program`: 执行的命令
    /// - `args`: 命令参数列表
    ///
    /// # 示例
    /// ```ignore
    /// let cfg = CommandConfig::new("echo", vec!["hello".to_string()]);
    /// println!("program = {}", cfg.program());
    /// ```
    pub fn new(program: &str, args: Vec<String>) -> Self {
        Self {
            program: program.to_string(),
            args,
            working_dir: None,
            timeout: Some(Duration::from_secs(10)),
        }
    }

    /// # 设置任务的工作目录
    ///
    /// 将命令的工作目录设置为给定路径，返回修改后的 `CommandConfig`，便于链式调用。
    ///
    /// # 参数
    /// - `dir`: 要在其中执行命令的工作目录路径。
    ///
    /// # 示例
    /// ```
    /// let cmd = CommandConfig::new("ls", vec!["-la".to_string()]).with_working_dir("/tmp");
    /// assert_eq!(cmd.working_dir().unwrap(), "/tmp".to_string());
    /// ```
    pub fn with_working_dir(mut self, dir: &str) -> Self {
        self.working_dir = Some(dir.to_string());
        self
    }

    /// # 设置任务超时时间
    ///
    /// 为该命令设置最大执行时长，超时后会尝试终止子进程并返回 `ExecuteError::Timeout`。
    ///
    /// # 参数
    /// - `timeout`: 超时时间长度，`Duration` 类型。
    ///
    /// # 示例
    /// ```
    /// let cmd = CommandConfig::new("sleep", vec!["5".to_string()]).with_timeout(Duration::from_secs(2));
    /// assert_eq!(cmd.timeout().unwrap().as_secs(), 2);
    /// ```
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// # 获取程序名
    pub fn program(&self) -> &str {
        &self.program
    }

    /// # 获取命令参数
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// # 获取工作目录
    pub fn working_dir(&self) -> Option<&str> {
        self.working_dir.as_deref()
    }

    /// # 获取超时时间
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }
}

#[derive(Clone)]
pub struct CommandPool {
    tasks: Arc<Mutex<VecDeque<CommandConfig>>>,
}

/// `CommandPool` 是一个简单的命令队列，支持多线程生产任务并由后台执行器消费执行。
///
/// 使用示例：
/// ```ignore
/// let pool = CommandPool::new();
/// pool.push_task(CommandConfig::new("echo", vec!["hi".to_string()]));
/// pool.start_executor(Duration::from_secs(1));
/// ```
impl CommandPool {
    /// # 创建一个CommandPool命令池
    ///
    /// # 示例
    /// ```
    /// let pool = CommandPool::new();
    /// ```
    ///
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// # 添加任务到命令池
    ///
    /// 将给定的 `CommandConfig` 推入命令池的队尾，等待执行器轮询时被取出执行。
    ///
    /// # 参数
    /// - `task`: 要添加到池中的 `CommandConfig` 实例。
    ///
    /// # 示例
    /// ```
    /// let pool = CommandPool::new();
    /// pool.push_task(CommandConfig::new("echo", vec!["hi".to_string()]));
    /// ```
    pub fn push_task(&self, task: CommandConfig) {
        let mut tasks = self.tasks.lock().expect("命令池锁获取失败");
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
        let mut tasks = self.tasks.lock().expect("命令池锁获取失败");
        tasks.pop_front()
    }

    /// # 池是否为空
    ///
    /// 返回当前命令池是否没有待处理任务。
    pub fn is_empty(&self) -> bool {
        let tasks = self.tasks.lock().expect("命令池锁获取失败");
        tasks.is_empty()
    }

    /// # 启动定时执行器
    ///
    /// 在后台线程中启动轮询执行器，按指定 `interval` 轮询命令池并执行任务。
    ///
    /// # 参数
    /// - `interval`: 两次轮询之间的间隔时间。
    ///
    /// # 示例
    /// ```
    /// let pool = CommandPool::new();
    /// pool.start_executor(Duration::from_secs(1));
    /// ```
    pub fn start_executor(&self, interval: Duration) {
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        self.start_executor_with_workers(interval, workers);
    }

    /// 启动具有固定工作线程数的执行器以复用线程并发执行任务。
    pub fn start_executor_with_workers(&self, interval: Duration, workers: usize) {
        for _ in 0..workers {
            let pool_clone = self.clone();
            let interval = interval;
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
    pub fn start_executor_with_workers_and_limit(&self, interval: Duration, workers: usize, limit: usize) {
        let sem = Arc::new(Semaphore::new(limit));
        for _ in 0..workers {
            let pool_clone = self.clone();
            let sem = sem.clone();
            let interval = interval;
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool_clone.pop_task() {
                        sem.acquire();
                        let _ = pool_clone.execute_task(&task);
                        sem.release();
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
    pub fn execute_task(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        execute_command(config)
    }
}



/// 执行单个命令配置 | Execute a single command configuration
///
/// 内部函数，用于启动子进程并处理超时。使用 wait-timeout crate 在同一线程中进行超时等待，
/// 避免为每个任务生成额外的等待线程，提高性能和降低系统开销。
fn execute_command(config: &CommandConfig) -> Result<Output, ExecuteError> {
    // 启动子进程，重定向 stdout 和 stderr | Spawn child with piped stdout/stderr
    let mut cmd = Command::new(&config.program);
    cmd.args(&config.args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = &config.working_dir {
        cmd.current_dir(dir);
    }

    let mut child = cmd.spawn()?;

    // 根据是否设置超时进行等待处理 | Handle waiting based on timeout configuration
    match config.timeout {
        Some(timeout) => {
            // 使用 wait-timeout 在当前线程中等待，不产生额外线程
            // Use wait-timeout for in-thread waiting without spawning extra threads
            use wait_timeout::ChildExt;
            match child.wait_timeout(timeout).map_err(|e| ExecuteError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))? {
                Some(_) => {
                    // 子进程在超时前正常退出，收集输出 | Child exited within timeout; collect output
                    let output = child.wait_with_output()?;
                    Ok(output)
                }
                None => {
                    // 超时：尝试杀死子进程 | Timeout: attempt to kill the child process
                    let _ = child.kill();
                    let _ = child.wait();
                    Err(ExecuteError::Timeout(timeout))
                }
            }
        }
        None => {
            // 无超时限制，直接等待子进程完成 | No timeout: wait and collect without limit
            let output = child.wait_with_output()?;
            Ok(output)
        }
    }
}

/// 基于无锁队列（SegQueue）的命令池 | Lock-free command pool using SegQueue
///
/// 相比 CommandPool 的 Mutex-based 实现，SegQueue 提供更高的并发吞吐量。
/// 特别是在多生产者场景下性能更优（避免了锁竞争）。
#[derive(Clone)]
pub struct CommandPoolSeg {
    tasks: Arc<SegQueue<CommandConfig>>,
}

impl CommandPoolSeg {
    /// 创建一个新的无锁命令池 | Create a new lock-free command pool
    pub fn new() -> Self {
        Self { tasks: Arc::new(SegQueue::new()) }
    }

    /// 无阻塞地推入任务 | Push a task without blocking (lock-free)
    pub fn push_task(&self, task: CommandConfig) {
        self.tasks.push(task);
    }

    /// 无阻塞地尝试弹出任务 | Try to pop a task without blocking
    pub fn pop_task(&self) -> Option<CommandConfig> {
        self.tasks.pop()
    }

    /// 返回队列是否为空
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
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
            let interval = interval;
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool.pop_task() {
                        let _ = execute_command(&task);
                    }
                    thread::sleep(interval);
                }
            });
        }
    }

    /// 启动限制并发的执行器 | Start executor with concurrency limit
    ///
    /// 使用信号量限制同时执行的外部进程数量，防止资源耗尽。
    pub fn start_executor_with_workers_and_limit(&self, interval: Duration, workers: usize, limit: usize) {
        let sem = Arc::new(Semaphore::new(limit));
        for _ in 0..workers {
            let pool = self.clone();
            let sem = sem.clone();
            let interval = interval;
            thread::spawn(move || {
                loop {
                    while let Some(task) = pool.pop_task() {
                        // 获取信号量许可证，限制并发执行数量
                        // Acquire semaphore permit to enforce concurrency limit
                        sem.acquire();
                        let _ = execute_command(&task);
                        // 释放信号量许可证 | Release semaphore permit
                        sem.release();
                    }
                    thread::sleep(interval);
                }
            });
        }
    }

}


/// 简单的计数信号量 | Simple counting semaphore
///
/// 基于 `Mutex` 和 `Condvar` 实现，用于轻量级的并发执行控制。
/// 限制同时执行的外部子进程数量，防止系统资源耗尽。
pub struct Semaphore {
    inner: Arc<(Mutex<usize>, Condvar)>,
}

impl Semaphore {
    /// 创建一个信号量，初始许可证数为 `permits` | Create a semaphore with initial permits
    pub fn new(permits: usize) -> Self {
        Self { inner: Arc::new((Mutex::new(permits), Condvar::new())) }
    }

    /// 获取一个许可证，若许可证数为 0 则阻塞等待 | Acquire a permit, blocking if none available
    pub fn acquire(&self) {
        let (lock, cvar) = &*self.inner;
        let mut cnt = lock.lock().expect("semaphore lock");
        // 自旋等待直到有可用许可证 | Spin-wait until a permit is available
        while *cnt == 0 {
            cnt = cvar.wait(cnt).expect("semaphore wait");
        }
        *cnt -= 1;
    }

    /// 释放一个许可证，唤醒等待的线程 | Release a permit and wake up waiting threads
    pub fn release(&self) {
        let (lock, cvar) = &*self.inner;
        let mut cnt = lock.lock().expect("semaphore lock");
        *cnt += 1;
        // 通知一个等待线程 | Notify one waiting thread
        cvar.notify_one();
    }
}
 
