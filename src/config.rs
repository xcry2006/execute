use std::time::Duration;

/// CommandConfig 表示要执行的外部命令及其执行参数。
///
/// 字段：
/// - `program`: 可执行程序名或路径。
/// - `args`: 传递给程序的参数列表。
/// - `working_dir`: 可选的工作目录，若为 `None` 则使用当前目录。
/// - `timeout`: 可选的超时时间，超时后会尝试终止子进程。
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
pub struct CommandConfig {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) working_dir: Option<String>,
    pub(crate) timeout: Option<Duration>,
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
}

