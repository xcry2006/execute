use std::process::{Command, Output, Stdio};

use crate::{CommandConfig, ExecuteError};

/// 命令执行器 trait | Command executor trait
///
/// 抽象命令执行的接口，支持不同的运行时实现（std::process、tokio、async-std 等）。
/// Defines the interface for command execution, supporting different runtime implementations.
pub trait CommandExecutor: Send + Sync {
    /// 执行命令并返回输出 | Execute a command and return output
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError>;
}

/// 标准库命令执行器 | Standard library command executor
///
/// 使用 std::process::Command 实现，基于线程同步的同步执行。
/// Implementation using std::process::Command with synchronous thread-based execution.
pub struct StdCommandExecutor;

impl CommandExecutor for StdCommandExecutor {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        execute_command(config)
    }
}

/// 执行单个命令配置 | Execute a single command configuration
///
/// 内部函数，用于启动子进程并处理超时。使用 wait-timeout crate 在同一线程中进行超时等待，
/// 避免为每个任务生成额外的等待线程，提高性能和降低系统开销。
pub(crate) fn execute_command(config: &CommandConfig) -> Result<Output, ExecuteError> {
    // 启动子进程，重定向 stdout 和 stderr | Spawn child with piped stdout/stderr
    let mut cmd = Command::new(&config.program);
    cmd.args(&config.args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = &config.working_dir {
        cmd.current_dir(dir);
    }

    let mut child = cmd.spawn()?;

    // 根据是否设置超时进行等待处理 | Handle waiting based on timeout configuration
    match config.timeout {
        Some(timeout) => {
            // 使用 wait-timeout 在当前线程中等待，不产生额外线程
            // Use wait-timeout for in-thread waiting without spawning extra threads
            use wait_timeout::ChildExt;
            match child
                .wait_timeout(timeout)
                .map_err(|e| ExecuteError::Io(std::io::Error::other(e)))?
            {
                Some(_) => {
                    // 子进程在超时前正常退出，收集输出 | Child exited within timeout; collect output
                    let output = child.wait_with_output()?;
                    Ok(output)
                }
                None => {
                    // 超时：尝试杀死子进程 | Timeout: attempt to kill the child process
                    let _ = child.kill();
                    let _ = child.wait();
                    Err(ExecuteError::Timeout(timeout))
                }
            }
        }
        None => {
            // 无超时限制，直接等待子进程完成 | No timeout: wait and collect without limit
            let output = child.wait_with_output()?;
            Ok(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    #[cfg(unix)]
    fn execute_command_true_succeeds() {
        let cfg = CommandConfig::new("true", vec![]);
        let output = execute_command(&cfg).expect("command should succeed");
        assert!(output.status.success());
    }

    #[test]
    #[cfg(unix)]
    fn execute_command_times_out() {
        let cfg = CommandConfig::new("sleep", vec!["1".to_string()])
            .with_timeout(Duration::from_millis(100));

        let err = execute_command(&cfg).expect_err("command should time out");
        match err {
            ExecuteError::Timeout(dur) => {
                assert_eq!(dur, Duration::from_millis(100));
            }
            other => panic!("expected Timeout error, got {other:?}"),
        }
    }
}
