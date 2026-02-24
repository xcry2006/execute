# 设计文档

## 概述

本设计文档描述了 Rust 命令池库的生产环境就绪改进方案。改进分为三个阶段实施，重点提升系统的可观测性、可靠性和生产环境适用性。设计遵循以下原则：

- **向后兼容**: 保持现有 API 不变，新功能通过可选配置启用
- **零成本抽象**: 未启用的功能不产生运行时开销
- **类型安全**: 充分利用 Rust 类型系统防止误用
- **可测试性**: 所有功能都可通过单元测试和属性测试验证

## 架构

### 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                      CommandPool API                         │
│  (submit, shutdown, health_check, metrics)                   │
└────────────────────┬────────────────────────────────────────┘
                     │
        ┌────────────┴────────────┐
        │                         │
┌───────▼────────┐       ┌───────▼────────┐
│  CommandPool   │       │ CommandPoolSeg │
│  (Mutex-based) │       │ (Lock-free)    │
└───────┬────────┘       └───────┬────────┘
        │                        │
        └────────────┬───────────┘
                     │
        ┌────────────┴───────────────────────────────────┐
        │                                                │
┌───────▼────────┐  ┌──────────────┐  ┌───────────────▼──┐
│  Task Queue    │  │   Metrics    │  │  Health Monitor  │
│                │  │  Collector   │  │                  │
└───────┬────────┘  └──────┬───────┘  └───────┬──────────┘
        │                  │                   │
┌───────▼──────────────────▼───────────────────▼──────────┐
│              Worker Thread Pool                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ Worker 1 │  │ Worker 2 │  │ Worker N │              │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘              │
└───────┼─────────────┼─────────────┼────────────────────┘
        │             │             │
┌───────▼─────────────▼─────────────▼────────────────────┐
│              Tracing & Logging Layer                    │
│  (structured logs, spans, events)                       │
└─────────────────────────────────────────────────────────┘
```

### 核心组件关系

1. **CommandPool/CommandPoolSeg**: 主入口，负责任务调度和生命周期管理
2. **Task Queue**: 存储待执行任务，支持优先级和取消
3. **Worker Thread Pool**: 执行任务的工作线程池
4. **Metrics Collector**: 收集和聚合运行时指标
5. **Health Monitor**: 监控系统健康状态
6. **Tracing Layer**: 提供结构化日志和追踪

## 组件和接口

### Phase 1: 高优先级改进

#### 1. 结构化日志系统

**设计决策**: 使用 `tracing` crate 提供结构化日志，支持日志级别过滤和上下文传播。

**新增类型**:

```rust
// 日志配置
pub struct LogConfig {
    pub level: LogLevel,
    pub format: LogFormat,
    pub target: LogTarget,
}

pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

pub enum LogFormat {
    Json,      // 结构化 JSON 格式
    Pretty,    // 人类可读格式
    Compact,   // 紧凑格式
}

pub enum LogTarget {
    Stdout,
    Stderr,
    File(PathBuf),
}
```

**集成方式**:
- 在 `CommandPool::new()` 中初始化 tracing subscriber
- 为每个任务创建 span，包含 task_id 和 command 信息
- 在关键操作点记录事件：任务提交、开始执行、完成、错误

**日志结构**:
```rust
// 任务提交日志
tracing::info!(
    task_id = %task_id,
    command = %cmd,
    timestamp = %now,
    "Task submitted"
);

// 任务执行日志
tracing::info!(
    task_id = %task_id,
    worker_id = %worker_id,
    start_time = %start,
    "Task execution started"
);

// 任务完成日志
tracing::info!(
    task_id = %task_id,
    exit_code = %code,
    duration_ms = %duration,
    "Task completed"
);

// 错误日志
tracing::error!(
    task_id = %task_id,
    error = %err,
    context = ?ctx,
    "Task failed"
);
```

#### 2. 优雅关闭机制

**设计决策**: 使用原子标志和条件变量实现优雅关闭，支持超时强制终止。

**新增类型**:

```rust
pub struct ShutdownConfig {
    pub timeout: Duration,
    pub force_kill: bool,  // 超时后是否强制 kill 进程
}

pub enum ShutdownState {
    Running,
    ShuttingDown,
    Shutdown,
}
```

**实现策略**:

1. **停止接受新任务**: 设置 `shutdown_flag: AtomicBool`
2. **等待任务完成**: 使用 `Arc<Barrier>` 等待所有 worker 完成
3. **超时处理**: 使用 `tokio::time::timeout` 或标准库 `thread::park_timeout`
4. **资源清理**: Drop 时清理所有句柄和线程

**API 设计**:

```rust
impl CommandPool {
    pub fn shutdown(&self) -> Result<(), ShutdownError> {
        self.shutdown_with_timeout(Duration::from_secs(30))
    }
    
    pub fn shutdown_with_timeout(&self, timeout: Duration) -> Result<(), ShutdownError> {
        // 1. 设置 shutdown flag
        self.shutdown_flag.store(true, Ordering::SeqCst);
        
        // 2. 通知所有 worker
        self.shutdown_notify.notify_all();
        
        // 3. 等待 worker 完成或超时
        let start = Instant::now();
        for worker in &self.workers {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err(ShutdownError::Timeout);
            }
            worker.join_timeout(remaining)?;
        }
        
        Ok(())
    }
}
```

#### 3. 错误上下文增强

**设计决策**: 创建丰富的错误类型，包含完整的执行上下文。

**新增类型**:

```rust
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Command execution failed: {context}")]
    ExecutionFailed {
        context: ErrorContext,
        source: io::Error,
    },
    
    #[error("Command timeout: {context}")]
    Timeout {
        context: ErrorContext,
        configured_timeout: Duration,
        actual_duration: Duration,
    },
    
    #[error("Spawn failed: {context}")]
    SpawnFailed {
        context: ErrorContext,
        source: io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub task_id: TaskId,
    pub command: String,
    pub working_dir: PathBuf,
    pub timestamp: SystemTime,
    pub worker_id: Option<usize>,
}

impl ErrorContext {
    pub fn new(task_id: TaskId, command: &str, working_dir: &Path) -> Self {
        Self {
            task_id,
            command: command.to_string(),
            working_dir: working_dir.to_path_buf(),
            timestamp: SystemTime::now(),
            worker_id: None,
        }
    }
}
```

**使用示例**:

```rust
let ctx = ErrorContext::new(task_id, &cmd, &cwd);
match process.wait_timeout(timeout) {
    Ok(Some(status)) => Ok(status),
    Ok(None) => Err(CommandError::Timeout {
        context: ctx,
        configured_timeout: timeout,
        actual_duration: start.elapsed(),
    }),
    Err(e) => Err(CommandError::ExecutionFailed {
        context: ctx,
        source: e,
    }),
}
```

#### 4. CommandPoolSeg 停止机制

**设计决策**: 为无锁队列版本添加与 CommandPool 一致的停止 API。

**实现策略**:

```rust
impl CommandPoolSeg {
    pub fn stop(&self) -> Result<(), StopError> {
        // 设置停止标志
        self.stop_flag.store(true, Ordering::SeqCst);
        
        // 等待所有 worker 完成当前任务
        for worker in &self.workers {
            worker.join()?;
        }
        
        Ok(())
    }
    
    pub fn is_stopped(&self) -> bool {
        self.stop_flag.load(Ordering::SeqCst)
    }
}
```

#### 5. 配置参数验证

**设计决策**: 在构造函数中验证所有配置参数，使用 builder 模式提供友好的 API。

**新增类型**:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid thread count: {0}, must be >= 1")]
    InvalidThreadCount(usize),
    
    #[error("Invalid queue capacity: {0}, must be >= 1")]
    InvalidQueueCapacity(usize),
    
    #[error("Invalid timeout: {0:?}, must be positive")]
    InvalidTimeout(Duration),
    
    #[error("Invalid poll interval: {0:?}, must be positive")]
    InvalidPollInterval(Duration),
    
    #[error("Thread count {0} exceeds system limit {1}")]
    ThreadCountExceedsLimit(usize, usize),
}

pub struct PoolConfigBuilder {
    thread_count: Option<usize>,
    queue_capacity: Option<usize>,
    timeout: Option<Duration>,
    poll_interval: Option<Duration>,
}

impl PoolConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn thread_count(mut self, count: usize) -> Self {
        self.thread_count = Some(count);
        self
    }
    
    pub fn build(self) -> Result<PoolConfig, ConfigError> {
        let thread_count = self.thread_count.unwrap_or_else(num_cpus::get);
        
        // 验证线程数
        if thread_count < 1 {
            return Err(ConfigError::InvalidThreadCount(thread_count));
        }
        
        let max_threads = get_system_thread_limit();
        if thread_count > max_threads {
            return Err(ConfigError::ThreadCountExceedsLimit(thread_count, max_threads));
        }
        
        // 验证其他参数...
        
        Ok(PoolConfig {
            thread_count,
            // ...
        })
    }
}
```

### Phase 2: 中优先级改进

#### 6. 优化轮询机制

**设计决策**: 使用条件变量替代固定间隔轮询，减少 CPU 使用。

**实现策略**:

```rust
struct TaskQueue<T> {
    queue: Mutex<VecDeque<T>>,
    condvar: Condvar,
    shutdown: AtomicBool,
}

impl<T> TaskQueue<T> {
    pub fn pop(&self) -> Option<T> {
        let mut queue = self.queue.lock().unwrap();
        
        loop {
            if let Some(task) = queue.pop_front() {
                return Some(task);
            }
            
            if self.shutdown.load(Ordering::SeqCst) {
                return None;
            }
            
            // 等待新任务或 shutdown 信号
            queue = self.condvar.wait(queue).unwrap();
        }
    }
    
    pub fn push(&self, task: T) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(task);
        self.condvar.notify_one();  // 唤醒一个等待的 worker
    }
}
```

#### 7. 指标收集系统

**设计决策**: 使用原子计数器和 RwLock 保护的统计数据结构。

**新增类型**:

```rust
pub struct Metrics {
    // 计数器
    pub tasks_submitted: AtomicU64,
    pub tasks_completed: AtomicU64,
    pub tasks_failed: AtomicU64,
    pub tasks_cancelled: AtomicU64,
    
    // 当前状态
    pub tasks_queued: AtomicUsize,
    pub tasks_running: AtomicUsize,
    
    // 执行时间统计
    execution_times: RwLock<ExecutionStats>,
}

#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub count: u64,
    pub sum: Duration,
    pub min: Duration,
    pub max: Duration,
    pub histogram: Histogram,  // 使用 hdrhistogram crate
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub tasks_submitted: u64,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub tasks_cancelled: u64,
    pub tasks_queued: usize,
    pub tasks_running: usize,
    pub success_rate: f64,
    pub avg_execution_time: Duration,
    pub p50_execution_time: Duration,
    pub p95_execution_time: Duration,
    pub p99_execution_time: Duration,
}

impl Metrics {
    pub fn record_task_submitted(&self) {
        self.tasks_submitted.fetch_add(1, Ordering::Relaxed);
        self.tasks_queued.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn record_task_completed(&self, duration: Duration) {
        self.tasks_completed.fetch_add(1, Ordering::Relaxed);
        self.tasks_running.fetch_sub(1, Ordering::Relaxed);
        
        let mut stats = self.execution_times.write().unwrap();
        stats.record(duration);
    }
    
    pub fn snapshot(&self) -> MetricsSnapshot {
        let submitted = self.tasks_submitted.load(Ordering::Relaxed);
        let completed = self.tasks_completed.load(Ordering::Relaxed);
        let failed = self.tasks_failed.load(Ordering::Relaxed);
        
        let success_rate = if submitted > 0 {
            (completed as f64) / (submitted as f64)
        } else {
            0.0
        };
        
        let stats = self.execution_times.read().unwrap();
        
        MetricsSnapshot {
            tasks_submitted: submitted,
            tasks_completed: completed,
            tasks_failed: failed,
            tasks_cancelled: self.tasks_cancelled.load(Ordering::Relaxed),
            tasks_queued: self.tasks_queued.load(Ordering::Relaxed),
            tasks_running: self.tasks_running.load(Ordering::Relaxed),
            success_rate,
            avg_execution_time: stats.avg(),
            p50_execution_time: stats.percentile(50.0),
            p95_execution_time: stats.percentile(95.0),
            p99_execution_time: stats.percentile(99.0),
        }
    }
}
```

#### 8. 资源限制

**设计决策**: 在 CommandConfig 中添加资源限制选项，在执行时强制执行。

**新增类型**:

```rust
pub struct ResourceLimits {
    pub max_output_size: Option<usize>,  // 字节
    pub max_memory: Option<usize>,        // 字节
}

impl CommandConfig {
    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.resource_limits = Some(limits);
        self
    }
}

// 执行时应用限制
fn execute_with_limits(cmd: &mut Command, limits: &ResourceLimits) -> Result<Output> {
    let mut child = cmd.spawn()?;
    
    // 监控输出大小
    let stdout = child.stdout.take().unwrap();
    let limited_stdout = LimitedReader::new(stdout, limits.max_output_size);
    
    // 监控内存使用（使用 procfs 或 /proc/[pid]/status）
    if let Some(max_mem) = limits.max_memory {
        let pid = child.id();
        std::thread::spawn(move || {
            monitor_memory(pid, max_mem);
        });
    }
    
    let output = child.wait_with_output()?;
    Ok(output)
}

struct LimitedReader<R> {
    inner: R,
    limit: Option<usize>,
    read: usize,
}

impl<R: Read> Read for LimitedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(limit) = self.limit {
            if self.read >= limit {
                tracing::warn!("Output size limit reached: {} bytes", limit);
                return Ok(0);  // 截断
            }
            
            let max_read = std::cmp::min(buf.len(), limit - self.read);
            let n = self.inner.read(&mut buf[..max_read])?;
            self.read += n;
            Ok(n)
        } else {
            self.inner.read(buf)
        }
    }
}
```

#### 9. 僵尸进程清理

**设计决策**: 启动后台线程定期检查和清理僵尸进程。

**实现策略**:

```rust
pub struct ZombieReaper {
    check_interval: Duration,
    handle: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
}

impl ZombieReaper {
    pub fn new(check_interval: Duration) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        
        let handle = std::thread::spawn(move || {
            reaper_loop(check_interval, shutdown_clone);
        });
        
        Self {
            check_interval,
            handle: Some(handle),
            shutdown,
        }
    }
}

fn reaper_loop(interval: Duration, shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        let cleaned = reap_zombies();
        if cleaned > 0 {
            tracing::info!("Reaped {} zombie processes", cleaned);
        }
        
        std::thread::sleep(interval);
    }
}

#[cfg(unix)]
fn reap_zombies() -> usize {
    use nix::sys::wait::{waitpid, WaitPidFlag};
    use nix::unistd::Pid;
    
    let mut count = 0;
    loop {
        match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::Exited(_, _)) | Ok(WaitStatus::Signaled(_, _, _)) => {
                count += 1;
            }
            _ => break,
        }
    }
    count
}
```

#### 10. 健康检查接口

**设计决策**: 提供健康检查 API，返回系统状态和问题诊断。

**新增类型**:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded { issues: Vec<String> },
    Unhealthy { issues: Vec<String> },
}

pub struct HealthCheck {
    pub status: HealthStatus,
    pub timestamp: SystemTime,
    pub details: HealthDetails,
}

pub struct HealthDetails {
    pub workers_alive: usize,
    pub workers_total: usize,
    pub queue_usage: f64,  // 0.0 - 1.0
    pub long_running_tasks: usize,
    pub avg_task_duration: Duration,
}

impl CommandPool {
    pub fn health_check(&self) -> HealthCheck {
        let mut issues = Vec::new();
        
        // 检查工作线程
        let workers_alive = self.count_alive_workers();
        if workers_alive < self.config.thread_count {
            issues.push(format!(
                "Only {}/{} workers alive",
                workers_alive, self.config.thread_count
            ));
        }
        
        // 检查队列使用率
        let queue_usage = self.queue_usage();
        if queue_usage > 0.9 {
            issues.push(format!("Queue usage high: {:.1}%", queue_usage * 100.0));
        }
        
        // 检查长时间运行的任务
        let long_running = self.count_long_running_tasks(Duration::from_secs(300));
        if long_running > 0 {
            issues.push(format!("{} tasks running > 5 minutes", long_running));
        }
        
        let status = if issues.is_empty() {
            HealthStatus::Healthy
        } else if workers_alive > 0 {
            HealthStatus::Degraded { issues }
        } else {
            HealthStatus::Unhealthy { issues }
        };
        
        HealthCheck {
            status,
            timestamp: SystemTime::now(),
            details: HealthDetails {
                workers_alive,
                workers_total: self.config.thread_count,
                queue_usage,
                long_running_tasks: long_running,
                avg_task_duration: self.metrics.snapshot().avg_execution_time,
            },
        }
    }
}
```

### Phase 3: 低优先级改进

#### 11. 错误重试机制

**设计决策**: 在 CommandConfig 中配置重试策略，由执行器自动处理重试。

**新增类型**:

```rust
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub strategy: RetryStrategy,
}

pub enum RetryStrategy {
    FixedInterval(Duration),
    ExponentialBackoff {
        initial: Duration,
        max: Duration,
        multiplier: f64,
    },
}

impl CommandConfig {
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }
}

// 执行时应用重试
async fn execute_with_retry(
    cmd: &Command,
    policy: &RetryPolicy,
) -> Result<Output, CommandError> {
    let mut attempt = 0;
    let mut last_error = None;
    
    while attempt < policy.max_attempts {
        match execute_command(cmd).await {
            Ok(output) => return Ok(output),
            Err(e) => {
                attempt += 1;
                last_error = Some(e);
                
                if attempt < policy.max_attempts {
                    let delay = policy.strategy.delay_for_attempt(attempt);
                    tracing::warn!(
                        attempt = attempt,
                        delay_ms = delay.as_millis(),
                        "Retrying failed command"
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
    
    Err(last_error.unwrap())
}
```

#### 12. 超时粒度控制

**设计决策**: 分离启动超时和执行超时，提供更精确的控制。

**新增类型**:

```rust
pub struct TimeoutConfig {
    pub spawn_timeout: Option<Duration>,
    pub execution_timeout: Option<Duration>,
}

#[derive(Debug, thiserror::Error)]
pub enum TimeoutError {
    #[error("Spawn timeout after {0:?}")]
    SpawnTimeout(Duration),
    
    #[error("Execution timeout after {0:?}")]
    ExecutionTimeout(Duration),
}

impl CommandConfig {
    pub fn with_timeouts(mut self, config: TimeoutConfig) -> Self {
        self.timeout_config = config;
        self
    }
}

async fn execute_with_timeouts(
    cmd: &mut Command,
    config: &TimeoutConfig,
) -> Result<Output, TimeoutError> {
    // 启动超时
    let child = if let Some(spawn_timeout) = config.spawn_timeout {
        tokio::time::timeout(spawn_timeout, cmd.spawn())
            .await
            .map_err(|_| TimeoutError::SpawnTimeout(spawn_timeout))??
    } else {
        cmd.spawn()?
    };
    
    // 执行超时
    let output = if let Some(exec_timeout) = config.execution_timeout {
        tokio::time::timeout(exec_timeout, child.wait_with_output())
            .await
            .map_err(|_| TimeoutError::ExecutionTimeout(exec_timeout))??
    } else {
        child.wait_with_output().await?
    };
    
    Ok(output)
}
```

#### 13. 任务取消机制

**设计决策**: 返回 TaskHandle，支持取消队列中或执行中的任务。

**新增类型**:

```rust
pub struct TaskHandle {
    task_id: TaskId,
    cancel_token: CancellationToken,
    state: Arc<Mutex<TaskState>>,
}

#[derive(Debug, Clone, PartialEq)]
enum TaskState {
    Queued,
    Running { pid: Option<u32> },
    Completed,
    Cancelled,
}

impl TaskHandle {
    pub fn cancel(&self) -> Result<(), CancelError> {
        let mut state = self.state.lock().unwrap();
        
        match *state {
            TaskState::Queued => {
                // 从队列中移除
                self.cancel_token.cancel();
                *state = TaskState::Cancelled;
                Ok(())
            }
            TaskState::Running { pid: Some(pid) } => {
                // 终止进程
                kill_process(pid)?;
                self.cancel_token.cancel();
                *state = TaskState::Cancelled;
                Ok(())
            }
            TaskState::Completed => Err(CancelError::AlreadyCompleted),
            TaskState::Cancelled => Err(CancelError::AlreadyCancelled),
            _ => Err(CancelError::InvalidState),
        }
    }
    
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
    
    pub async fn wait(self) -> Result<Output, TaskError> {
        // 等待任务完成或取消
        tokio::select! {
            _ = self.cancel_token.cancelled() => {
                Err(TaskError::Cancelled)
            }
            result = self.wait_completion() => {
                result
            }
        }
    }
}

impl CommandPool {
    pub fn submit(&self, cmd: Command) -> Result<TaskHandle, SubmitError> {
        let task_id = self.next_task_id();
        let cancel_token = CancellationToken::new();
        let state = Arc::new(Mutex::new(TaskState::Queued));
        
        let task = Task {
            id: task_id,
            command: cmd,
            cancel_token: cancel_token.clone(),
            state: state.clone(),
        };
        
        self.queue.push(task)?;
        
        Ok(TaskHandle {
            task_id,
            cancel_token,
            state,
        })
    }
}
```

#### 14. 环境变量支持

**设计决策**: 在 CommandConfig 中添加环境变量配置。

**新增类型**:

```rust
pub struct EnvConfig {
    vars: HashMap<String, Option<String>>,  // None 表示清除变量
    inherit_parent: bool,
}

impl EnvConfig {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
            inherit_parent: true,
        }
    }
    
    pub fn set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.insert(key.into(), Some(value.into()));
        self
    }
    
    pub fn remove(mut self, key: impl Into<String>) -> Self {
        self.vars.insert(key.into(), None);
        self
    }
    
    pub fn no_inherit(mut self) -> Self {
        self.inherit_parent = false;
        self
    }
}

impl CommandConfig {
    pub fn with_env(mut self, env: EnvConfig) -> Self {
        self.env_config = Some(env);
        self
    }
}

fn apply_env_config(cmd: &mut Command, config: &EnvConfig) {
    if !config.inherit_parent {
        cmd.env_clear();
    }
    
    for (key, value) in &config.vars {
        match value {
            Some(v) => cmd.env(key, v),
            None => cmd.env_remove(key),
        };
    }
}
```

#### 15. 性能分析钩子

**设计决策**: 提供钩子接口，允许用户在任务执行前后插入自定义逻辑。

**新增类型**:

```rust
pub trait ExecutionHook: Send + Sync {
    fn before_execute(&self, ctx: &ExecutionContext);
    fn after_execute(&self, ctx: &ExecutionContext, result: &TaskResult);
}

pub struct ExecutionContext {
    pub task_id: TaskId,
    pub command: String,
    pub worker_id: usize,
    pub start_time: Instant,
}

pub struct TaskResult {
    pub exit_code: Option<i32>,
    pub duration: Duration,
    pub output_size: usize,
    pub error: Option<String>,
}

impl CommandPool {
    pub fn with_hook(mut self, hook: Arc<dyn ExecutionHook>) -> Self {
        self.hooks.push(hook);
        self
    }
}

// 在 worker 中调用钩子
fn execute_task_with_hooks(
    task: Task,
    hooks: &[Arc<dyn ExecutionHook>],
) -> TaskResult {
    let ctx = ExecutionContext {
        task_id: task.id,
        command: task.command.to_string(),
        worker_id: current_worker_id(),
        start_time: Instant::now(),
    };
    
    // 执行前钩子
    for hook in hooks {
        hook.before_execute(&ctx);
    }
    
    // 执行任务
    let result = execute_command(&task.command);
    
    let task_result = TaskResult {
        exit_code: result.as_ref().ok().and_then(|o| o.status.code()),
        duration: ctx.start_time.elapsed(),
        output_size: result.as_ref().ok().map(|o| o.stdout.len()).unwrap_or(0),
        error: result.as_ref().err().map(|e| e.to_string()),
    };
    
    // 执行后钩子
    for hook in hooks {
        hook.after_execute(&ctx, &task_result);
    }
    
    task_result
}
```

## 数据模型

### 核心数据结构

```rust
// 任务 ID
pub type TaskId = u64;

// 任务定义
pub struct Task {
    pub id: TaskId,
    pub command: Command,
    pub config: CommandConfig,
    pub cancel_token: CancellationToken,
    pub state: Arc<Mutex<TaskState>>,
}

// 命令配置
pub struct CommandConfig {
    pub working_dir: Option<PathBuf>,
    pub timeout_config: TimeoutConfig,
    pub retry_policy: Option<RetryPolicy>,
    pub resource_limits: Option<ResourceLimits>,
    pub env_config: Option<EnvConfig>,
}

// 命令池配置
pub struct PoolConfig {
    pub thread_count: usize,
    pub queue_capacity: usize,
    pub log_config: LogConfig,
    pub shutdown_config: ShutdownConfig,
    pub enable_metrics: bool,
    pub enable_health_check: bool,
    pub zombie_reaper_interval: Option<Duration>,
}
```

## 正确性属性

*属性是一个特征或行为，应该在系统的所有有效执行中保持为真——本质上是关于系统应该做什么的形式化陈述。属性作为人类可读规范和机器可验证正确性保证之间的桥梁。*

### Phase 1 属性

**属性 1: 日志完整性**
*对于任意*任务执行，日志应该包含任务的完整生命周期信息（提交、开始、完成/失败），包括任务 ID、命令、时间戳和执行结果
**验证需求: 1.1, 1.2, 1.3, 1.4, 1.5**

**属性 2: 日志级别过滤**
*对于任意*配置的日志级别，系统应该只输出该级别及以上的日志消息
**验证需求: 1.7**

**属性 3: 优雅关闭等待**
*对于任意*正在执行的任务集合，调用 shutdown 后系统应该等待所有任务完成或超时
**验证需求: 2.2, 2.3**

**属性 4: 关闭后拒绝新任务**
*对于任意*任务，在 shutdown 被调用后提交应该失败
**验证需求: 2.1**

**属性 5: 错误上下文完整性**
*对于任意*失败的命令，错误信息应该包含命令字符串、工作目录、任务 ID 和时间戳
**验证需求: 3.1, 3.2, 3.3, 3.4**

**属性 6: 超时错误详情**
*对于任意*超时的命令，错误信息应该包含配置的超时值和实际执行时长
**验证需求: 3.5**

**属性 7: CommandPoolSeg 停止行为**
*对于任意*队列中的任务，调用 stop 后它们应该继续执行完成，但新任务提交应该失败
**验证需求: 4.2, 4.3**

**属性 8: 配置验证错误消息**
*对于任意*无效配置参数，系统应该返回清晰描述问题的错误消息
**验证需求: 5.6**

### Phase 2 属性

**属性 9: 指标准确性**
*对于任意*时刻，metrics 返回的队列任务数、执行中任务数、完成任务数和失败任务数应该与实际状态一致
**验证需求: 7.1, 7.2, 7.3, 7.4**

**属性 10: 成功率计算**
*对于任意*任务集合，成功率应该等于成功任务数除以总任务数
**验证需求: 7.6**

**属性 11: 执行时间统计**
*对于任意*任务集合，统计信息（平均值、最小值、最大值、百分位数）应该正确计算
**验证需求: 7.5**

**属性 12: 输出大小限制**
*对于任意*输出超过限制的命令，输出应该被截断并记录警告
**验证需求: 8.2**

**属性 13: 内存限制终止**
*对于任意*内存使用超过限制的任务，应该被终止并返回错误
**验证需求: 8.4**

**属性 14: 僵尸进程清理**
*对于任意*已终止的子进程，系统应该定期回收，shutdown 后不应有僵尸进程残留
**验证需求: 9.1, 9.3, 9.5**

**属性 15: 健康检查准确性**
*对于任意*系统状态，health_check 应该正确报告工作线程状态、队列使用率和长时间运行任务
**验证需求: 10.2, 10.3, 10.4**

**属性 16: 健康状态分类**
*对于任意*系统状态，当所有检查通过时返回 Healthy，存在问题但可运行时返回 Degraded，无法运行时返回 Unhealthy
**验证需求: 10.5, 10.6**

### Phase 3 属性

**属性 17: 重试行为**
*对于任意*失败的任务，如果配置了重试且未达到最大次数，应该自动重试；达到最大次数后应该返回最终错误
**验证需求: 11.4, 11.6**

**属性 18: 重试日志**
*对于任意*重试的任务，日志应该包含重试次数和原因
**验证需求: 11.5**

**属性 19: 超时类型区分**
*对于任意*超时错误，应该明确区分是启动超时还是执行超时
**验证需求: 12.3, 12.4, 12.5**

**属性 20: 任务取消有效性**
*对于任意*任务，调用 cancel 后，如果任务在队列中应该被移除，如果正在执行应该被终止，并返回 Cancelled 错误
**验证需求: 13.3, 13.4, 13.5**

**属性 21: 环境变量传递**
*对于任意*设置的环境变量，子进程应该能访问到相同的值（往返测试）
**验证需求: 14.3**

**属性 22: 环境变量清除**
*对于任意*标记为清除的环境变量，子进程不应该能访问到该变量
**验证需求: 14.5**

**属性 23: 钩子调用顺序**
*对于任意*任务，before_execute 钩子应该在执行前被调用，after_execute 钩子应该在执行后被调用并接收执行结果
**验证需求: 15.3, 15.4**

**属性 24: 钩子信息访问**
*对于任意*钩子调用，应该能访问任务 ID、命令和执行时长
**验证需求: 15.5**

**属性 25: 钩子隔离性**
*对于任意*钩子实现（包括出错的），任务执行的正确性不应该受影响
**验证需求: 15.6**

## 错误处理

### 错误类型层次

```rust
#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("Submit error: {0}")]
    Submit(#[from] SubmitError),
    
    #[error("Shutdown error: {0}")]
    Shutdown(#[from] ShutdownError),
    
    #[error("Command error: {0}")]
    Command(#[from] CommandError),
}

#[derive(Debug, thiserror::Error)]
pub enum SubmitError {
    #[error("Queue is full")]
    QueueFull,
    
    #[error("Pool is shutting down")]
    ShuttingDown,
    
    #[error("Pool is stopped")]
    Stopped,
}

#[derive(Debug, thiserror::Error)]
pub enum ShutdownError {
    #[error("Shutdown timeout after {0:?}")]
    Timeout(Duration),
    
    #[error("Worker thread panicked")]
    WorkerPanic,
}
```

### 错误恢复策略

1. **配置错误**: 在构造时失败，不创建池
2. **提交错误**: 返回错误，不影响已有任务
3. **执行错误**: 记录日志，更新指标，返回错误给调用者
4. **关闭错误**: 尽力清理资源，记录未完成的任务

## 测试策略

### 单元测试

单元测试用于验证特定示例、边界条件和错误处理：

- 配置验证的边界条件（线程数 0、负超时等）
- 特定的错误场景（队列满、关闭中提交等）
- API 存在性检查（方法是否存在、返回类型正确）
- 集成点测试（组件间交互）

### 属性测试

属性测试用于验证通用正确性属性，每个测试运行最少 100 次迭代：

**测试库**: 使用 `proptest` crate 进行属性测试

**测试标记格式**: 每个属性测试必须包含注释标记
```rust
// Feature: production-ready-improvements, Property 1: 日志完整性
#[proptest]
fn test_log_completeness(task: Task) {
    // 测试实现
}
```

**关键属性测试**:

1. **日志完整性** (属性 1): 生成随机任务，验证日志包含所有必需信息
2. **优雅关闭** (属性 3): 生成随机任务集，验证 shutdown 等待行为
3. **错误上下文** (属性 5): 生成随机失败命令，验证错误信息完整性
4. **指标准确性** (属性 9): 生成随机任务序列，验证指标计数正确
5. **取消有效性** (属性 20): 生成随机任务和取消时机，验证取消行为
6. **环境变量往返** (属性 21): 生成随机环境变量，验证子进程能访问

**生成器策略**:
- 使用 `proptest` 的 `prop_compose!` 宏创建复杂类型生成器
- 为 Task、Command、Config 等类型实现 `Arbitrary` trait
- 生成边界情况：空命令、长命令、特殊字符等

### 集成测试

- 完整的任务提交-执行-完成流程
- 多线程并发提交和执行
- 优雅关闭场景
- 资源限制和超时场景
- 钩子和指标集成

### 性能测试

- 吞吐量测试：每秒处理任务数
- 延迟测试：任务提交到开始执行的延迟
- CPU 使用率：空闲和负载下的 CPU 使用
- 内存使用：长时间运行的内存稳定性

## 实施阶段

### Phase 1: 高优先级（2-3 周）

重点：可观测性和可靠性基础

1. 结构化日志系统
2. 优雅关闭机制
3. 错误上下文增强
4. CommandPoolSeg 停止机制
5. 配置参数验证

**里程碑**: 系统具备生产环境基本可观测性和可靠性

### Phase 2: 中优先级（2-3 周）

重点：性能优化和监控

6. 优化轮询机制
7. 指标收集系统
8. 资源限制
9. 僵尸进程清理
10. 健康检查接口

**里程碑**: 系统具备完整的监控和资源管理能力

### Phase 3: 低优先级（2-3 周）

重点：高级功能和扩展性

11. 错误重试机制
12. 超时粒度控制
13. 任务取消机制
14. 环境变量支持
15. 性能分析钩子

**里程碑**: 系统具备完整的生产环境特性

## 向后兼容性

所有新功能通过以下方式保持向后兼容：

1. **可选配置**: 新功能默认禁用或使用合理默认值
2. **Builder 模式**: 使用 builder 模式添加新配置，不修改现有构造函数
3. **特性标志**: 使用 Cargo features 控制可选依赖
4. **版本化 API**: 保留旧 API，新 API 使用不同名称

示例：
```rust
// 旧 API 保持不变
impl CommandPool {
    pub fn new(thread_count: usize) -> Self { ... }
}

// 新 API 使用 builder
impl CommandPool {
    pub fn builder() -> PoolConfigBuilder { ... }
}

// 使用示例
let pool = CommandPool::builder()
    .thread_count(4)
    .enable_metrics(true)
    .with_log_config(LogConfig::default())
    .build()?;
```

## 依赖项

```toml
[dependencies]
# 日志和追踪
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }

# 错误处理
thiserror = "1.0"
anyhow = "1.0"

# 指标
hdrhistogram = "7.5"

# 并发
tokio = { version = "1.0", features = ["full"], optional = true }
crossbeam = "0.8"

# 系统调用
nix = { version = "0.27", features = ["process", "signal"] }

# 测试
[dev-dependencies]
proptest = "1.0"
criterion = "0.5"
```

## 文档要求

每个新功能必须包含：

1. **API 文档**: Rustdoc 注释，包含示例
2. **使用指南**: 在 README 中添加使用示例
3. **迁移指南**: 说明如何从旧版本迁移
4. **性能影响**: 说明功能的性能开销

## 安全考虑

1. **进程隔离**: 确保子进程不能影响父进程
2. **资源限制**: 防止资源耗尽攻击
3. **输入验证**: 验证所有用户输入的命令和配置
4. **权限检查**: 不以提升的权限执行命令
5. **日志脱敏**: 避免在日志中记录敏感信息
