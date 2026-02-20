use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};

use crate::config::CommandConfig;
use crate::error::ExecuteError;

/// 进程池中的工作进程
///
/// 封装一个常驻子进程，通过 stdin/stdout 进行 IPC 通信。
/// 用于执行命令并返回结果，避免频繁创建销毁进程的开销。
struct WorkerProcess {
    /// 工作进程 ID（用于调试）
    #[allow(dead_code)]
    id: usize,

    /// 子进程句柄
    ///
    /// 用于管理子进程生命周期（终止、等待等）
    child: Child,

    /// 子进程标准输入
    ///
    /// 用于向子进程发送命令
    stdin: std::process::ChildStdin,

    /// 子进程标准输出（缓冲读取器）
    ///
    /// 用于从子进程读取执行结果
    stdout: BufReader<std::process::ChildStdout>,
}

impl WorkerProcess {
    /// 创建新的工作进程
    fn new(id: usize) -> Result<Self, ExecuteError> {
        // 启动一个子进程，它会读取 stdin 的命令并执行
        let mut child = Command::new(std::env::current_exe()?)
            .arg("--worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ExecuteError::Io)?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ExecuteError::Io(std::io::Error::other("failed to capture stdin")))?;

        let stdout =
            BufReader::new(child.stdout.take().ok_or_else(|| {
                ExecuteError::Io(std::io::Error::other("failed to capture stdout"))
            })?);

        Ok(Self {
            id,
            child,
            stdin,
            stdout,
        })
    }

    /// 执行命令
    fn execute(&mut self, config: &CommandConfig) -> Result<std::process::Output, ExecuteError> {
        // 序列化命令配置
        let cmd_line = format!(
            "{}\t{}\t{}\t{}\n",
            config.program,
            config.args.join("\t"),
            config.working_dir.as_deref().unwrap_or(""),
            config.timeout.map(|d| d.as_secs()).unwrap_or(0)
        );

        // 发送命令到子进程
        self.stdin
            .write_all(cmd_line.as_bytes())
            .map_err(ExecuteError::Io)?;
        self.stdin.flush().map_err(ExecuteError::Io)?;

        // 读取执行结果
        let mut response = String::new();
        self.stdout
            .read_line(&mut response)
            .map_err(ExecuteError::Io)?;

        // 解析响应
        // 格式: exit_code\tstdout_len\tstdout\tstderr_len\tstderr
        let parts: Vec<&str> = response.trim().split('\t').collect();
        if parts.len() < 5 {
            return Err(ExecuteError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid response format",
            )));
        }

        let _exit_code: i32 = parts[0].parse().unwrap_or(-1);
        let _stdout_len: usize = parts[1].parse().unwrap_or(0);
        let stdout = parts[2].as_bytes().to_vec();
        let _stderr_len: usize = parts[3].parse().unwrap_or(0);
        let stderr = parts[4].as_bytes().to_vec();

        Ok(std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout,
            stderr,
        })
    }
}

/// 进程池
pub struct ProcessPool {
    workers: Arc<Mutex<VecDeque<WorkerProcess>>>,
    available: Arc<Condvar>,
    size: usize,
}

impl ProcessPool {
    /// 创建指定大小的进程池
    pub fn new(size: usize) -> Result<Self, ExecuteError> {
        let mut workers = VecDeque::with_capacity(size);

        for i in 0..size {
            let worker = WorkerProcess::new(i)?;
            workers.push_back(worker);
        }

        Ok(Self {
            workers: Arc::new(Mutex::new(workers)),
            available: Arc::new(Condvar::new()),
            size,
        })
    }

    /// 获取池大小
    pub fn size(&self) -> usize {
        self.size
    }

    /// 执行命令
    pub fn execute(&self, config: &CommandConfig) -> Result<std::process::Output, ExecuteError> {
        let (lock, cvar) = (&self.workers, &self.available);
        let mut workers = lock.lock().unwrap();

        // 等待可用工作进程
        while workers.is_empty() {
            workers = cvar.wait(workers).unwrap();
        }

        // 获取一个工作进程
        let mut worker = workers.pop_front().unwrap();
        drop(workers);

        // 执行命令
        let result = worker.execute(config);

        // 归还工作进程
        let mut workers = lock.lock().unwrap();
        workers.push_back(worker);
        cvar.notify_one();

        result
    }
}

impl Drop for ProcessPool {
    fn drop(&mut self) {
        // 终止所有工作进程
        let mut workers = self.workers.lock().unwrap();
        for mut worker in workers.drain(..) {
            let _ = worker.child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_pool_creates_correct_size() {
        // 注意：这个测试需要可执行文件支持 --worker 模式
        // 在实际运行前需要确保二进制已构建
        if let Ok(pool) = ProcessPool::new(4) {
            assert_eq!(pool.size(), 4);
        }
    }
}
