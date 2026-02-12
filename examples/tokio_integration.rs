use execute::{CommandConfig, CommandExecutor, CommandPool, ExecuteError};
use std::process::Output;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::runtime::Runtime;
use tokio::time::timeout;

/// 示例：在 CommandPool 中使用 Tokio 异步执行器，并支持超时与错误处理。
struct TokioWithTimeoutExecutor {
    rt: Runtime,
}

impl TokioWithTimeoutExecutor {
    fn new() -> Result<Self, ExecuteError> {
        let rt = Runtime::new().map_err(|e| {
            ExecuteError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?;
        Ok(Self { rt })
    }
}

impl CommandExecutor for TokioWithTimeoutExecutor {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        self.rt.block_on(async {
            let mut cmd = Command::new(config.program());
            cmd.args(config.args());

            if let Some(dir) = config.working_dir() {
                cmd.current_dir(dir);
            }

            match config.timeout() {
                Some(dur) => {
                    timeout(dur, cmd.output())
                        .await
                        .map_err(|_| ExecuteError::Timeout(dur))?
                        .map_err(ExecuteError::Io)
                }
                None => cmd.output().await.map_err(ExecuteError::Io),
            }
        })
    }
}

fn main() -> Result<(), ExecuteError> {
    let pool = CommandPool::new();
    let executor = Arc::new(TokioWithTimeoutExecutor::new()?);

    // 添加几个示例任务
    pool.push_task(CommandConfig::new(
        "echo",
        vec!["hello from tokio".to_string()],
    ));
    pool.push_task(
        CommandConfig::new("sleep", vec!["1".to_string()])
            .with_timeout(Duration::from_millis(200)),
    );

    // 使用自定义 Tokio 执行器，4 个工作线程，最多 2 个并发执行外部命令
    pool.start_executor_with_executor_and_limit(
        Duration::from_millis(50),
        4,
        2,
        executor,
    );

    // 简单等待一段时间以便任务运行完成
    std::thread::sleep(Duration::from_secs(2));
    Ok(())
}

