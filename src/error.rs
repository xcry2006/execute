use std::time::Duration;
use thiserror::Error;

/// ExecuteError 表示在启动或等待子进程过程中可能遇到的错误。
///
/// 常见变体包括 IO 错误、超时错误以及子进程状态异常等。
#[derive(Error, Debug)]
pub enum ExecuteError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("command execution timed out after {0:?}")]
    Timeout(Duration),

    #[error("child process error: {0}")]
    Child(String),
}

