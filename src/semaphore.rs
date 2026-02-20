use std::sync::{Arc, Condvar, Mutex};

/// 简单的计数信号量 | Simple counting semaphore
///
/// 基于 `Mutex` 和 `Condvar` 实现，用于轻量级的并发执行控制。
/// 限制同时执行的外部子进程数量，防止系统资源耗尽。
pub struct Semaphore {
    /// 内部状态：当前可用许可证数和条件变量
    ///
    /// - Mutex<usize>: 当前可用的许可证数量
    /// - Condvar: 用于阻塞等待许可证的条件变量
    inner: Arc<(Mutex<usize>, Condvar)>,
}

/// RAII 信号量守卫，在 Drop 时自动释放许可证 | RAII semaphore guard that releases permit on drop.
pub struct SemaphoreGuard {
    /// 内部状态的克隆，用于在 Drop 时释放许可证
    inner: Arc<(Mutex<usize>, Condvar)>,
}

impl Semaphore {
    /// 创建一个信号量，初始许可证数为 `permits` | Create a semaphore with initial permits
    pub fn new(permits: usize) -> Self {
        Self {
            inner: Arc::new((Mutex::new(permits), Condvar::new())),
        }
    }

    /// 获取一个许可证，若许可证数为 0 则阻塞等待 | Acquire a permit, blocking if none available
    pub fn acquire(&self) {
        let (lock, cvar) = &*self.inner;
        let mut cnt = lock.lock().unwrap_or_else(|e| e.into_inner());
        // 自旋等待直到有可用许可证 | Spin-wait until a permit is available
        while *cnt == 0 {
            cnt = cvar.wait(cnt).unwrap_or_else(|e| e.into_inner());
        }
        *cnt -= 1;
    }

    /// 获取一个 RAII 守卫，在生命周期结束时自动释放许可证。
    pub fn acquire_guard(&self) -> SemaphoreGuard {
        // 复用 acquire 的阻塞获取逻辑
        self.acquire();
        SemaphoreGuard {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Drop for SemaphoreGuard {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.inner;
        let mut cnt = lock.lock().unwrap_or_else(|e| e.into_inner());
        *cnt += 1;
        cvar.notify_one();
    }
}
