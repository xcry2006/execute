#![cfg_attr(not(feature = "logging"), allow(dead_code))]

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::Instant;

use crate::error::{CommandError, ErrorContext};
use crate::hooks::{ExecutionContext, ExecutionHook, HookTaskResult};
use crate::{CommandConfig, ExecuteError};

/// 日志宏：在 logging feature 启用时使用 tracing，否则不记录
#[cfg(feature = "logging")]
macro_rules! log_warn {
    ($($arg:tt)*) => { ::tracing::warn!($($arg)*) };
}
#[cfg(not(feature = "logging"))]
macro_rules! log_warn {
    ($($arg:tt)*) => {};
}

#[cfg(feature = "logging")]
macro_rules! log_info {
    ($($arg:tt)*) => { ::tracing::info!($($arg)*) };
}
#[cfg(not(feature = "logging"))]
macro_rules! log_info {
    ($($arg:tt)*) => {};
}

#[cfg(feature = "logging")]
macro_rules! log_debug {
    ($($arg:tt)*) => { ::tracing::debug!($($arg)*) };
}
#[cfg(not(feature = "logging"))]
macro_rules! log_debug {
    ($($arg:tt)*) => {};
}

#[cfg(feature = "logging")]
macro_rules! log_error {
    ($($arg:tt)*) => { ::tracing::error!($($arg)*) };
}
#[cfg(not(feature = "logging"))]
macro_rules! log_error {
    ($($arg:tt)*) => {};
}

/// 限制读取大小的 Reader 包装器
///
/// 用于限制命令输出的大小，防止单个命令产生过大的输出。
/// 当读取的字节数超过限制时，会截断输出并记录警告。
struct LimitedReader<R> {
    inner: R,
    limit: Option<usize>,
    read: usize,
}

impl<R> LimitedReader<R> {
    /// 创建新的 LimitedReader
    ///
    /// # 参数
    ///
    /// * `inner` - 内部 Reader
    /// * `limit` - 最大读取字节数，None 表示无限制
    fn new(inner: R, limit: Option<usize>) -> Self {
        Self {
            inner,
            limit,
            read: 0,
        }
    }
}

impl<R: Read> Read for LimitedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(limit) = self.limit {
            if self.read >= limit {
                // 已达到限制，记录警告并返回 EOF
                log_warn!(
                    limit = limit,
                    "Output size limit reached, truncating output"
                );
                return Ok(0);
            }

            // 计算本次最多可以读取的字节数
            let max_read = std::cmp::min(buf.len(), limit - self.read);
            let n = self.inner.read(&mut buf[..max_read])?;
            self.read += n;
            Ok(n)
        } else {
            // 无限制，直接读取
            let n = self.inner.read(buf)?;
            self.read += n;
            Ok(n)
        }
    }
}

/// 获取进程的内存使用量（字节）
///
/// 在 Linux 上读取 /proc/[pid]/status 文件获取 VmRSS（常驻内存大小）。
/// 在其他平台上返回 None。
///
/// # 参数
///
/// * `pid` - 进程 ID
///
/// # 返回
///
/// 成功时返回内存使用量（字节），失败时返回 None
#[cfg(target_os = "linux")]
fn get_process_memory(pid: u32) -> Option<usize> {
    use std::fs;

    let status_path = format!("/proc/{}/status", pid);
    let content = fs::read_to_string(status_path).ok()?;

    // 查找 VmRSS 行，格式为 "VmRSS:    1234 kB"
    for line in content.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2
                && let Ok(kb) = parts[1].parse::<usize>()
            {
                // 转换为字节
                return Some(kb * 1024);
            }
        }
    }

    None
}

#[cfg(not(target_os = "linux"))]
fn get_process_memory(_pid: u32) -> Option<usize> {
    // 在非 Linux 平台上不支持内存监控
    None
}

/// 监控进程内存使用，超过限制时终止进程
///
/// 此函数会定期检查进程的内存使用，如果超过限制则终止进程。
/// 应该在单独的线程中运行。
///
/// # 参数
///
/// * `pid` - 进程 ID
/// * `max_memory` - 最大内存限制（字节）
/// * `check_interval` - 检查间隔
///
/// # 返回
///
/// 如果进程内存超过限制返回 true，否则返回 false
fn monitor_memory(pid: u32, max_memory: usize, check_interval: std::time::Duration) -> bool {
    loop {
        std::thread::sleep(check_interval);

        // 检查进程是否还在运行
        #[cfg(unix)]
        {
            use nix::sys::signal::{Signal, kill};
            use nix::unistd::Pid;

            // 发送信号 0 检查进程是否存在
            if kill(Pid::from_raw(pid as i32), Signal::SIGCONT).is_err() {
                // 进程已经不存在了
                return false;
            }
        }

        // 获取内存使用
        if let Some(memory) = get_process_memory(pid)
            && memory > max_memory
        {
            log_warn!(
                pid = pid,
                memory = memory,
                max_memory = max_memory,
                "Process memory limit exceeded, terminating process"
            );

            // 终止进程
            #[cfg(unix)]
            {
                use nix::sys::signal::{Signal, kill};
                use nix::unistd::Pid;

                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
            }

            return true;
        }
    }
}

/// 应用环境变量配置到命令
///
/// 根据 EnvConfig 配置设置命令的环境变量。
/// 支持继承父进程环境变量、设置新变量和清除特定变量。
///
/// # 参数
///
/// * `cmd` - 要配置的命令
/// * `env_config` - 环境变量配置
///
/// # 行为
///
/// 1. 如果 `inherit_parent` 为 false，清除所有继承的环境变量
/// 2. 遍历 `vars` 映射：
///    - `Some(value)`: 设置环境变量为指定值
///    - `None`: 清除该环境变量
///
/// # 示例
///
/// ```ignore
/// use std::process::Command;
/// use execute::config::EnvConfig;
/// use execute::executor::apply_env_config;
///
/// let mut cmd = Command::new("printenv");
/// let env = EnvConfig::new()
///     .set("MY_VAR", "my_value")
///     .remove("TEMP_VAR");
/// apply_env_config(&mut cmd, &env);
/// ```
pub fn apply_env_config(cmd: &mut Command, env_config: &crate::config::EnvConfig) {
    // 如果不继承父进程环境变量，清除所有环境变量
    if !env_config.inherit_parent() {
        cmd.env_clear();
    }

    // 应用配置的环境变量
    for (key, value) in env_config.vars() {
        match value {
            Some(v) => {
                // 设置环境变量
                cmd.env(key, v);
            }
            None => {
                // 清除环境变量
                cmd.env_remove(key);
            }
        }
    }
}

/// 命令执行器 trait
///
/// 抽象命令执行的接口，支持不同的运行时实现（std::process、tokio、async-std 等）。
pub trait CommandExecutor: Send + Sync {
    /// 执行命令并返回输出
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError>;
}

/// 标准库命令执行器
///
/// 使用 std::process::Command 实现，基于线程同步的同步执行。
pub struct StdCommandExecutor;

impl CommandExecutor for StdCommandExecutor {
    fn execute(&self, config: &CommandConfig) -> Result<Output, ExecuteError> {
        execute_command(config)
    }
}

/// 执行单个命令配置
///
/// 内部函数，用于启动子进程并处理超时。使用 wait-timeout crate 在同一线程中进行超时等待，
/// 避免为每个任务生成额外的等待线程，提高性能和降低系统开销。
pub(crate) fn execute_command(config: &CommandConfig) -> Result<Output, ExecuteError> {
    // 启动子进程，重定向 stdout 和 stderr
    let mut cmd = Command::new(&config.program);
    cmd.args(&config.args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = &config.working_dir {
        cmd.current_dir(dir);
    }

    // 应用环境变量配置
    if let Some(env_config) = config.env_config() {
        apply_env_config(&mut cmd, env_config);
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

/// 执行命令并返回带有丰富错误上下文的结果
///
/// 此函数提供了增强的错误处理，包含完整的执行上下文信息。
/// 支持资源限制，包括输出大小限制和内存使用限制。
///
/// # 参数
///
/// * `config` - 命令配置
/// * `task_id` - 任务 ID
///
/// # 返回
///
/// 成功时返回命令输出，失败时返回包含详细上下文的 CommandError
///
/// # 示例
///
/// ```ignore
/// use execute::{CommandConfig, executor::execute_command_with_context};
///
/// let config = CommandConfig::new("ls", vec!["-la".to_string()]);
/// let result = execute_command_with_context(&config, 1);
/// ```
pub fn execute_command_with_context(
    config: &CommandConfig,
    task_id: u64,
) -> Result<Output, CommandError> {
    let start_time = Instant::now();

    // 构建完整的命令字符串用于错误上下文
    let command_str = format!("{} {}", config.program(), config.args().join(" "));
    let working_dir = std::path::Path::new(config.working_dir().unwrap_or("."));

    // 创建错误上下文
    let create_context = || ErrorContext::new(task_id, &command_str, working_dir);

    // 启动子进程
    let mut cmd = Command::new(&config.program);
    cmd.args(&config.args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = &config.working_dir {
        cmd.current_dir(dir);
    }

    // 应用环境变量配置
    if let Some(env_config) = config.env_config() {
        apply_env_config(&mut cmd, env_config);
    }

    let mut child = cmd.spawn().map_err(|e| CommandError::SpawnFailed {
        context: create_context(),
        source: e,
    })?;

    let pid = child.id();

    // 如果配置了内存限制，启动内存监控线程
    let memory_monitor_handle = if let Some(limits) = config.resource_limits() {
        if let Some(max_memory) = limits.max_memory {
            let check_interval = std::time::Duration::from_millis(100);
            Some(std::thread::spawn(move || {
                monitor_memory(pid, max_memory, check_interval)
            }))
        } else {
            None
        }
    } else {
        None
    };

    // 根据是否设置超时进行等待处理
    let result = match config.timeout {
        Some(timeout) => {
            use wait_timeout::ChildExt;
            match child
                .wait_timeout(timeout)
                .map_err(|e| CommandError::ExecutionFailed {
                    context: create_context(),
                    source: std::io::Error::other(e),
                })? {
                Some(_) => {
                    // 子进程在超时前正常退出
                    // 如果配置了输出大小限制，使用 LimitedReader 读取输出
                    if let Some(limits) = config.resource_limits() {
                        if limits.max_output_size.is_some() {
                            // 使用 LimitedReader 读取输出
                            read_output_with_limit(&mut child, limits.max_output_size).map_err(
                                |e| CommandError::ExecutionFailed {
                                    context: create_context(),
                                    source: e,
                                },
                            )
                        } else {
                            // 无输出限制，直接读取
                            child
                                .wait_with_output()
                                .map_err(|e| CommandError::ExecutionFailed {
                                    context: create_context(),
                                    source: e,
                                })
                        }
                    } else {
                        // 无资源限制，直接读取
                        child
                            .wait_with_output()
                            .map_err(|e| CommandError::ExecutionFailed {
                                context: create_context(),
                                source: e,
                            })
                    }
                }
                None => {
                    // 超时：尝试杀死子进程
                    let _ = child.kill();
                    let _ = child.wait();
                    Err(CommandError::Timeout {
                        context: create_context(),
                        configured_timeout: timeout,
                        actual_duration: start_time.elapsed(),
                    })
                }
            }
        }
        None => {
            // 无超时限制
            if let Some(limits) = config.resource_limits() {
                if limits.max_output_size.is_some() {
                    // 使用 LimitedReader 读取输出
                    read_output_with_limit(&mut child, limits.max_output_size).map_err(|e| {
                        CommandError::ExecutionFailed {
                            context: create_context(),
                            source: e,
                        }
                    })
                } else {
                    // 无输出限制，直接读取
                    child
                        .wait_with_output()
                        .map_err(|e| CommandError::ExecutionFailed {
                            context: create_context(),
                            source: e,
                        })
                }
            } else {
                // 无资源限制，直接读取
                child
                    .wait_with_output()
                    .map_err(|e| CommandError::ExecutionFailed {
                        context: create_context(),
                        source: e,
                    })
            }
        }
    };

    // 等待内存监控线程结束
    if let Some(handle) = memory_monitor_handle {
        let _ = handle.join();
    }

    result
}

/// 使用 LimitedReader 读取子进程输出
///
/// # 参数
///
/// * `child` - 子进程
/// * `max_output_size` - 最大输出大小（字节）
///
/// # 返回
///
/// 成功时返回命令输出
fn read_output_with_limit(
    child: &mut std::process::Child,
    max_output_size: Option<usize>,
) -> std::io::Result<Output> {
    use std::io::Read;

    // 等待进程完成
    let status = child.wait()?;

    // 读取 stdout
    let mut stdout = Vec::new();
    if let Some(mut stdout_pipe) = child.stdout.take() {
        let mut limited_reader = LimitedReader::new(&mut stdout_pipe, max_output_size);
        limited_reader.read_to_end(&mut stdout)?;
    }

    // 读取 stderr
    let mut stderr = Vec::new();
    if let Some(mut stderr_pipe) = child.stderr.take() {
        let mut limited_reader = LimitedReader::new(&mut stderr_pipe, max_output_size);
        limited_reader.read_to_end(&mut stderr)?;
    }

    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// 执行命令并支持分离的超时控制
///
/// 此函数提供对启动超时和执行超时的细粒度控制。
/// 启动超时限制进程创建的时间，执行超时限制整个命令的运行时间。
///
/// # 参数
///
/// * `config` - 命令配置（包含超时配置）
/// * `task_id` - 任务 ID
///
/// # 返回
///
/// 成功时返回命令输出，失败时返回包含明确超时类型的错误
///
/// # 错误
///
/// * `TimeoutError::SpawnTimeout` - 进程启动超时
/// * `TimeoutError::ExecutionTimeout` - 命令执行超时
/// * `CommandError::SpawnFailed` - 进程启动失败
/// * `CommandError::ExecutionFailed` - 命令执行失败
///
/// # 示例
///
/// ```ignore
/// use execute::{CommandConfig, TimeoutConfig, executor::execute_with_timeouts};
/// use std::time::Duration;
///
/// let timeout_config = TimeoutConfig::new()
///     .with_spawn_timeout(Duration::from_secs(5))
///     .with_execution_timeout(Duration::from_secs(30));
/// let config = CommandConfig::new("sleep", vec!["10".to_string()])
///     .with_timeouts(timeout_config);
/// let result = execute_with_timeouts(&config, 1);
/// ```
pub fn execute_with_timeouts(config: &CommandConfig, task_id: u64) -> Result<Output, CommandError> {
    let start_time = Instant::now();

    // 构建完整的命令字符串用于错误上下文
    let command_str = format!("{} {}", config.program(), config.args().join(" "));
    let working_dir = std::path::Path::new(config.working_dir().unwrap_or("."));

    // 创建错误上下文
    let create_context = || ErrorContext::new(task_id, &command_str, working_dir);

    // 获取超时配置
    let timeout_config = match config.timeout_config() {
        Some(cfg) => cfg,
        None => {
            // 如果没有配置细粒度超时，回退到使用 execute_command_with_context
            return execute_command_with_context(config, task_id);
        }
    };

    // 构建命令
    let mut cmd = Command::new(&config.program);
    cmd.args(&config.args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = &config.working_dir {
        cmd.current_dir(dir);
    }

    // 应用环境变量配置
    if let Some(env_config) = config.env_config() {
        apply_env_config(&mut cmd, env_config);
    }

    // 处理启动超时
    let spawn_start = Instant::now();
    let mut child = if let Some(spawn_timeout) = timeout_config.spawn_timeout() {
        // 使用线程来实现启动超时
        // 注意：std::process::Command::spawn 是同步的，无法直接超时
        // 我们在这里记录启动时间，如果启动时间过长会在日志中体现
        let child = cmd.spawn().map_err(|e| CommandError::SpawnFailed {
            context: create_context(),
            source: e,
        })?;

        let spawn_duration = spawn_start.elapsed();
        if spawn_duration > spawn_timeout {
            // 启动时间超过限制，记录警告并返回超时错误
            log_warn!(
                task_id = task_id,
                spawn_duration_ms = spawn_duration.as_millis(),
                spawn_timeout_ms = spawn_timeout.as_millis(),
                "Process spawn exceeded timeout"
            );

            // 尝试终止刚启动的进程
            let mut child_mut = child;
            let _ = child_mut.kill();
            let _ = child_mut.wait();

            return Err(CommandError::Timeout {
                context: create_context(),
                configured_timeout: spawn_timeout,
                actual_duration: spawn_duration,
            });
        }

        child
    } else {
        // 无启动超时限制
        cmd.spawn().map_err(|e| CommandError::SpawnFailed {
            context: create_context(),
            source: e,
        })?
    };

    let pid = child.id();

    // 如果配置了内存限制，启动内存监控线程
    let memory_monitor_handle = if let Some(limits) = config.resource_limits() {
        if let Some(max_memory) = limits.max_memory {
            let check_interval = std::time::Duration::from_millis(100);
            Some(std::thread::spawn(move || {
                monitor_memory(pid, max_memory, check_interval)
            }))
        } else {
            None
        }
    } else {
        None
    };

    // 处理执行超时
    let result = if let Some(execution_timeout) = timeout_config.execution_timeout() {
        use wait_timeout::ChildExt;
        match child
            .wait_timeout(execution_timeout)
            .map_err(|e| CommandError::ExecutionFailed {
                context: create_context(),
                source: std::io::Error::other(e),
            })? {
            Some(_) => {
                // 子进程在超时前正常退出
                // 如果配置了输出大小限制，使用 LimitedReader 读取输出
                if let Some(limits) = config.resource_limits() {
                    if limits.max_output_size.is_some() {
                        read_output_with_limit(&mut child, limits.max_output_size).map_err(|e| {
                            CommandError::ExecutionFailed {
                                context: create_context(),
                                source: e,
                            }
                        })
                    } else {
                        child
                            .wait_with_output()
                            .map_err(|e| CommandError::ExecutionFailed {
                                context: create_context(),
                                source: e,
                            })
                    }
                } else {
                    child
                        .wait_with_output()
                        .map_err(|e| CommandError::ExecutionFailed {
                            context: create_context(),
                            source: e,
                        })
                }
            }
            None => {
                // 执行超时：尝试杀死子进程
                log_warn!(
                    task_id = task_id,
                    execution_timeout_ms = execution_timeout.as_millis(),
                    actual_duration_ms = start_time.elapsed().as_millis(),
                    "Command execution exceeded timeout"
                );

                let _ = child.kill();
                let _ = child.wait();
                Err(CommandError::Timeout {
                    context: create_context(),
                    configured_timeout: execution_timeout,
                    actual_duration: start_time.elapsed(),
                })
            }
        }
    } else {
        // 无执行超时限制
        if let Some(limits) = config.resource_limits() {
            if limits.max_output_size.is_some() {
                read_output_with_limit(&mut child, limits.max_output_size).map_err(|e| {
                    CommandError::ExecutionFailed {
                        context: create_context(),
                        source: e,
                    }
                })
            } else {
                child
                    .wait_with_output()
                    .map_err(|e| CommandError::ExecutionFailed {
                        context: create_context(),
                        source: e,
                    })
            }
        } else {
            child
                .wait_with_output()
                .map_err(|e| CommandError::ExecutionFailed {
                    context: create_context(),
                    source: e,
                })
        }
    };

    // 等待内存监控线程结束
    if let Some(handle) = memory_monitor_handle {
        let _ = handle.join();
    }

    result
}

/// 执行命令并支持重试
///
/// 此函数根据配置的重试策略自动重试失败的命令。
/// 每次重试都会记录日志，包括重试次数和失败原因。
///
/// # 参数
///
/// * `config` - 命令配置（包含重试策略）
/// * `task_id` - 任务 ID
///
/// # 返回
///
/// 成功时返回命令输出，失败时返回最后一次尝试的错误
///
/// # 示例
///
/// ```ignore
/// use execute::{CommandConfig, RetryPolicy, RetryStrategy, executor::execute_with_retry};
/// use std::time::Duration;
///
/// let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_secs(1)));
/// let config = CommandConfig::new("curl", vec!["https://example.com".to_string()])
///     .with_retry(policy);
/// let result = execute_with_retry(&config, 1);
/// ```
pub fn execute_with_retry(config: &CommandConfig, task_id: u64) -> Result<Output, CommandError> {
    // 如果没有配置重试策略，直接执行
    let retry_policy = match config.retry_policy() {
        Some(policy) => policy,
        None => {
            // 如果配置了细粒度超时，使用 execute_with_timeouts
            if config.timeout_config().is_some() {
                return execute_with_timeouts(config, task_id);
            } else {
                return execute_command_with_context(config, task_id);
            }
        }
    };

    let mut attempt = 0;
    let mut last_error = None;
    let max_attempts = retry_policy.max_attempts + 1; // +1 因为包括初始尝试

    // 重试循环
    while attempt < max_attempts {
        // 记录尝试日志
        if attempt == 0 {
            log_debug!(
                task_id = task_id,
                command = format!("{} {}", config.program(), config.args().join(" ")),
                "Executing command (initial attempt)"
            );
        } else {
            log_info!(
                task_id = task_id,
                attempt = attempt,
                max_attempts = retry_policy.max_attempts,
                command = format!("{} {}", config.program(), config.args().join(" ")),
                "Retrying command after failure"
            );
        }

        // 执行命令
        let execution_result = if config.timeout_config().is_some() {
            // 使用细粒度超时执行
            execute_with_timeouts(config, task_id)
        } else {
            // 使用标准执行
            execute_command_with_context(config, task_id)
        };

        match execution_result {
            Ok(output) => {
                // 成功，记录日志并返回
                if attempt > 0 {
                    log_info!(
                        task_id = task_id,
                        attempt = attempt,
                        "Command succeeded after retry"
                    );
                }
                return Ok(output);
            }
            Err(e) => {
                // 失败，记录错误
                attempt += 1;

                log_warn!(
                    task_id = task_id,
                    attempt = attempt,
                    max_attempts = max_attempts,
                    error = %e,
                    "Command execution failed"
                );

                last_error = Some(e);

                // 如果还有重试机会，等待后重试
                if attempt < max_attempts {
                    let delay = retry_policy.delay_for_attempt(attempt);
                    log_debug!(
                        task_id = task_id,
                        attempt = attempt,
                        delay_ms = delay.as_millis(),
                        "Waiting before retry"
                    );
                    std::thread::sleep(delay);
                }
            }
        }
    }

    // 所有尝试都失败了，返回最后一次的错误
    log_error!(
        task_id = task_id,
        attempts = attempt,
        "Command failed after all retry attempts"
    );

    Err(last_error.unwrap())
}

/// 执行任务并调用钩子
///
/// 此函数在任务执行前后调用注册的钩子，用于性能分析和自定义监控。
/// 钩子的执行被包装在 catch_unwind 中，确保钩子中的错误不会影响任务执行。
///
/// # 参数
///
/// * `config` - 命令配置
/// * `task_id` - 任务 ID
/// * `worker_id` - 工作线程 ID
/// * `hooks` - 执行钩子列表
///
/// # 返回
///
/// 成功时返回命令输出，失败时返回错误
///
/// # 需求
///
/// - Validates: Requirement 15.3 (WHEN 任务开始执行前，THE System SHALL 调用 before_execute 钩子)
/// - Validates: Requirement 15.4 (WHEN 任务执行完成后，THE System SHALL 调用 after_execute 钩子并传递执行结果)
/// - Validates: Requirement 15.5 (THE System SHALL 允许钩子访问任务 ID、命令和执行时长)
/// - Validates: Requirement 15.6 (THE System SHALL 确保钩子执行不影响任务执行的正确性)
///
/// # 示例
///
/// ```ignore
/// use execute::{CommandConfig, executor::execute_task_with_hooks};
/// use std::sync::Arc;
///
/// let config = CommandConfig::new("echo", vec!["hello".to_string()]);
/// let hooks = vec![Arc::new(MyHook::new())];
/// let result = execute_task_with_hooks(&config, 1, 0, &hooks);
/// ```
pub fn execute_task_with_hooks(
    config: &CommandConfig,
    task_id: u64,
    worker_id: usize,
    hooks: &[Arc<dyn ExecutionHook>],
) -> Result<Output, CommandError> {
    // 构建完整的命令字符串
    let command_str = format!("{} {}", config.program(), config.args().join(" "));

    // 创建执行上下文
    let ctx = ExecutionContext::new(task_id, command_str, worker_id);

    // 执行前钩子 (Requirement 15.3)
    // 使用 catch_unwind 确保钩子错误不影响任务执行 (Requirement 15.6)
    for hook in hooks {
        let hook_clone = hook.clone();
        let ctx_clone = ctx.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            hook_clone.before_execute(&ctx_clone);
        }))
        .map_err(|e| {
            log_warn!(
                task_id = task_id,
                error = ?e,
                "Hook before_execute panicked, continuing with task execution"
            );
        });
    }

    // 执行任务
    let execution_start = Instant::now();
    let execution_result = if config.retry_policy().is_some() {
        // 如果配置了重试策略，使用重试执行
        execute_with_retry(config, task_id)
    } else if config.timeout_config().is_some() {
        // 如果配置了细粒度超时，使用超时执行
        execute_with_timeouts(config, task_id)
    } else {
        // 否则使用标准执行
        execute_command_with_context(config, task_id)
    };

    // 计算执行时长
    let duration = execution_start.elapsed();

    // 构建任务结果 (Requirement 15.4, 15.5)
    let task_result = match &execution_result {
        Ok(output) => HookTaskResult::new(
            output.status.code(),
            duration,
            output.stdout.len() + output.stderr.len(),
            None,
        ),
        Err(e) => HookTaskResult::new(None, duration, 0, Some(e.to_string())),
    };

    // 执行后钩子 (Requirement 15.4)
    // 使用 catch_unwind 确保钩子错误不影响任务执行 (Requirement 15.6)
    for hook in hooks {
        let hook_clone = hook.clone();
        let ctx_clone = ctx.clone();
        let result_clone = task_result.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            hook_clone.after_execute(&ctx_clone, &result_clone);
        }))
        .map_err(|e| {
            log_warn!(
                task_id = task_id,
                error = ?e,
                "Hook after_execute panicked, ignoring error"
            );
        });
    }

    // 返回任务执行结果（不受钩子影响）
    execution_result
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

    #[test]
    #[cfg(unix)]
    fn test_execute_task_with_hooks_calls_before_and_after() {
        use std::sync::{Arc, Mutex};

        // 类型别名简化复杂类型
        type AfterCallRecord = (u64, Option<i32>);

        // 创建测试钩子
        struct TestHook {
            before_calls: Arc<Mutex<Vec<u64>>>,
            after_calls: Arc<Mutex<Vec<AfterCallRecord>>>,
        }

        impl ExecutionHook for TestHook {
            fn before_execute(&self, ctx: &ExecutionContext) {
                self.before_calls.lock().unwrap().push(ctx.task_id);
            }

            fn after_execute(&self, ctx: &ExecutionContext, result: &HookTaskResult) {
                self.after_calls
                    .lock()
                    .unwrap()
                    .push((ctx.task_id, result.exit_code));
            }
        }

        let before_calls = Arc::new(Mutex::new(Vec::new()));
        let after_calls = Arc::new(Mutex::new(Vec::new()));

        let hook = Arc::new(TestHook {
            before_calls: before_calls.clone(),
            after_calls: after_calls.clone(),
        });

        // 执行任务
        let config = CommandConfig::new("echo", vec!["test".to_string()]);
        let result = execute_task_with_hooks(&config, 42, 0, &[hook]);

        // 验证任务成功
        assert!(result.is_ok());

        // 验证钩子被调用
        assert_eq!(*before_calls.lock().unwrap(), vec![42]);
        assert_eq!(*after_calls.lock().unwrap(), vec![(42, Some(0))]);
    }

    #[test]
    #[cfg(unix)]
    fn test_execute_task_with_hooks_handles_hook_panic() {
        use std::sync::{Arc, Mutex};

        // 创建会 panic 的钩子
        struct PanicHook;

        impl ExecutionHook for PanicHook {
            fn before_execute(&self, _ctx: &ExecutionContext) {
                panic!("before_execute panic");
            }

            fn after_execute(&self, _ctx: &ExecutionContext, _result: &HookTaskResult) {
                panic!("after_execute panic");
            }
        }

        // 创建正常的钩子来验证任务仍然执行
        struct NormalHook {
            after_calls: Arc<Mutex<Vec<u64>>>,
        }

        impl ExecutionHook for NormalHook {
            fn before_execute(&self, _ctx: &ExecutionContext) {}

            fn after_execute(&self, ctx: &ExecutionContext, _result: &HookTaskResult) {
                self.after_calls.lock().unwrap().push(ctx.task_id);
            }
        }

        let after_calls = Arc::new(Mutex::new(Vec::new()));
        let normal_hook = Arc::new(NormalHook {
            after_calls: after_calls.clone(),
        });

        let panic_hook = Arc::new(PanicHook);

        // 执行任务，即使钩子 panic 也应该成功
        let config = CommandConfig::new("echo", vec!["test".to_string()]);
        let result = execute_task_with_hooks(&config, 99, 0, &[panic_hook, normal_hook]);

        // 验证任务成功（钩子 panic 不影响任务执行）
        assert!(result.is_ok());

        // 验证正常钩子仍然被调用
        assert_eq!(*after_calls.lock().unwrap(), vec![99]);
    }

    #[test]
    #[cfg(unix)]
    fn test_execute_task_with_hooks_provides_correct_context() {
        use std::sync::{Arc, Mutex};

        // 创建验证上下文的钩子
        struct ContextVerifyHook {
            captured_context: Arc<Mutex<Option<ExecutionContext>>>,
            captured_result: Arc<Mutex<Option<HookTaskResult>>>,
        }

        impl ExecutionHook for ContextVerifyHook {
            fn before_execute(&self, ctx: &ExecutionContext) {
                *self.captured_context.lock().unwrap() = Some(ctx.clone());
            }

            fn after_execute(&self, _ctx: &ExecutionContext, result: &HookTaskResult) {
                *self.captured_result.lock().unwrap() = Some(result.clone());
            }
        }

        let captured_context = Arc::new(Mutex::new(None));
        let captured_result = Arc::new(Mutex::new(None));

        let hook = Arc::new(ContextVerifyHook {
            captured_context: captured_context.clone(),
            captured_result: captured_result.clone(),
        });

        // 执行任务
        let config = CommandConfig::new("echo", vec!["hello".to_string()]);
        let _ = execute_task_with_hooks(&config, 123, 5, &[hook]);

        // 验证上下文信息
        let ctx = captured_context.lock().unwrap();
        assert!(ctx.is_some());
        let ctx = ctx.as_ref().unwrap();
        assert_eq!(ctx.task_id, 123);
        assert_eq!(ctx.worker_id, 5);
        assert!(ctx.command.contains("echo"));
        assert!(ctx.command.contains("hello"));

        // 验证结果信息
        let result = captured_result.lock().unwrap();
        assert!(result.is_some());
        let result = result.as_ref().unwrap();
        assert_eq!(result.exit_code, Some(0));
        assert!(result.duration.as_nanos() > 0);
        assert!(result.error.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn test_execute_task_with_hooks_on_failure() {
        use std::sync::{Arc, Mutex};

        // 创建钩子来捕获失败信息
        struct FailureHook {
            captured_result: Arc<Mutex<Option<HookTaskResult>>>,
        }

        impl ExecutionHook for FailureHook {
            fn before_execute(&self, _ctx: &ExecutionContext) {}

            fn after_execute(&self, _ctx: &ExecutionContext, result: &HookTaskResult) {
                *self.captured_result.lock().unwrap() = Some(result.clone());
            }
        }

        let captured_result = Arc::new(Mutex::new(None));
        let hook = Arc::new(FailureHook {
            captured_result: captured_result.clone(),
        });

        // 执行一个会失败的命令
        let config = CommandConfig::new("false", vec![]);
        let result = execute_task_with_hooks(&config, 456, 0, &[hook]);

        // 验证任务失败（false 命令返回非零退出码）
        assert!(result.is_ok()); // 命令执行成功，但退出码非零
        let output = result.unwrap();
        assert!(!output.status.success());

        // 验证钩子捕获了失败信息
        let hook_result = captured_result.lock().unwrap();
        assert!(hook_result.is_some());
        let hook_result = hook_result.as_ref().unwrap();
        assert_eq!(hook_result.exit_code, Some(1)); // false 返回 1
    }
}
