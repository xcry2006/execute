use std::process::Output;

use crate::config::CommandConfig;
use crate::error::ExecuteError;

/// Pipeline 阶段
///
/// 表示 pipeline 中的一个命令阶段
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// 命令配置
    pub config: CommandConfig,
    /// 是否忽略前一个阶段的输出（作为独立命令运行）
    pub ignore_input: bool,
}

impl PipelineStage {
    /// 创建新的 pipeline 阶段
    pub fn new(config: CommandConfig) -> Self {
        Self {
            config,
            ignore_input: false,
        }
    }

    /// 设置是否忽略输入
    pub fn ignore_input(mut self, ignore: bool) -> Self {
        self.ignore_input = ignore;
        self
    }
}

/// Pipeline 构建器
///
/// 用于构建命令 pipeline，支持链式调用
#[derive(Debug, Clone)]
pub struct Pipeline {
    stages: Vec<PipelineStage>,
}

impl Pipeline {
    /// 创建空的 pipeline
    pub fn new() -> Self {
        Self { stages: vec![] }
    }

    /// 添加阶段到 pipeline
    pub fn add_stage(mut self, stage: PipelineStage) -> Self {
        self.stages.push(stage);
        self
    }

    /// 添加命令到 pipeline（快捷方法）
    pub fn pipe(mut self, config: CommandConfig) -> Self {
        self.stages.push(PipelineStage::new(config));
        self
    }

    /// 获取所有阶段
    pub fn stages(&self) -> &[PipelineStage] {
        &self.stages
    }

    /// 获取阶段数量
    pub fn len(&self) -> usize {
        self.stages.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    /// 构建 pipeline 命令字符串（用于调试）
    pub fn to_command_string(&self) -> String {
        self.stages
            .iter()
            .map(|s| {
                let args = s.config.args.join(" ");
                if args.is_empty() {
                    s.config.program.clone()
                } else {
                    format!("{} {}", s.config.program, args)
                }
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Pipeline 执行器
///
/// 执行 pipeline 中的命令，将前一个命令的输出传递给下一个命令
pub struct PipelineExecutor;

impl PipelineExecutor {
    /// 执行 pipeline
    ///
    /// 依次执行每个阶段的命令，将前一个阶段的 stdout 作为下一个阶段的 stdin
    pub fn execute(pipeline: &Pipeline) -> Result<Output, ExecuteError> {
        if pipeline.is_empty() {
            return Err(ExecuteError::Io(std::io::Error::other("pipeline is empty")));
        }

        let stages = pipeline.stages();
        let mut last_output: Option<Output> = None;

        for (i, stage) in stages.iter().enumerate() {
            let is_first = i == 0;
            let _is_last = i == stages.len() - 1;

            // 构建命令
            let mut cmd = std::process::Command::new(&stage.config.program);
            cmd.args(&stage.config.args);

            // 设置工作目录
            if let Some(ref dir) = stage.config.working_dir {
                cmd.current_dir(dir);
            }

            // 如果不是第一个阶段，且不是忽略输入的阶段，将前一个输出作为输入
            if !is_first && !stage.ignore_input && last_output.is_some() {
                cmd.stdin(std::process::Stdio::piped());
            }

            // 捕获输出（除了最后一个阶段可选）
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            // 启动进程
            let mut child = cmd.spawn().map_err(ExecuteError::Io)?;

            // 如果不是第一个阶段，写入前一个阶段的输出
            if !is_first
                && !stage.ignore_input
                && let Some(ref prev_output) = last_output
                && let Some(mut stdin) = child.stdin.take()
            {
                use std::io::Write;
                stdin
                    .write_all(&prev_output.stdout)
                    .map_err(ExecuteError::Io)?;
                // 必须关闭 stdin，否则子进程会一直等待输入
                drop(stdin);
            }

            // 等待进程完成
            let output = child.wait_with_output().map_err(ExecuteError::Io)?;

            // 检查是否成功
            if !output.status.success() {
                return Ok(output);
            }

            last_output = Some(output);
        }

        // 返回最后一个阶段的输出
        last_output
            .ok_or_else(|| ExecuteError::Io(std::io::Error::other("pipeline execution failed")))
    }

    /// 异步执行 pipeline（在单独线程中）
    pub fn execute_async(
        pipeline: Pipeline,
    ) -> std::thread::JoinHandle<Result<Output, ExecuteError>> {
        std::thread::spawn(move || Self::execute(&pipeline))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_builder_works() {
        let pipeline = Pipeline::new()
            .pipe(CommandConfig::new("echo", vec!["hello".to_string()]))
            .pipe(CommandConfig::new(
                "tr",
                vec!["a-z".to_string(), "A-Z".to_string()],
            ));

        assert_eq!(pipeline.len(), 2);
        assert!(!pipeline.is_empty());
    }

    #[test]
    fn pipeline_to_command_string() {
        let pipeline = Pipeline::new()
            .pipe(CommandConfig::new("echo", vec!["hello".to_string()]))
            .pipe(CommandConfig::new("cat", vec![]));

        let cmd_str = pipeline.to_command_string();
        assert!(cmd_str.contains("echo hello"));
        assert!(cmd_str.contains("|"));
        assert!(cmd_str.contains("cat"));
    }

    #[test]
    fn pipeline_stage_ignore_input() {
        let stage = PipelineStage::new(CommandConfig::new("echo", vec!["test".to_string()]))
            .ignore_input(true);

        assert!(stage.ignore_input);
    }

    #[test]
    fn pipeline_executor_empty_pipeline_fails() {
        let pipeline = Pipeline::new();
        let result = PipelineExecutor::execute(&pipeline);
        assert!(result.is_err());
    }

    #[test]
    fn pipeline_executor_single_stage() {
        let pipeline = Pipeline::new().pipe(CommandConfig::new("echo", vec!["hello".to_string()]));

        let result = PipelineExecutor::execute(&pipeline);
        assert!(result.is_ok());

        let output = result.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello"));
    }

    #[test]
    fn pipeline_executor_two_stages() {
        // echo "hello world" | tr 'a-z' 'A-Z'
        let pipeline = Pipeline::new()
            .pipe(CommandConfig::new("echo", vec!["hello world".to_string()]))
            .pipe(CommandConfig::new(
                "tr",
                vec!["a-z".to_string(), "A-Z".to_string()],
            ));

        let result = PipelineExecutor::execute(&pipeline);
        assert!(result.is_ok(), "Pipeline execution failed: {:?}", result);

        let output = result.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("HELLO WORLD"),
            "Expected 'HELLO WORLD', got '{}'",
            stdout
        );
    }

    #[test]
    fn pipeline_executor_async() {
        let pipeline = Pipeline::new().pipe(CommandConfig::new("echo", vec!["async".to_string()]));

        let handle = PipelineExecutor::execute_async(pipeline);
        let result = handle.join().unwrap();

        assert!(result.is_ok());
        let output = result.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("async"));
    }
}
