use std::time::Duration;
use thiserror::Error;

/// ExecuteError 表示在启动或等待子进程过程中可能遇到的错误。
///
/// 常见变体包括 IO 错误、超时错误以及子进程状态异常等。
#[derive(Error, Debug)]
pub enum ExecuteError {
    /// IO 错误
    ///
    /// 当系统调用失败时返回，如进程创建失败、管道创建失败等。
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// 命令执行超时
    ///
    /// 当命令执行时间超过设定的超时时间时返回。
    /// 包含实际的超时时长。
    #[error("command execution timed out after {0:?}")]
    Timeout(Duration),

    /// 子进程错误
    ///
    /// 当子进程返回非零退出码或其他异常状态时返回。
    /// 包含错误描述信息。
    #[error("child process error: {0}")]
    Child(String),
}
