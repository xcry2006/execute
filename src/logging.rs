#![cfg(feature = "logging")]

use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// 日志级别
///
/// 定义日志消息的严重程度级别。
/// 级别从低到高依次为：Trace < Debug < Info < Warn < Error。
///
/// # 示例
///
/// ```
/// use execute::LogLevel;
///
/// let level = LogLevel::Info;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// 追踪级别，最详细的日志
    Trace,
    /// 调试级别，用于开发调试
    Debug,
    /// 信息级别，记录正常操作
    Info,
    /// 警告级别，记录潜在问题
    Warn,
    /// 错误级别，记录错误和异常
    Error,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Level::TRACE,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Info => Level::INFO,
            LogLevel::Warn => Level::WARN,
            LogLevel::Error => Level::ERROR,
        }
    }
}

/// 日志格式
///
/// 定义日志输出的格式。
///
/// # 变体
///
/// * `Json` - 结构化 JSON 格式，适合机器解析和日志聚合系统
/// * `Pretty` - 人类可读格式，带有颜色和缩进，适合开发调试
/// * `Compact` - 紧凑格式，单行输出，适合生产环境
///
/// # 示例
///
/// ```
/// use execute::LogFormat;
///
/// let format = LogFormat::Pretty;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// 结构化 JSON 格式
    Json,
    /// 人类可读格式
    Pretty,
    /// 紧凑格式
    Compact,
}

/// 日志输出目标
///
/// 定义日志消息的输出位置。
///
/// # 变体
///
/// * `Stdout` - 标准输出
/// * `Stderr` - 标准错误输出
/// * `File(PathBuf)` - 文件输出，指定文件路径
///
/// # 示例
///
/// ```
/// use execute::LogTarget;
/// use std::path::PathBuf;
///
/// let target = LogTarget::Stdout;
/// let file_target = LogTarget::File(PathBuf::from("/var/log/app.log"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogTarget {
    /// 标准输出
    Stdout,
    /// 标准错误输出
    Stderr,
    /// 文件输出
    File(PathBuf),
}

/// 日志配置
///
/// 用于配置日志系统的行为，包括日志级别、格式和输出目标。
///
/// # 字段
///
/// * `level` - 日志级别，只有该级别及以上的日志会被输出
/// * `format` - 日志格式
/// * `target` - 日志输出目标
///
/// # 示例
///
/// ```ignore
/// use execute::{LogConfig, LogLevel, LogFormat, LogTarget};
///
/// // 使用默认配置
/// let config = LogConfig::default();
///
/// // 自定义配置
/// let config = LogConfig::new()
///     .with_level(LogLevel::Debug)
///     .with_format(LogFormat::Json)
///     .with_target(LogTarget::Stderr);
///
/// // 初始化日志系统
/// config.init().unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct LogConfig {
    pub level: LogLevel,
    pub format: LogFormat,
    pub target: LogTarget,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Pretty,
            target: LogTarget::Stdout,
        }
    }
}

impl LogConfig {
    /// 创建新的日志配置
    ///
    /// 使用默认值：Info 级别、Pretty 格式、Stdout 输出。
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::LogConfig;
    ///
    /// let config = LogConfig::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置日志级别
    ///
    /// # 参数
    ///
    /// * `level` - 日志级别
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::{LogConfig, LogLevel};
    ///
    /// let config = LogConfig::new().with_level(LogLevel::Debug);
    /// ```
    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    /// 设置日志格式
    ///
    /// # 参数
    ///
    /// * `format` - 日志格式
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::{LogConfig, LogFormat};
    ///
    /// let config = LogConfig::new().with_format(LogFormat::Json);
    /// ```
    pub fn with_format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// 设置日志输出目标
    ///
    /// # 参数
    ///
    /// * `target` - 日志输出目标
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::{LogConfig, LogTarget};
    ///
    /// let config = LogConfig::new().with_target(LogTarget::Stderr);
    /// ```
    pub fn with_target(mut self, target: LogTarget) -> Self {
        self.target = target;
        self
    }

    /// 初始化日志系统
    ///
    /// 根据配置初始化 tracing subscriber。
    /// 此方法只能调用一次，多次调用会返回错误。
    ///
    /// # 返回
    ///
    /// * `Ok(())` - 初始化成功
    /// * `Err(Box<dyn std::error::Error>)` - 初始化失败
    ///
    /// # 错误
    ///
    /// 如果日志系统已经初始化，或者配置无效，会返回错误。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// use execute::{LogConfig, LogLevel};
    ///
    /// let config = LogConfig::new().with_level(LogLevel::Info);
    /// config.init().expect("Failed to initialize logging");
    ///
    /// tracing::info!("Logging initialized");
    /// ```
    pub fn init(&self) -> Result<(), Box<dyn std::error::Error>> {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            let level: Level = self.level.into();
            EnvFilter::new(level.to_string())
        });

        match self.format {
            LogFormat::Json => {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().json())
                    .try_init()?;
            }
            LogFormat::Pretty => {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().pretty())
                    .try_init()?;
            }
            LogFormat::Compact => {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(fmt::layer().compact())
                    .try_init()?;
            }
        }

        Ok(())
    }
}
