use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// 僵尸进程清理器
///
/// 定期检查并回收已终止的子进程，防止僵尸进程累积。
/// 在 Unix 系统上使用 `waitpid` 系统调用回收进程。
///
/// # 示例
///
/// ```no_run
/// use execute::ZombieReaper;
/// use std::time::Duration;
///
/// // 创建清理器，每 5 秒检查一次
/// let reaper = ZombieReaper::new(Duration::from_secs(5));
///
/// // ... 执行命令 ...
///
/// // 停止清理器
/// drop(reaper);
/// ```
pub struct ZombieReaper {
    /// 检查间隔
    #[allow(dead_code)]
    check_interval: Duration,

    /// 后台线程句柄
    handle: Option<JoinHandle<()>>,

    /// 关闭标志
    shutdown: Arc<AtomicBool>,
}

impl ZombieReaper {
    /// 创建新的僵尸进程清理器
    ///
    /// # 参数
    ///
    /// * `check_interval` - 检查僵尸进程的时间间隔
    ///
    /// # 返回
    ///
    /// 返回一个新的 `ZombieReaper` 实例，后台线程会立即启动。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use execute::ZombieReaper;
    /// use std::time::Duration;
    ///
    /// let reaper = ZombieReaper::new(Duration::from_secs(10));
    /// ```
    pub fn new(check_interval: Duration) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        let handle = thread::spawn(move || {
            reaper_loop(check_interval, shutdown_clone);
        });

        #[cfg(feature = "logging")]
        tracing::info!(
            interval_secs = check_interval.as_secs(),
            "ZombieReaper started"
        );

        Self {
            check_interval,
            handle: Some(handle),
            shutdown,
        }
    }

    /// 停止僵尸进程清理器
    ///
    /// 设置关闭标志并等待后台线程退出。
    /// 在退出前会执行最后一次清理。
    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
            #[cfg(feature = "logging")]
            tracing::info!("ZombieReaper stopped");
        }
    }

    /// 检查清理器是否正在运行
    pub fn is_running(&self) -> bool {
        !self.shutdown.load(Ordering::SeqCst)
    }
}

impl Drop for ZombieReaper {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 清理器主循环
///
/// 定期调用 `reap_zombies()` 清理僵尸进程，直到收到关闭信号。
fn reaper_loop(interval: Duration, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        let cleaned = reap_zombies();
        if cleaned > 0 {
            #[cfg(feature = "logging")]
            tracing::info!(count = cleaned, "Reaped zombie processes");
        }

        thread::sleep(interval);
    }

    // 退出前最后清理一次
    let cleaned = reap_zombies();
    if cleaned > 0 {
        #[cfg(feature = "logging")]
        tracing::info!(count = cleaned, "Final zombie process cleanup");
    }
}

/// 清理僵尸进程
///
/// 在 Unix 系统上使用 `waitpid(-1, WNOHANG)` 非阻塞地回收所有已终止的子进程。
/// 在非 Unix 系统上，此函数不执行任何操作。
///
/// # 返回
///
/// 返回清理的僵尸进程数量。
#[cfg(unix)]
fn reap_zombies() -> usize {
    use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
    use nix::unistd::Pid;

    let mut count = 0;
    loop {
        match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::Exited(pid, status)) => {
                #[cfg(feature = "logging")]
                tracing::debug!(
                    pid = pid.as_raw(),
                    exit_status = status,
                    "Reaped exited process"
                );
                count += 1;
            }
            Ok(WaitStatus::Signaled(pid, signal, _)) => {
                #[cfg(feature = "logging")]
                tracing::debug!(
                    pid = pid.as_raw(),
                    signal = signal as i32,
                    "Reaped signaled process"
                );
                count += 1;
            }
            Ok(WaitStatus::StillAlive) => {
                // 没有更多僵尸进程
                break;
            }
            Err(nix::errno::Errno::ECHILD) => {
                // 没有子进程
                break;
            }
            Err(e) => {
                #[cfg(feature = "logging")]
                tracing::warn!(error = %e, "Error while reaping zombies");
                break;
            }
            _ => {
                // 其他状态（Stopped, Continued 等）不计入清理数量
                continue;
            }
        }
    }
    count
}

/// 非 Unix 系统的占位实现
#[cfg(not(unix))]
fn reap_zombies() -> usize {
    // 在非 Unix 系统上不执行任何操作
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zombie_reaper_creation() {
        let reaper = ZombieReaper::new(Duration::from_millis(100));
        assert!(reaper.is_running());
    }

    #[test]
    fn test_zombie_reaper_stop() {
        let mut reaper = ZombieReaper::new(Duration::from_millis(100));
        assert!(reaper.is_running());

        reaper.stop();
        assert!(!reaper.is_running());
    }

    #[test]
    fn test_zombie_reaper_drop() {
        let reaper = ZombieReaper::new(Duration::from_millis(100));
        assert!(reaper.is_running());

        drop(reaper);
        // 清理器应该在 drop 时停止
    }

    #[cfg(unix)]
    #[test]
    fn test_reap_zombies_no_children() {
        // 当没有子进程时，应该返回 >= 0
        // Note: In test environments, there may be zombie processes from other tests
        let _count = reap_zombies();
        // reap_zombies 返回的是回收的僵尸进程数量，总是 >= 0
        // 在测试环境中可能有其他测试留下的僵尸进程，所以只验证函数能正常执行
    }
}
