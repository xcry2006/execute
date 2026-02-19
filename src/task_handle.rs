use std::process::Output;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use crate::error::ExecuteError;

/// 任务结果
pub type TaskResult = Result<Output, ExecuteError>;

/// 任务句柄
///
/// 用于获取异步执行的任务结果。任务提交后返回此句柄，
/// 可以通过它等待任务完成并获取执行结果。
pub struct TaskHandle {
    /// 任务 ID
    id: u64,
    /// 结果接收器
    receiver: Arc<Mutex<Receiver<TaskResult>>>,
}

impl TaskHandle {
    /// 创建新的任务句柄
    pub fn new(id: u64) -> (Self, Sender<TaskResult>) {
        let (sender, receiver) = channel();
        (
            Self {
                id,
                receiver: Arc::new(Mutex::new(receiver)),
            },
            sender,
        )
    }

    /// 获取任务 ID
    pub fn id(&self) -> u64 {
        self.id
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
            id: self.id,
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
}
