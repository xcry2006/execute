use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::config::CommandConfig;
use crate::error::ExecuteError;

/// 线程任务 trait | Thread task trait
///
/// 定义可以在线程池中执行的任务接口。
pub trait ThreadTask: Send + 'static {
    /// 执行任务
    fn execute(&self) -> Result<(), ExecuteError>;
}

/// 线程池执行器 | Thread pool executor
///
/// 管理一组工作线程，在共享进程内并发执行任务。
/// 与多进程模式不同，线程模式共享内存空间，适合计算密集型任务。
#[derive(Clone)]
pub struct ThreadExecutor {
    workers: usize,
    tasks: Arc<Mutex<Vec<Box<dyn ThreadTask>>>>,
}

impl ThreadExecutor {
    /// 创建新的线程执行器
    ///
    /// # 参数
    /// - `workers`: 工作线程数
    pub fn new(workers: usize) -> Self {
        Self {
            workers,
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 提交任务到线程池
    ///
    /// # 参数
    /// - `task`: 要实现 ThreadTask trait 的任务
    pub fn submit<T: ThreadTask>(&self, task: T) {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.push(Box::new(task));
    }

    /// 启动执行器
    ///
    /// 启动指定数量的工作线程，定期轮询任务队列。
    ///
    /// # 参数
    /// - `interval`: 轮询间隔
    pub fn start(&self, interval: Duration) {
        for _ in 0..self.workers {
            let tasks = self.tasks.clone();
            thread::spawn(move || {
                loop {
                    let task_opt = {
                        let mut guard = tasks.lock().unwrap();
                        guard.pop()
                    };
                    if let Some(task) = task_opt {
                        let _ = task.execute();
                    } else {
                        thread::sleep(interval);
                    }
                }
            });
        }
    }

    /// 获取工作线程数
    pub fn workers(&self) -> usize {
        self.workers
    }
}

impl Default for ThreadExecutor {
    fn default() -> Self {
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self::new(workers)
    }
}

/// 命令任务包装器 | Command task wrapper
///
/// 将 CommandConfig 包装为可在线程池中执行的任务。
/// 在线程模式下，命令通过 std::process::Command 执行。
#[derive(Clone)]
pub struct CommandTask {
    config: CommandConfig,
}

impl CommandTask {
    /// 创建新的命令任务
    pub fn new(config: CommandConfig) -> Self {
        Self { config }
    }
}

impl ThreadTask for CommandTask {
    fn execute(&self) -> Result<(), ExecuteError> {
        // 在线程模式下，我们仍然使用子进程执行外部命令
        // 但任务调度是在线程池中进行的
        crate::executor::execute_command(&self.config)?;
        Ok(())
    }
}

/// 线程模式执行器 | Thread mode executor
///
/// 专门用于在线程模式下执行命令的执行器。
#[derive(Clone)]
pub struct ThreadModeExecutor {
    executor: Arc<ThreadExecutor>,
}

impl ThreadModeExecutor {
    /// 创建新的线程模式执行器
    pub fn new(workers: usize) -> Self {
        Self {
            executor: Arc::new(ThreadExecutor::new(workers)),
        }
    }

    /// 启动执行器
    pub fn start(&self, interval: Duration) {
        self.executor.start(interval);
    }

    /// 提交命令任务
    pub fn submit(&self, config: CommandConfig) {
        self.executor.submit(CommandTask::new(config));
    }
}

impl Default for ThreadModeExecutor {
    fn default() -> Self {
        Self::new(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        )
    }
}
