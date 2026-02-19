use std::env;
use std::io::{self, BufRead, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use execute::{CommandConfig, CommandPool};
use wait_timeout::ChildExt;

/// # 程序入口
///
/// 启动一个 `CommandPool` 并启动后台执行器，然后在另一个线程中向池中推入示例任务：
/// 1. 一个短命令 `echo`；
/// 2. 带工作目录和超时配置的 `echo`；
/// 3. 一个可能超时的 `sleep`（用于演示超时处理）。
///
/// # 返回
/// - `Ok(())`：主流程正常结束。
/// - `Err(ExecuteError)`：若在主流程中遇到不可恢复的错误则返回。
fn main() -> Result<(), execute::ExecuteError> {
    // 检查是否是 worker 模式
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "--worker" {
        return run_worker_mode();
    }

    println!("[main] 启动命令池并启动执行器...");
    let command_pool = CommandPool::new();
    command_pool.start_executor(Duration::from_millis(500));

    let pool_clone = command_pool.clone();
    thread::spawn(move || {
        println!("[producer] 向池中推入第一个任务");
        let task1 = CommandConfig::new("echo", vec!["第一次任务执行".to_string()]);
        pool_clone.push_task(task1);

        thread::sleep(Duration::from_secs(2));

        println!("[producer] 向池中推入第二个任务");
        let task2 = CommandConfig::new("echo", vec!["第二次任务执行".to_string()])
            .with_working_dir(".")
            .with_timeout(Duration::from_secs(5));
        pool_clone.push_task(task2);

        thread::sleep(Duration::from_secs(2));

        println!("[producer] 向池中推入第三个任务（会超时）");
        let task3 = CommandConfig::new("sleep", vec!["20".to_string()])
            .with_timeout(Duration::from_secs(3));
        pool_clone.push_task(task3);
    });

    println!("[main] 等待所有任务执行完毕...");
    thread::sleep(Duration::from_secs(15));
    println!("[main] 程序结束");
    Ok(())
}

/// Worker 模式 - 作为进程池的工作进程运行
///
/// 从 stdin 读取命令，执行后返回结果到 stdout
fn run_worker_mode() -> Result<(), execute::ExecuteError> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| execute::ExecuteError::Io(e))?;
        if line.is_empty() {
            continue;
        }

        // 解析命令
        // 格式: program\targ1\targ2\t...\tworking_dir\ttimeout
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.is_empty() {
            continue;
        }

        let program = parts[0];
        let args: Vec<String> = if parts.len() > 1 {
            parts[1..parts.len().saturating_sub(2)]
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            vec![]
        };

        let working_dir = if parts.len() > 2 && !parts[parts.len() - 2].is_empty() {
            Some(parts[parts.len() - 2].to_string())
        } else {
            None
        };

        let timeout_secs = if parts.len() > 1 {
            parts.last().unwrap_or(&"0").parse::<u64>().unwrap_or(0)
        } else {
            0
        };

        // 构建命令
        let mut cmd = Command::new(program);
        cmd.args(&args);

        if let Some(ref dir) = working_dir {
            cmd.current_dir(dir);
        }

        // 执行命令
        let output = if timeout_secs > 0 {
            cmd.stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    let timeout = Duration::from_secs(timeout_secs);
                    match child.wait_timeout(timeout) {
                        Ok(Some(status)) => {
                            let mut out = Vec::new();
                            let mut err = Vec::new();
                            if let Some(mut stdout) = child.stdout.take() {
                                use std::io::Read;
                                let _ = stdout.read_to_end(&mut out);
                            }
                            if let Some(mut stderr) = child.stderr.take() {
                                use std::io::Read;
                                let _ = stderr.read_to_end(&mut err);
                            }
                            Ok(std::process::Output {
                                status,
                                stdout: out,
                                stderr: err,
                            })
                        }
                        Ok(None) => {
                            let _ = child.kill();
                            Err(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "command timed out",
                            ))
                        }
                        Err(e) => Err(e),
                    }
                })
        } else {
            cmd.output()
        };

        // 发送结果
        match output {
            Ok(out) => {
                let exit_code = out.status.code().unwrap_or(-1);
                let stdout_str = String::from_utf8_lossy(&out.stdout);
                let stderr_str = String::from_utf8_lossy(&out.stderr);
                let response = format!(
                    "{}\t{}\t{}\t{}\t{}\n",
                    exit_code,
                    out.stdout.len(),
                    stdout_str,
                    out.stderr.len(),
                    stderr_str
                );
                let _ = stdout.write_all(response.as_bytes());
                let _ = stdout.flush();
            }
            Err(e) => {
                let response = format!("-1\t0\t\t0\t{}\n", e);
                let _ = stdout.write_all(response.as_bytes());
                let _ = stdout.flush();
            }
        }
    }

    Ok(())
}
