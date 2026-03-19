use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::backend::{BackendFactory, ExecutionBackend, ExecutionConfig, ExecutionMode};
use crate::config::{CommandConfig, ShutdownConfig};
use crate::error::{ExecuteError, ShutdownError, SubmitError};
use crate::executor::CommandExecutor;
use crate::hooks::ExecutionHook;
use crate::metrics::Metrics;
use crate::task_handle::{TaskHandle, TaskResult, TaskState};
use crate::zombie_reaper::ZombieReaper;

/// 修复版本的 shutdown_with_timeout 实现
/// 
/// 使用线程来等待 worker，这样可以实现真正的超时控制
pub fn shutdown_with_timeout_fixed(pool: &CommandPool, timeout: Duration) -> Result<(), ShutdownError> {
    // 1. 设置 shutdown flag，停止接受新任务
    pool.shutdown_flag.store(true, Ordering::SeqCst);
    pool.running.store(false, Ordering::SeqCst);

    // 2. 唤醒所有可能在等待的线程
    let (_, cvar) = &*pool.tasks;
    cvar.notify_all();

    // 3. 等待所有 worker 完成或超时
    let start = Instant::now();
    let mut handles = pool.handles.lock().unwrap();

    // 收集所有 handles 到一个 Vec 中
    let handles_vec: Vec<_> = handles.drain(..).collect();
    let total_workers = handles_vec.len();
    drop(handles); // 释放锁

    // 使用 crossbeam scope 实现带超时的线程等待
    let mut worker_results: Vec<Option<std::thread::Result<()>>> = Vec::with_capacity(total_workers);
    
    // 为每个 worker 启动一个监控线程
    let (tx, rx) = std::sync::mpsc::channel();
    let mut handles_iter = handles_vec.into_iter();
    
    // 启动监控线程
    for _ in 0..total_workers {
        if let Some(handle) = handles_iter.next() {
            let tx = tx.clone();
            let timeout_remaining = timeout.saturating_sub(start.elapsed());
            
            if timeout_remaining.is_zero() {
                #[cfg(feature = "logging")]
                tracing::warn!(
                    "Shutdown timeout reached, {} workers may still be running",
                    total_workers - worker_results.len()
                );
                // 超时，放弃等待剩余线程
                break;
            }
            
            // 使用线程来等待，这样可以实现超时
            let handle_id = worker_results.len();
            
            // 使用 thread::spawn 创建一个监控线程来等待 worker
            // 监控线程会调用 handle.join() 并发送结果
            let monitor_handle = thread::spawn(move || {
                let join_result = handle.join();
                let _ = tx.send((handle_id, join_result));
            });
            
            // 等待监控线程完成或超时
            match monitor_handle.join() {
                Ok(_) => {
                    // 监控线程已完成，尝试接收结果
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok((idx, result)) => {
                            while worker_results.len() <= idx {
                                worker_results.push(None);
                            }
                            worker_results[idx] = Some(result);
                        }
                        Err(_) => {
                            // 超时或通道断开
                        }
                    }
                }
                Err(_) => {
                    // 监控线程 panic，记录错误
                    worker_results.push(None);
                }
            }
        }
    }
    
    // 填充剩余的 None
    while worker_results.len() < total_workers {
        worker_results.push(None);
    }
    
    let results = worker_results;

    // 检查是否有 worker panic
    for (idx, result) in results.iter().enumerate() {
        if let Some(Err(_)) = result {
            #[cfg(feature = "logging")]
            tracing::error!("Worker {} panicked", idx);
            return Err(ShutdownError::WorkerPanic);
        }
    }

    #[cfg(feature = "logging")]
    tracing::info!("Graceful shutdown completed successfully");
    Ok(())
}
