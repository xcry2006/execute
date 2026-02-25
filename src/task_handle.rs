use std::process::Output;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use crate::error::ExecuteError;

/// 任务结果
pub type TaskResult = Result<Output, ExecuteError>;

/// 取消令牌
///
/// 用于取消任务执行的令牌。可以在多个线程间共享，
/// 当调用 cancel() 时，所有持有该令牌的任务都会收到取消信号。
#[derive(Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// 创建新的取消令牌
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 取消任务
    ///
    /// 设置取消标志，所有持有该令牌的任务都会收到取消信号。
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// 检查是否已取消
    ///
    /// # 返回
    /// - `true`：任务已被取消
    /// - `false`：任务未被取消
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// 任务状态
///
/// 表示任务在其生命周期中的不同状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    /// 任务在队列中等待执行
    Queued,
    /// 任务正在执行
    ///
    /// 包含可选的进程 ID（如果任务已启动子进程）
    Running { pid: Option<u32> },
    /// 任务已完成
    Completed,
    /// 任务已被取消
    Cancelled,
}

/// 任务句柄
///
/// 用于获取异步执行的任务结果和控制任务执行。任务提交后返回此句柄，
/// 可以通过它等待任务完成、获取执行结果或取消任务。
///
/// # 示例
///
/// ```ignore
/// use execute::TaskHandle;
///
/// // 提交任务并获取句柄
/// let handle = pool.submit(command)?;
///
/// // 取消任务
/// handle.cancel()?;
///
/// // 或等待任务完成
/// let result = handle.wait()?;
/// ```
pub struct TaskHandle {
    /// 任务 ID
    task_id: u64,
    /// 取消令牌
    cancel_token: CancellationToken,
    /// 任务状态
    state: Arc<Mutex<TaskState>>,
    /// 结果接收器
    receiver: Arc<Mutex<Receiver<TaskResult>>>,
}

impl TaskHandle {
    /// 创建新的任务句柄
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务 ID
    ///
    /// # 返回
    ///
    /// 返回一个元组，包含任务句柄和结果发送器
    pub fn new(task_id: u64) -> (Self, Sender<TaskResult>) {
        let (sender, receiver) = channel();
        let cancel_token = CancellationToken::new();
        let state = Arc::new(Mutex::new(TaskState::Queued));

        (
            Self {
                task_id,
                cancel_token,
                state,
                receiver: Arc::new(Mutex::new(receiver)),
            },
            sender,
        )
    }

    /// 创建带有指定取消令牌和状态的任务句柄
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务 ID
    /// * `cancel_token` - 取消令牌
    /// * `state` - 任务状态
    ///
    /// # 返回
    ///
    /// 返回一个元组，包含任务句柄和结果发送器
    pub fn with_cancel_token(
        task_id: u64,
        cancel_token: CancellationToken,
        state: Arc<Mutex<TaskState>>,
    ) -> (Self, Sender<TaskResult>) {
        let (sender, receiver) = channel();

        (
            Self {
                task_id,
                cancel_token,
                state,
                receiver: Arc::new(Mutex::new(receiver)),
            },
            sender,
        )
    }

    /// 获取任务 ID
    pub fn id(&self) -> u64 {
        self.task_id
    }

    /// 获取取消令牌的引用
    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel_token
    }

    /// 获取任务状态
    ///
    /// # 返回
    ///
    /// 返回当前任务状态的副本
    pub fn state(&self) -> TaskState {
        self.state.lock().unwrap().clone()
    }

    /// 更新任务状态
    ///
    /// # 参数
    ///
    /// * `new_state` - 新的任务状态
    pub fn set_state(&self, new_state: TaskState) {
        let mut state = self.state.lock().unwrap();
        *state = new_state;
    }

    /// 检查任务是否已取消
    ///
    /// # 返回
    /// - `true`：任务已被取消
    /// - `false`：任务未被取消
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// 取消任务
    ///
    /// 根据任务当前状态执行不同的取消操作：
    /// - 如果任务在队列中：设置取消标志，任务将在执行前被跳过
    /// - 如果任务正在执行：终止执行进程
    /// - 如果任务已完成：返回错误
    /// - 如果任务已取消：返回错误
    ///
    /// # 返回
    ///
    /// * `Ok(())` - 取消成功
    /// * `Err(CancelError)` - 取消失败
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::TaskHandle;
    ///
    /// let handle = pool.submit(command)?;
    /// handle.cancel()?;
    /// ```
    pub fn cancel(&self) -> Result<(), crate::error::CancelError> {
        use crate::error::CancelError;

        let mut state = self.state.lock().unwrap();

        match *state {
            TaskState::Queued => {
                // 任务在队列中，设置取消标志
                // 执行器会在执行前检查取消标志并跳过任务
                self.cancel_token.cancel();
                *state = TaskState::Cancelled;

                #[cfg(feature = "logging")]
                tracing::info!(task_id = self.task_id, "Task cancelled while queued");

                Ok(())
            }
            TaskState::Running { pid: Some(pid) } => {
                // 任务正在执行，终止进程
                #[cfg(feature = "logging")]
                tracing::warn!(
                    task_id = self.task_id,
                    pid = pid,
                    "Terminating running task"
                );

                #[cfg(unix)]
                {
                    use nix::sys::signal::{Signal, kill};
                    use nix::unistd::Pid;

                    // 尝试使用 SIGTERM 优雅终止
                    match kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                        Ok(_) => {
                            // 等待一小段时间让进程优雅退出
                            std::thread::sleep(std::time::Duration::from_millis(100));

                            // 检查进程是否还在运行
                            if kill(Pid::from_raw(pid as i32), Signal::SIGCONT).is_ok() {
                                // 进程还在运行，使用 SIGKILL 强制终止
                                #[cfg(feature = "logging")]
                                tracing::warn!(
                                    task_id = self.task_id,
                                    pid = pid,
                                    "Process did not respond to SIGTERM, sending SIGKILL"
                                );
                                kill(Pid::from_raw(pid as i32), Signal::SIGKILL)
                                    .map_err(|e| CancelError::KillFailed(e.to_string()))?;
                            }
                        }
                        Err(e) => {
                            // SIGTERM 失败，尝试 SIGKILL
                            #[cfg(feature = "logging")]
                            tracing::warn!(
                                task_id = self.task_id,
                                pid = pid,
                                error = %e,
                                "SIGTERM failed, trying SIGKILL"
                            );
                            kill(Pid::from_raw(pid as i32), Signal::SIGKILL)
                                .map_err(|e| CancelError::KillFailed(e.to_string()))?;
                        }
                    }
                }

                #[cfg(not(unix))]
                {
                    // 在非 Unix 平台上，我们无法直接终止进程
                    // 这里只设置取消标志，让执行器处理
                    #[cfg(feature = "logging")]
                    tracing::warn!(
                        task_id = self.task_id,
                        "Process termination not supported on this platform"
                    );
                }

                // 设置取消标志和状态
                self.cancel_token.cancel();
                *state = TaskState::Cancelled;

                #[cfg(feature = "logging")]
                tracing::info!(task_id = self.task_id, "Task cancelled while running");

                Ok(())
            }
            TaskState::Running { pid: None } => {
                // 任务正在执行但没有 PID（可能还在启动中）
                // 设置取消标志，让执行器处理
                self.cancel_token.cancel();
                *state = TaskState::Cancelled;

                #[cfg(feature = "logging")]
                tracing::info!(task_id = self.task_id, "Task cancelled while starting");

                Ok(())
            }
            TaskState::Completed => {
                // 任务已完成，无法取消
                Err(CancelError::AlreadyCompleted)
            }
            TaskState::Cancelled => {
                // 任务已取消
                Err(CancelError::AlreadyCancelled)
            }
        }
    }

    /// 等待并获取任务结果（阻塞）
    ///
    /// # 返回
    /// - `Ok(Output)`：任务成功执行
    /// - `Err(ExecuteError)`：任务执行失败或结果已被获取
    pub fn wait(&self) -> TaskResult {
        let receiver = self.receiver.lock().unwrap();
        receiver
            .recv()
            .map_err(|_| ExecuteError::Io(std::io::Error::other("failed to receive task result")))?
    }

    /// 尝试获取任务结果（非阻塞）
    ///
    /// # 返回
    /// - `Ok(Some(Output))`：任务已完成且成功
    /// - `Ok(None)`：任务尚未完成
    /// - `Err(ExecuteError)`：任务执行失败
    pub fn try_get(&self) -> Result<Option<Output>, ExecuteError> {
        let receiver = self.receiver.lock().unwrap();
        match receiver.try_recv() {
            Ok(result) => result.map(Some),
            Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(ExecuteError::Io(
                std::io::Error::other("task result channel disconnected"),
            )),
        }
    }

    /// 检查任务是否已完成（非阻塞）
    ///
    /// # 返回
    /// - `Ok(true)`：任务已完成
    /// - `Ok(false)`：任务尚未完成
    /// - `Err(ExecuteError)`：通道已断开
    pub fn is_done(&self) -> Result<bool, ExecuteError> {
        match self.try_get() {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }
}

impl Clone for TaskHandle {
    fn clone(&self) -> Self {
        Self {
            task_id: self.task_id,
            cancel_token: self.cancel_token.clone(),
            state: Arc::clone(&self.state),
            receiver: Arc::clone(&self.receiver),
        }
    }
}

/// 带结果通道的任务
///
/// 内部使用，将任务配置与结果发送器绑定
pub struct TaskWithResult {
    /// 任务 ID
    pub id: u64,
    /// 结果发送器
    pub result_sender: Sender<TaskResult>,
}

impl TaskWithResult {
    /// 创建新的带结果任务
    pub fn new(id: u64) -> (Self, TaskHandle) {
        let (handle, sender) = TaskHandle::new(id);
        (
            Self {
                id,
                result_sender: sender,
            },
            handle,
        )
    }

    /// 发送任务结果
    #[allow(clippy::result_unit_err)]
    pub fn send_result(&self, result: TaskResult) -> Result<(), ()> {
        self.result_sender.send(result).map_err(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Output;
    use std::thread;

    #[test]
    fn cancellation_token_new_is_not_cancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancellation_token_cancel_sets_flag() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn cancellation_token_clone_shares_state() {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        token.cancel();
        assert!(token_clone.is_cancelled());
    }

    #[test]
    fn task_state_equality() {
        assert_eq!(TaskState::Queued, TaskState::Queued);
        assert_eq!(
            TaskState::Running { pid: Some(123) },
            TaskState::Running { pid: Some(123) }
        );
        assert_eq!(TaskState::Completed, TaskState::Completed);
        assert_eq!(TaskState::Cancelled, TaskState::Cancelled);

        assert_ne!(TaskState::Queued, TaskState::Completed);
        assert_ne!(
            TaskState::Running { pid: Some(123) },
            TaskState::Running { pid: Some(456) }
        );
    }

    #[test]
    fn task_handle_new_creates_queued_state() {
        let (handle, _sender) = TaskHandle::new(1);
        assert_eq!(handle.id(), 1);
        assert_eq!(handle.state(), TaskState::Queued);
        assert!(!handle.is_cancelled());
    }

    #[test]
    fn task_handle_set_state_updates_state() {
        let (handle, _sender) = TaskHandle::new(1);

        handle.set_state(TaskState::Running { pid: Some(123) });
        assert_eq!(handle.state(), TaskState::Running { pid: Some(123) });

        handle.set_state(TaskState::Completed);
        assert_eq!(handle.state(), TaskState::Completed);
    }

    #[test]
    fn task_handle_cancel_token_is_accessible() {
        let (handle, _sender) = TaskHandle::new(1);

        assert!(!handle.cancel_token().is_cancelled());
        handle.cancel_token().cancel();
        assert!(handle.is_cancelled());
    }

    #[test]
    fn task_handle_wait_receives_result() {
        let (handle, sender) = TaskHandle::new(1);

        // 模拟异步任务完成
        thread::spawn(move || {
            let output = Output {
                status: std::process::ExitStatus::default(),
                stdout: b"hello".to_vec(),
                stderr: vec![],
            };
            let _ = sender.send(Ok(output));
        });

        let result = handle.wait();
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.stdout, b"hello");
    }

    #[test]
    fn task_handle_try_get_returns_none_when_pending() {
        let (handle, _sender) = TaskHandle::new(1);

        let result = handle.try_get();
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn task_handle_try_get_returns_result_when_done() {
        let (handle, sender) = TaskHandle::new(1);

        let output = Output {
            status: std::process::ExitStatus::default(),
            stdout: b"world".to_vec(),
            stderr: vec![],
        };
        let _ = sender.send(Ok(output));

        let result = handle.try_get();
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn task_handle_is_done_returns_correct_status() {
        let (handle, sender) = TaskHandle::new(1);

        assert!(!handle.is_done().unwrap());

        let output = Output {
            status: std::process::ExitStatus::default(),
            stdout: vec![],
            stderr: vec![],
        };
        let _ = sender.send(Ok(output));

        assert!(handle.is_done().unwrap());
    }

    #[test]
    fn task_handle_id_returns_correct_id() {
        let (handle, _sender) = TaskHandle::new(42);
        assert_eq!(handle.id(), 42);
    }

    #[test]
    fn task_handle_with_cancel_token_uses_provided_token() {
        let cancel_token = CancellationToken::new();
        let state = Arc::new(Mutex::new(TaskState::Queued));

        let (handle, _sender) = TaskHandle::with_cancel_token(1, cancel_token.clone(), state);

        cancel_token.cancel();
        assert!(handle.is_cancelled());
    }

    #[test]
    fn task_handle_clone_shares_state() {
        let (handle, _sender) = TaskHandle::new(1);
        let handle_clone = handle.clone();

        handle.set_state(TaskState::Running { pid: Some(123) });
        assert_eq!(handle_clone.state(), TaskState::Running { pid: Some(123) });

        handle.cancel_token().cancel();
        assert!(handle_clone.is_cancelled());
    }

    #[test]
    fn task_with_result_sends_result() {
        let (task, handle) = TaskWithResult::new(1);

        let output = Output {
            status: std::process::ExitStatus::default(),
            stdout: b"test".to_vec(),
            stderr: vec![],
        };

        task.send_result(Ok(output)).unwrap();

        let result = handle.wait();
        assert!(result.is_ok());
    }

    #[test]
    fn cancel_queued_task_succeeds() {
        let (handle, _sender) = TaskHandle::new(1);

        // 任务初始状态为 Queued
        assert_eq!(handle.state(), TaskState::Queued);
        assert!(!handle.is_cancelled());

        // 取消任务
        let result = handle.cancel();
        assert!(result.is_ok());

        // 验证状态已更新
        assert_eq!(handle.state(), TaskState::Cancelled);
        assert!(handle.is_cancelled());
    }

    #[test]
    fn cancel_running_task_without_pid_succeeds() {
        let (handle, _sender) = TaskHandle::new(1);

        // 设置任务为运行状态（无 PID）
        handle.set_state(TaskState::Running { pid: None });

        // 取消任务
        let result = handle.cancel();
        assert!(result.is_ok());

        // 验证状态已更新
        assert_eq!(handle.state(), TaskState::Cancelled);
        assert!(handle.is_cancelled());
    }

    #[test]
    fn cancel_completed_task_fails() {
        use crate::error::CancelError;

        let (handle, _sender) = TaskHandle::new(1);

        // 设置任务为已完成状态
        handle.set_state(TaskState::Completed);

        // 尝试取消任务
        let result = handle.cancel();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CancelError::AlreadyCompleted);
    }

    #[test]
    fn cancel_already_cancelled_task_fails() {
        use crate::error::CancelError;

        let (handle, _sender) = TaskHandle::new(1);

        // 第一次取消
        handle.cancel().unwrap();

        // 第二次取消应该失败
        let result = handle.cancel();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), CancelError::AlreadyCancelled);
    }

    #[test]
    #[cfg(unix)]
    fn cancel_running_task_with_pid_attempts_kill() {
        use std::process::Command;

        // 启动一个长时间运行的进程
        let mut child = Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("Failed to spawn sleep process");

        let pid = child.id();

        let (handle, _sender) = TaskHandle::new(1);

        // 设置任务为运行状态（有 PID）
        handle.set_state(TaskState::Running { pid: Some(pid) });

        // 取消任务
        let result = handle.cancel();
        assert!(result.is_ok());

        // 验证状态已更新
        assert_eq!(handle.state(), TaskState::Cancelled);
        assert!(handle.is_cancelled());

        // 验证进程已被终止
        std::thread::sleep(std::time::Duration::from_millis(200));
        // Try to wait for the child, but don't fail if it's already been reaped
        // by the zombie reaper or the OS
        let _ = child.try_wait();
    }
}
