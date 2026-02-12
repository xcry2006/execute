use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_queue::SegQueue;

use crate::config::CommandConfig;
use crate::executor::{execute_command, CommandExecutor};
use crate::semaphore::Semaphore;

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
            thread::spawn(move || loop {
                while let Some(task) = pool.pop_task() {
                    let _ = execute_command(&task);
                }
                thread::sleep(interval);
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
            let interval = interval;
            thread::spawn(move || loop {
                while let Some(task) = pool.pop_task() {
                    let _permit = sem.acquire_guard();
                    let _ = execute_command(&task);
                }
                thread::sleep(interval);
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
            let interval = interval;
            thread::spawn(move || loop {
                while let Some(task) = pool.pop_task() {
                    let _ = executor.execute(&task);
                }
                thread::sleep(interval);
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
            let interval = interval;
            thread::spawn(move || loop {
                while let Some(task) = pool.pop_task() {
                    let _permit = sem.acquire_guard();
                    let _ = executor.execute(&task);
                }
                thread::sleep(interval);
            });
        }
    }
}

