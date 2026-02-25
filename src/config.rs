use std::collections::HashMap;
use std::time::Duration;

use crate::error::ConfigError;

/// 重试策略
///
/// 定义任务失败后的重试行为，包括最大重试次数和重试延迟策略。
///
/// # 示例
///
/// ```ignore
/// use execute::config::{RetryPolicy, RetryStrategy};
/// use std::time::Duration;
///
/// // 固定间隔重试
/// let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_secs(1)));
///
/// // 指数退避重试
/// let policy = RetryPolicy::new(
///     5,
///     RetryStrategy::ExponentialBackoff {
///         initial: Duration::from_millis(100),
///         max: Duration::from_secs(10),
///         multiplier: 2.0,
///     }
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    /// 最大重试次数（不包括初始尝试）
    ///
    /// 例如，max_attempts = 3 表示总共会尝试 4 次（1 次初始 + 3 次重试）
    pub max_attempts: usize,

    /// 重试延迟策略
    pub strategy: RetryStrategy,
}

impl RetryPolicy {
    /// 创建新的重试策略
    ///
    /// # 参数
    ///
    /// * `max_attempts` - 最大重试次数（不包括初始尝试）
    /// * `strategy` - 重试延迟策略
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::{RetryPolicy, RetryStrategy};
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_secs(1)));
    /// ```
    pub fn new(max_attempts: usize, strategy: RetryStrategy) -> Self {
        Self {
            max_attempts,
            strategy,
        }
    }

    /// 计算指定重试次数的延迟时间
    ///
    /// # 参数
    ///
    /// * `attempt` - 重试次数（从 1 开始）
    ///
    /// # 返回
    ///
    /// 该次重试前应该等待的时间
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::{RetryPolicy, RetryStrategy};
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_secs(1)));
    /// assert_eq!(policy.delay_for_attempt(1), Duration::from_secs(1));
    /// assert_eq!(policy.delay_for_attempt(2), Duration::from_secs(1));
    ///
    /// let policy = RetryPolicy::new(
    ///     3,
    ///     RetryStrategy::ExponentialBackoff {
    ///         initial: Duration::from_millis(100),
    ///         max: Duration::from_secs(10),
    ///         multiplier: 2.0,
    ///     }
    /// );
    /// assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(100));
    /// assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
    /// assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
    /// ```
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        self.strategy.delay_for_attempt(attempt)
    }
}

/// 重试延迟策略
///
/// 定义如何计算每次重试之间的延迟时间。
#[derive(Debug, Clone, PartialEq)]
pub enum RetryStrategy {
    /// 固定间隔重试
    ///
    /// 每次重试之间等待相同的时间。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::RetryStrategy;
    /// use std::time::Duration;
    ///
    /// let strategy = RetryStrategy::FixedInterval(Duration::from_secs(1));
    /// ```
    FixedInterval(Duration),

    /// 指数退避重试
    ///
    /// 每次重试的延迟时间按指数增长，直到达到最大值。
    /// 延迟计算公式：min(initial * multiplier^(attempt-1), max)
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::RetryStrategy;
    /// use std::time::Duration;
    ///
    /// let strategy = RetryStrategy::ExponentialBackoff {
    ///     initial: Duration::from_millis(100),
    ///     max: Duration::from_secs(10),
    ///     multiplier: 2.0,
    /// };
    /// ```
    ExponentialBackoff {
        /// 初始延迟时间
        initial: Duration,
        /// 最大延迟时间
        max: Duration,
        /// 延迟增长倍数
        multiplier: f64,
    },
}

impl RetryStrategy {
    /// 计算指定重试次数的延迟时间
    ///
    /// # 参数
    ///
    /// * `attempt` - 重试次数（从 1 开始）
    ///
    /// # 返回
    ///
    /// 该次重试前应该等待的时间
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        match self {
            RetryStrategy::FixedInterval(duration) => *duration,
            RetryStrategy::ExponentialBackoff {
                initial,
                max,
                multiplier,
            } => {
                // 处理边界情况：attempt 为 0 时返回 initial
                if attempt == 0 {
                    return *initial;
                }

                // 计算指数退避延迟：initial * multiplier^(attempt-1)
                let initial_ms = initial.as_millis() as f64;
                let exponent = (attempt - 1) as f64;
                let delay_ms = initial_ms * multiplier.powf(exponent);

                // 限制在最大值以内
                let max_ms = max.as_millis() as u64;
                let delay_ms = delay_ms.min(max_ms as f64) as u64;

                Duration::from_millis(delay_ms)
            }
        }
    }
}

/// 资源限制配置
///
/// 用于限制命令执行时的资源使用，防止单个任务消耗过多资源。
///
/// # 示例
///
/// ```ignore
/// use execute::config::ResourceLimits;
///
/// let limits = ResourceLimits::new()
///     .with_max_output_size(1024 * 1024)  // 1 MB
///     .with_max_memory(100 * 1024 * 1024); // 100 MB
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceLimits {
    /// 最大输出大小（字节）
    ///
    /// 限制命令的 stdout 和 stderr 输出总大小。
    /// 超过此限制时，输出将被截断并记录警告。
    pub max_output_size: Option<usize>,

    /// 最大内存使用（字节）
    ///
    /// 限制子进程的内存使用。
    /// 超过此限制时，进程将被终止并返回错误。
    pub max_memory: Option<usize>,
}

impl ResourceLimits {
    /// 创建新的资源限制配置
    ///
    /// 默认不设置任何限制。
    pub fn new() -> Self {
        Self {
            max_output_size: None,
            max_memory: None,
        }
    }

    /// 设置最大输出大小
    ///
    /// # 参数
    ///
    /// * `size` - 最大输出大小（字节）
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::ResourceLimits;
    ///
    /// let limits = ResourceLimits::new()
    ///     .with_max_output_size(1024 * 1024); // 1 MB
    /// ```
    pub fn with_max_output_size(mut self, size: usize) -> Self {
        self.max_output_size = Some(size);
        self
    }

    /// 设置最大内存使用
    ///
    /// # 参数
    ///
    /// * `size` - 最大内存使用（字节）
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::ResourceLimits;
    ///
    /// let limits = ResourceLimits::new()
    ///     .with_max_memory(100 * 1024 * 1024); // 100 MB
    /// ```
    pub fn with_max_memory(mut self, size: usize) -> Self {
        self.max_memory = Some(size);
        self
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self::new()
    }
}

/// CommandConfig 表示要执行的外部命令及其执行参数。
///
/// 字段：
/// - `program`: 可执行程序名或路径。
/// - `args`: 传递给程序的参数列表。
/// - `working_dir`: 可选的工作目录，若为 `None` 则使用当前目录。
/// - `timeout`: 可选的超时时间，超时后会尝试终止子进程。
/// - `resource_limits`: 可选的资源限制配置。
/// - `retry_policy`: 可选的重试策略配置。
/// - `timeout_config`: 可选的细粒度超时配置。
/// - `env_config`: 可选的环境变量配置。
///
/// 示例（构造一个带超时的命令配置）：
/// ```ignore
/// use execute::CommandConfig;
/// use std::time::Duration;
///
/// let cfg = CommandConfig::new("sleep", vec!["5".to_string()])
///     .with_timeout(Duration::from_secs(2));
/// ```
#[derive(Debug, Clone)]
#[derive(PartialEq)]
pub struct CommandConfig {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) working_dir: Option<String>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) resource_limits: Option<ResourceLimits>,
    pub(crate) retry_policy: Option<RetryPolicy>,
    pub(crate) timeout_config: Option<TimeoutConfig>,
    pub(crate) env_config: Option<EnvConfig>,
}

impl CommandConfig {
    /// # 创建一个CommandConfig结构体
    ///
    /// # 参数
    /// - `program`: 执行的命令
    /// - `args`: 命令参数列表
    ///
    /// # 示例
    /// ```ignore
    /// let cfg = CommandConfig::new("echo", vec!["hello".to_string()]);
    /// println!("program = {}", cfg.program());
    /// ```
    pub fn new(program: &str, args: Vec<String>) -> Self {
        Self {
            program: program.to_string(),
            args,
            working_dir: None,
            timeout: Some(Duration::from_secs(10)),
            resource_limits: None,
            retry_policy: None,
            timeout_config: None,
            env_config: None,
        }
    }

    /// # 设置任务的工作目录
    ///
    /// 将命令的工作目录设置为给定路径，返回修改后的 `CommandConfig`，便于链式调用。
    ///
    /// # 参数
    /// - `dir`: 要在其中执行命令的工作目录路径。
    ///
    /// # 示例
    /// ```ignore
    /// use execute::CommandConfig;
    ///
    /// let cmd = CommandConfig::new("ls", vec!["-la".to_string()])
    ///     .with_working_dir("/tmp");
    /// assert_eq!(cmd.working_dir().unwrap(), "/tmp".to_string());
    /// ```
    pub fn with_working_dir(mut self, dir: &str) -> Self {
        self.working_dir = Some(dir.to_string());
        self
    }

    /// # 设置任务超时时间
    ///
    /// 为该命令设置最大执行时长，超时后会尝试终止子进程并返回 `ExecuteError::Timeout`。
    ///
    /// # 参数
    /// - `timeout`: 超时时间长度，`Duration` 类型。
    ///
    /// # 示例
    /// ```ignore
    /// use execute::CommandConfig;
    /// use std::time::Duration;
    ///
    /// let cmd = CommandConfig::new("sleep", vec!["5".to_string()])
    ///     .with_timeout(Duration::from_secs(2));
    /// assert_eq!(cmd.timeout().unwrap().as_secs(), 2);
    /// ```
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// # 获取程序名
    pub fn program(&self) -> &str {
        &self.program
    }

    /// # 获取命令参数
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// # 获取工作目录
    pub fn working_dir(&self) -> Option<&str> {
        self.working_dir.as_deref()
    }

    /// # 获取超时时间
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// # 设置资源限制
    ///
    /// 为该命令设置资源使用限制，包括输出大小和内存使用。
    ///
    /// # 参数
    /// - `limits`: 资源限制配置
    ///
    /// # 示例
    /// ```ignore
    /// use execute::{CommandConfig, ResourceLimits};
    ///
    /// let limits = ResourceLimits::new()
    ///     .with_max_output_size(1024 * 1024);
    /// let cmd = CommandConfig::new("ls", vec!["-la".to_string()])
    ///     .with_resource_limits(limits);
    /// ```
    pub fn with_resource_limits(mut self, limits: ResourceLimits) -> Self {
        self.resource_limits = Some(limits);
        self
    }

    /// # 获取资源限制
    pub fn resource_limits(&self) -> Option<&ResourceLimits> {
        self.resource_limits.as_ref()
    }

    /// # 设置重试策略
    ///
    /// 为该命令设置失败后的重试策略。
    ///
    /// # 参数
    /// - `policy`: 重试策略配置
    ///
    /// # 示例
    /// ```ignore
    /// use execute::{CommandConfig, RetryPolicy, RetryStrategy};
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_secs(1)));
    /// let cmd = CommandConfig::new("curl", vec!["https://example.com".to_string()])
    ///     .with_retry(policy);
    /// ```
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }

    /// # 获取重试策略
    pub fn retry_policy(&self) -> Option<&RetryPolicy> {
        self.retry_policy.as_ref()
    }

    /// # 设置细粒度超时配置
    ///
    /// 为该命令设置分离的启动超时和执行超时。
    /// 这提供了比单一超时更精确的控制。
    ///
    /// # 参数
    /// - `config`: 超时配置
    ///
    /// # 示例
    /// ```ignore
    /// use execute::{CommandConfig, TimeoutConfig};
    /// use std::time::Duration;
    ///
    /// let timeout_config = TimeoutConfig::new()
    ///     .with_spawn_timeout(Duration::from_secs(5))
    ///     .with_execution_timeout(Duration::from_secs(30));
    /// let cmd = CommandConfig::new("sleep", vec!["10".to_string()])
    ///     .with_timeouts(timeout_config);
    /// ```
    pub fn with_timeouts(mut self, config: TimeoutConfig) -> Self {
        self.timeout_config = Some(config);
        self
    }

    /// # 获取超时配置
    pub fn timeout_config(&self) -> Option<&TimeoutConfig> {
        self.timeout_config.as_ref()
    }

    /// # 设置环境变量配置
    ///
    /// 为该命令设置环境变量配置，包括设置、清除环境变量和控制继承行为。
    ///
    /// # 参数
    /// - `env`: 环境变量配置
    ///
    /// # 示例
    /// ```ignore
    /// use execute::{CommandConfig, EnvConfig};
    ///
    /// let env = EnvConfig::new()
    ///     .set("PATH", "/usr/local/bin:/usr/bin")
    ///     .set("HOME", "/home/user");
    /// let cmd = CommandConfig::new("ls", vec!["-la".to_string()])
    ///     .with_env(env);
    /// ```
    pub fn with_env(mut self, env: EnvConfig) -> Self {
        self.env_config = Some(env);
        self
    }

    /// # 获取环境变量配置
    pub fn env_config(&self) -> Option<&EnvConfig> {
        self.env_config.as_ref()
    }
}

/// 命令池配置
///
/// 包含命令池的所有配置参数，包括线程数、队列容量、超时等。
/// 使用 PoolConfigBuilder 创建并验证配置。
#[derive(Debug, Clone, PartialEq)]
pub struct PoolConfig {
    /// 工作线程数
    pub thread_count: usize,
    /// 队列容量（None 表示无限制）
    pub queue_capacity: Option<usize>,
    /// 默认超时时间
    pub default_timeout: Option<Duration>,
    /// 轮询间隔
    pub poll_interval: Duration,
}

impl PoolConfig {
    /// 创建配置构建器
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::PoolConfig;
    /// use std::time::Duration;
    ///
    /// let config = PoolConfig::builder()
    ///     .thread_count(4)
    ///     .queue_capacity(100)
    ///     .poll_interval(Duration::from_millis(100))
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder() -> PoolConfigBuilder {
        PoolConfigBuilder::new()
    }
}

/// 命令池配置构建器
///
/// 使用 builder 模式创建和验证命令池配置。
/// 所有配置参数都会在 build() 时进行验证。
///
/// # 示例
///
/// ```ignore
/// use execute::config::PoolConfigBuilder;
/// use std::time::Duration;
///
/// let config = PoolConfigBuilder::new()
///     .thread_count(4)
///     .queue_capacity(100)
///     .default_timeout(Duration::from_secs(30))
///     .poll_interval(Duration::from_millis(100))
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Default)]
pub struct PoolConfigBuilder {
    thread_count: Option<usize>,
    queue_capacity: Option<usize>,
    default_timeout: Option<Duration>,
    poll_interval: Option<Duration>,
}

impl PoolConfigBuilder {
    /// 创建新的配置构建器
    ///
    /// 所有字段初始为 None，将在 build() 时使用默认值或验证用户提供的值。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置工作线程数
    ///
    /// # 参数
    ///
    /// * `count` - 工作线程数，必须 >= 1
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::PoolConfigBuilder;
    ///
    /// let builder = PoolConfigBuilder::new().thread_count(4);
    /// ```
    pub fn thread_count(mut self, count: usize) -> Self {
        self.thread_count = Some(count);
        self
    }

    /// 设置队列容量
    ///
    /// # 参数
    ///
    /// * `capacity` - 队列容量，必须 >= 1
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::PoolConfigBuilder;
    ///
    /// let builder = PoolConfigBuilder::new().queue_capacity(100);
    /// ```
    pub fn queue_capacity(mut self, capacity: usize) -> Self {
        self.queue_capacity = Some(capacity);
        self
    }

    /// 设置默认超时时间
    ///
    /// # 参数
    ///
    /// * `timeout` - 默认超时时间，必须为正数
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::PoolConfigBuilder;
    /// use std::time::Duration;
    ///
    /// let builder = PoolConfigBuilder::new()
    ///     .default_timeout(Duration::from_secs(30));
    /// ```
    pub fn default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = Some(timeout);
        self
    }

    /// 设置轮询间隔
    ///
    /// # 参数
    ///
    /// * `interval` - 轮询间隔，必须为正数
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::PoolConfigBuilder;
    /// use std::time::Duration;
    ///
    /// let builder = PoolConfigBuilder::new()
    ///     .poll_interval(Duration::from_millis(100));
    /// ```
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = Some(interval);
        self
    }

    /// 构建并验证配置
    ///
    /// 验证所有配置参数，如果有任何参数无效则返回错误。
    /// 未设置的参数将使用合理的默认值。
    ///
    /// # 返回
    ///
    /// * `Ok(PoolConfig)` - 验证通过的配置
    /// * `Err(ConfigError)` - 配置验证失败
    ///
    /// # 错误
    ///
    /// * `ConfigError::InvalidThreadCount` - 线程数 < 1
    /// * `ConfigError::ThreadCountExceedsLimit` - 线程数超过系统限制
    /// * `ConfigError::InvalidQueueCapacity` - 队列容量 < 1
    /// * `ConfigError::InvalidTimeout` - 超时时间 <= 0
    /// * `ConfigError::InvalidPollInterval` - 轮询间隔 <= 0
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::PoolConfigBuilder;
    /// use std::time::Duration;
    ///
    /// let config = PoolConfigBuilder::new()
    ///     .thread_count(4)
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn build(self) -> Result<PoolConfig, ConfigError> {
        // 验证线程数
        let thread_count = self.thread_count.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });

        if thread_count < 1 {
            return Err(ConfigError::InvalidThreadCount(thread_count));
        }

        // 检查系统线程限制
        let max_threads = get_system_thread_limit();
        if thread_count > max_threads {
            return Err(ConfigError::ThreadCountExceedsLimit(
                thread_count,
                max_threads,
            ));
        }

        // 验证队列容量
        if let Some(capacity) = self.queue_capacity
            && capacity < 1
        {
            return Err(ConfigError::InvalidQueueCapacity(capacity));
        }

        // 验证超时时间
        if let Some(timeout) = self.default_timeout
            && timeout.is_zero()
        {
            return Err(ConfigError::InvalidTimeout(timeout));
        }

        // 验证轮询间隔
        let poll_interval = self.poll_interval.unwrap_or(Duration::from_millis(100));
        if poll_interval.is_zero() {
            return Err(ConfigError::InvalidPollInterval(poll_interval));
        }

        Ok(PoolConfig {
            thread_count,
            queue_capacity: self.queue_capacity,
            default_timeout: self.default_timeout,
            poll_interval,
        })
    }
}

/// 关闭配置
///
/// 配置命令池的优雅关闭行为。
#[derive(Debug, Clone)]
pub struct ShutdownConfig {
    /// 关闭超时时间
    ///
    /// 等待所有任务完成的最大时间。
    /// 超时后将强制终止剩余任务（如果 force_kill 为 true）。
    pub timeout: Duration,

    /// 超时后是否强制终止进程
    ///
    /// 如果为 true，超时后会强制 kill 所有正在执行的进程。
    /// 如果为 false，超时后只是返回错误，但不会强制终止进程。
    pub force_kill: bool,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            force_kill: false,
        }
    }
}

impl ShutdownConfig {
    /// 创建新的关闭配置
    ///
    /// # 参数
    ///
    /// * `timeout` - 关闭超时时间
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::ShutdownConfig;
    /// use std::time::Duration;
    ///
    /// let config = ShutdownConfig::new(Duration::from_secs(60));
    /// ```
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            force_kill: false,
        }
    }

    /// 设置是否在超时后强制终止进程
    ///
    /// # 参数
    ///
    /// * `force_kill` - 是否强制终止
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::ShutdownConfig;
    /// use std::time::Duration;
    ///
    /// let config = ShutdownConfig::new(Duration::from_secs(60))
    ///     .with_force_kill(true);
    /// ```
    pub fn with_force_kill(mut self, force_kill: bool) -> Self {
        self.force_kill = force_kill;
        self
    }
}

/// 关闭状态
///
/// 表示命令池的关闭状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ShutdownState {
    /// 正常运行
    Running,
    /// 正在关闭
    ShuttingDown,
    /// 已关闭
    Shutdown,
}

/// 超时配置
///
/// 提供对命令启动和执行的细粒度超时控制。
/// 分离启动超时和执行超时可以更精确地管理任务生命周期。
///
/// # 示例
///
/// ```ignore
/// use execute::config::TimeoutConfig;
/// use std::time::Duration;
///
/// // 设置启动超时和执行超时
/// let config = TimeoutConfig::new()
///     .with_spawn_timeout(Duration::from_secs(5))
///     .with_execution_timeout(Duration::from_secs(30));
///
/// // 只设置执行超时
/// let config = TimeoutConfig::new()
///     .with_execution_timeout(Duration::from_secs(30));
/// ```
#[derive(Debug, Clone, Default)]
#[derive(PartialEq)]
pub struct TimeoutConfig {
    /// 命令启动超时
    ///
    /// 限制从调用 spawn 到进程成功启动的最大时间。
    /// 如果启动时间超过此值，将取消启动并返回 TimeoutError::SpawnTimeout。
    pub spawn_timeout: Option<Duration>,

    /// 命令执行超时
    ///
    /// 限制命令从启动到完成的最大执行时间。
    /// 如果执行时间超过此值，将终止进程并返回 TimeoutError::ExecutionTimeout。
    pub execution_timeout: Option<Duration>,
}

impl TimeoutConfig {
    /// 创建新的超时配置
    ///
    /// 默认不设置任何超时限制。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::TimeoutConfig;
    ///
    /// let config = TimeoutConfig::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置启动超时
    ///
    /// # 参数
    ///
    /// * `timeout` - 启动超时时间
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::TimeoutConfig;
    /// use std::time::Duration;
    ///
    /// let config = TimeoutConfig::new()
    ///     .with_spawn_timeout(Duration::from_secs(5));
    /// ```
    pub fn with_spawn_timeout(mut self, timeout: Duration) -> Self {
        self.spawn_timeout = Some(timeout);
        self
    }

    /// 设置执行超时
    ///
    /// # 参数
    ///
    /// * `timeout` - 执行超时时间
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::TimeoutConfig;
    /// use std::time::Duration;
    ///
    /// let config = TimeoutConfig::new()
    ///     .with_execution_timeout(Duration::from_secs(30));
    /// ```
    pub fn with_execution_timeout(mut self, timeout: Duration) -> Self {
        self.execution_timeout = Some(timeout);
        self
    }

    /// 获取启动超时
    pub fn spawn_timeout(&self) -> Option<Duration> {
        self.spawn_timeout
    }

    /// 获取执行超时
    pub fn execution_timeout(&self) -> Option<Duration> {
        self.execution_timeout
    }
}

/// 环境变量配置
///
/// 用于配置命令执行时的环境变量。
/// 支持设置、清除环境变量，以及控制是否继承父进程的环境变量。
///
/// # 示例
///
/// ```ignore
/// use execute::config::EnvConfig;
///
/// // 设置环境变量
/// let env = EnvConfig::new()
///     .set("PATH", "/usr/local/bin:/usr/bin")
///     .set("HOME", "/home/user");
///
/// // 清除特定环境变量
/// let env = EnvConfig::new()
///     .remove("TEMP_VAR");
///
/// // 不继承父进程环境变量
/// let env = EnvConfig::new()
///     .no_inherit()
///     .set("PATH", "/usr/bin");
/// ```
#[derive(Debug, Clone)]
#[derive(PartialEq)]
pub struct EnvConfig {
    /// 环境变量映射
    ///
    /// - `Some(value)`: 设置环境变量为指定值
    /// - `None`: 清除该环境变量
    vars: HashMap<String, Option<String>>,

    /// 是否继承父进程的环境变量
    ///
    /// 如果为 true，子进程将继承父进程的所有环境变量，
    /// 然后应用 vars 中的修改。
    /// 如果为 false，子进程只会有 vars 中设置的环境变量。
    inherit_parent: bool,
}

impl EnvConfig {
    /// 创建新的环境变量配置
    ///
    /// 默认继承父进程的环境变量。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::EnvConfig;
    ///
    /// let env = EnvConfig::new();
    /// ```
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
            inherit_parent: true,
        }
    }

    /// 设置环境变量
    ///
    /// 设置指定的环境变量为给定值。
    /// 如果该变量已存在，将覆盖其值。
    ///
    /// # 参数
    ///
    /// * `key` - 环境变量名
    /// * `value` - 环境变量值
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::EnvConfig;
    ///
    /// let env = EnvConfig::new()
    ///     .set("PATH", "/usr/local/bin:/usr/bin")
    ///     .set("HOME", "/home/user");
    /// ```
    pub fn set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.insert(key.into(), Some(value.into()));
        self
    }

    /// 清除环境变量
    ///
    /// 标记指定的环境变量为清除状态。
    /// 即使父进程有该变量，子进程也不会继承它。
    ///
    /// # 参数
    ///
    /// * `key` - 要清除的环境变量名
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::EnvConfig;
    ///
    /// let env = EnvConfig::new()
    ///     .remove("TEMP_VAR")
    ///     .remove("DEBUG");
    /// ```
    pub fn remove(mut self, key: impl Into<String>) -> Self {
        self.vars.insert(key.into(), None);
        self
    }

    /// 不继承父进程的环境变量
    ///
    /// 设置后，子进程将不会继承父进程的任何环境变量，
    /// 只会有通过 `set()` 方法显式设置的环境变量。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::config::EnvConfig;
    ///
    /// // 子进程只有 PATH 环境变量
    /// let env = EnvConfig::new()
    ///     .no_inherit()
    ///     .set("PATH", "/usr/bin");
    /// ```
    pub fn no_inherit(mut self) -> Self {
        self.inherit_parent = false;
        self
    }

    /// 获取环境变量映射
    ///
    /// 返回所有配置的环境变量。
    /// - `Some(value)`: 该变量应设置为指定值
    /// - `None`: 该变量应被清除
    pub fn vars(&self) -> &HashMap<String, Option<String>> {
        &self.vars
    }

    /// 是否继承父进程环境变量
    ///
    /// 返回是否继承父进程的环境变量。
    pub fn inherit_parent(&self) -> bool {
        self.inherit_parent
    }

    /// 应用到 Command
    ///
    /// 将环境变量配置应用到 std::process::Command。
    pub fn apply_to_command(&self, cmd: &mut std::process::Command) {
        if !self.inherit_parent {
            cmd.env_clear();
        }

        // 设置环境变量
        for (key, value) in &self.vars {
            match value {
                Some(v) => {
                    cmd.env(key, v);
                }
                None => {
                    cmd.env_remove(key);
                }
            }
        }
    }
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// 获取系统线程限制
///
/// 尝试从系统获取最大线程数限制。
/// 如果无法获取，返回一个合理的默认值（10000）。
fn get_system_thread_limit() -> usize {
    // 在 Linux 上，可以从 /proc/sys/kernel/threads-max 读取
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/sys/kernel/threads-max")
            && let Ok(limit) = content.trim().parse::<usize>()
        {
            return limit;
        }
    }

    // 在 macOS 上，可以使用 sysctl
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("sysctl")
            .arg("-n")
            .arg("kern.maxproc")
            .output()
        {
            if let Ok(s) = String::from_utf8(output.stdout) {
                if let Ok(limit) = s.trim().parse::<usize>() {
                    return limit;
                }
            }
        }
    }

    // 默认限制
    10000
}
