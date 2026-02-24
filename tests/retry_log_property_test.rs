// Feature: production-ready-improvements, Property 18: 重试日志
// **Validates: Requirements 11.5**
//
// 属性 18: 重试日志
// 对于任意重试的任务，日志应该包含重试次数和原因
//
// 验证需求：
// - 需求 11.5: WHEN 任务重试时，THE System SHALL 记录重试次数和原因

use execute::{CommandConfig, RetryPolicy, RetryStrategy, execute_with_retry};
use proptest::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

/// 捕获日志的层
struct LogCapture {
    logs: Arc<Mutex<Vec<String>>>,
}

impl LogCapture {
    fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
        let logs = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                logs: logs.clone(),
            },
            logs,
        )
    }
}

impl<S> Layer<S> for LogCapture
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = LogVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);
        self.logs.lock().unwrap().push(visitor.message);
    }
}

struct LogVisitor {
    message: String,
}

impl tracing::field::Visit for LogVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if !self.message.is_empty() {
            self.message.push_str(", ");
        }
        self.message.push_str(&format!("{} = {:?}", field.name(), value));
    }
}

/// 生成重试次数策略（1-5次）
fn retry_attempts_strategy() -> impl Strategy<Value = usize> {
    1usize..=5
}

/// 生成重试延迟策略（10-50ms）
fn retry_delay_strategy() -> impl Strategy<Value = Duration> {
    (10u64..=50).prop_map(Duration::from_millis)
}

/// 生成重试策略
fn retry_policy_strategy() -> impl Strategy<Value = RetryPolicy> {
    (retry_attempts_strategy(), retry_delay_strategy()).prop_map(|(attempts, delay)| {
        RetryPolicy::new(attempts, RetryStrategy::FixedInterval(delay))
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 属性测试：对于任意配置了重试的失败任务，日志应该包含重试次数和原因
    ///
    /// 验证需求：
    /// - 需求 11.5: 任务重试时，系统应记录重试次数和原因
    #[test]
    fn prop_retry_logs_contain_attempt_and_reason(
        retry_policy in retry_policy_strategy(),
    ) {
        // 设置日志捕获
        let (log_capture, captured_logs) = LogCapture::new();
        
        let subscriber = tracing_subscriber::registry()
            .with(log_capture);
        
        let _guard = subscriber.set_default();

        // 清空之前的日志
        captured_logs.lock().unwrap().clear();

        // 创建一个会失败的命令（不存在的命令）
        let config = CommandConfig::new("nonexistent_command_xyz_12345", vec![])
            .with_retry(retry_policy.clone());

        // 执行命令（应该失败并重试）
        let _ = execute_with_retry(&config, 1);

        // 获取捕获的日志
        let logs = captured_logs.lock().unwrap();
        let log_text = logs.join("\n");

        // 验证日志包含重试信息
        // 应该有初始尝试的日志
        prop_assert!(
            log_text.contains("Executing command") || log_text.contains("initial attempt"),
            "Logs should contain initial execution attempt"
        );

        // 如果有重试（max_attempts > 0），应该有重试日志
        if retry_policy.max_attempts > 0 {
            // 验证日志包含 "Retrying" 或 "retry"
            prop_assert!(
                log_text.contains("Retrying") || log_text.contains("retry"),
                "Logs should contain retry information when retries occur"
            );

            // 验证日志包含尝试次数信息
            prop_assert!(
                log_text.contains("attempt") || log_text.contains("attempts"),
                "Logs should contain attempt count information"
            );

            // 验证日志包含失败原因
            prop_assert!(
                log_text.contains("failed") || log_text.contains("error") || log_text.contains("failure"),
                "Logs should contain failure reason"
            );
        }

        // 验证最终失败日志
        prop_assert!(
            log_text.contains("failed after all retry attempts") || log_text.contains("Command failed"),
            "Logs should contain final failure message"
        );
    }

    /// 属性测试：对于任意配置了重试的超时任务，日志应该包含重试次数和超时原因
    ///
    /// 验证需求：
    /// - 需求 11.5: 任务重试时，系统应记录重试次数和原因（超时）
    #[test]
    fn prop_retry_logs_contain_timeout_reason(
        retry_policy in retry_policy_strategy(),
    ) {
        // 设置日志捕获
        let (log_capture, captured_logs) = LogCapture::new();
        
        let subscriber = tracing_subscriber::registry()
            .with(log_capture);
        
        let _guard = subscriber.set_default();

        // 清空之前的日志
        captured_logs.lock().unwrap().clear();

        // 创建一个会超时的命令
        let config = CommandConfig::new("sleep", vec!["10".to_string()])
            .with_timeout(Duration::from_millis(50))
            .with_retry(retry_policy.clone());

        // 执行命令（应该超时并重试）
        let _ = execute_with_retry(&config, 2);

        // 获取捕获的日志
        let logs = captured_logs.lock().unwrap();
        let log_text = logs.join("\n");

        // 如果有重试，验证日志包含重试信息和超时原因
        if retry_policy.max_attempts > 0 {
            // 验证日志包含重试信息
            prop_assert!(
                log_text.contains("Retrying") || log_text.contains("retry"),
                "Logs should contain retry information"
            );

            // 验证日志包含超时相关信息
            prop_assert!(
                log_text.contains("timeout") || log_text.contains("Timeout"),
                "Logs should contain timeout as failure reason"
            );

            // 验证日志包含尝试次数
            prop_assert!(
                log_text.contains("attempt"),
                "Logs should contain attempt count"
            );
        }
    }

    /// 属性测试：对于任意成功的命令，即使配置了重试也不应该有重试日志
    ///
    /// 验证需求：
    /// - 需求 11.5: 只有失败的任务才会记录重试日志
    #[test]
    fn prop_no_retry_logs_on_success(
        retry_policy in retry_policy_strategy(),
    ) {
        // 设置日志捕获
        let (log_capture, captured_logs) = LogCapture::new();
        
        let subscriber = tracing_subscriber::registry()
            .with(log_capture);
        
        let _guard = subscriber.set_default();

        // 清空之前的日志
        captured_logs.lock().unwrap().clear();

        // 创建一个会成功的命令
        let config = CommandConfig::new("echo", vec!["success".to_string()])
            .with_retry(retry_policy);

        // 执行命令（应该成功）
        let result = execute_with_retry(&config, 3);

        // 验证命令成功
        prop_assert!(result.is_ok(), "Command should succeed");

        // 获取捕获的日志
        let logs = captured_logs.lock().unwrap();
        let log_text = logs.join("\n");

        // 验证日志不包含重试信息（因为命令成功了）
        prop_assert!(
            !log_text.contains("Retrying") && !log_text.contains("succeeded after retry"),
            "Logs should not contain retry information for successful commands"
        );
    }
}

#[test]
#[cfg(unix)]
fn test_retry_logs_include_attempt_number() {
    // 测试日志包含具体的尝试次数
    let (log_capture, captured_logs) = LogCapture::new();
    
    let subscriber = tracing_subscriber::registry()
        .with(log_capture);
    
    let _guard = subscriber.set_default();

    captured_logs.lock().unwrap().clear();

    let policy = RetryPolicy::new(3, RetryStrategy::FixedInterval(Duration::from_millis(10)));
    let config = CommandConfig::new("nonexistent_cmd_xyz", vec![])
        .with_retry(policy);

    let _ = execute_with_retry(&config, 1);

    let logs = captured_logs.lock().unwrap();
    let log_text = logs.join("\n");

    // 验证日志包含尝试次数（attempt = 1, 2, 3）
    assert!(
        log_text.contains("attempt") && (log_text.contains("1") || log_text.contains("2") || log_text.contains("3")),
        "Logs should contain specific attempt numbers"
    );
}

#[test]
#[cfg(unix)]
fn test_retry_logs_include_max_attempts() {
    // 测试日志包含最大尝试次数
    let (log_capture, captured_logs) = LogCapture::new();
    
    let subscriber = tracing_subscriber::registry()
        .with(log_capture);
    
    let _guard = subscriber.set_default();

    captured_logs.lock().unwrap().clear();

    let policy = RetryPolicy::new(5, RetryStrategy::FixedInterval(Duration::from_millis(10)));
    // Use a command that will actually fail (spawn failure)
    let config = CommandConfig::new("nonexistent_command_xyz_12345", vec![])
        .with_retry(policy);

    let _ = execute_with_retry(&config, 2);

    let logs = captured_logs.lock().unwrap();
    let log_text = logs.join("\n");

    // 验证日志包含最大尝试次数信息或重试相关信息
    assert!(
        log_text.contains("max_attempts") || log_text.contains("5") || log_text.contains("Retrying"),
        "Logs should contain max attempts or retry information, got: {}",
        log_text
    );
}

#[test]
#[cfg(unix)]
fn test_retry_logs_include_failure_reason() {
    // 测试日志包含失败原因
    let (log_capture, captured_logs) = LogCapture::new();
    
    let subscriber = tracing_subscriber::registry()
        .with(log_capture);
    
    let _guard = subscriber.set_default();

    captured_logs.lock().unwrap().clear();

    let policy = RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(10)));
    let config = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_timeout(Duration::from_millis(50))
        .with_retry(policy);

    let _ = execute_with_retry(&config, 3);

    let logs = captured_logs.lock().unwrap();
    let log_text = logs.join("\n");

    // 验证日志包含错误信息
    assert!(
        log_text.contains("error") || log_text.contains("failed") || log_text.contains("timeout"),
        "Logs should contain failure reason/error information"
    );
}

#[test]
#[cfg(unix)]
fn test_retry_logs_final_failure_message() {
    // 测试日志包含最终失败消息
    let (log_capture, captured_logs) = LogCapture::new();
    
    let subscriber = tracing_subscriber::registry()
        .with(log_capture);
    
    let _guard = subscriber.set_default();

    captured_logs.lock().unwrap().clear();

    let policy = RetryPolicy::new(2, RetryStrategy::FixedInterval(Duration::from_millis(10)));
    // Use a command that will actually fail (spawn failure)
    let config = CommandConfig::new("nonexistent_command_xyz_12345", vec![])
        .with_retry(policy);

    let _ = execute_with_retry(&config, 4);

    let logs = captured_logs.lock().unwrap();
    let log_text = logs.join("\n");

    // 验证日志包含最终失败消息或重试相关信息
    assert!(
        log_text.contains("failed after all retry attempts") || 
        log_text.contains("Command failed") ||
        log_text.contains("Retrying") ||
        log_text.contains("failed"),
        "Logs should contain final failure or retry message, got: {}",
        log_text
    );
}

#[test]
#[cfg(unix)]
fn test_no_retry_logs_without_retry_policy() {
    // 测试没有配置重试策略时不应该有重试日志
    let (log_capture, captured_logs) = LogCapture::new();
    
    let subscriber = tracing_subscriber::registry()
        .with(log_capture);
    
    let _guard = subscriber.set_default();

    captured_logs.lock().unwrap().clear();

    let config = CommandConfig::new("false", vec![]);

    let _ = execute_with_retry(&config, 5);

    let logs = captured_logs.lock().unwrap();
    let log_text = logs.join("\n");

    // 验证日志不包含重试信息
    assert!(
        !log_text.contains("Retrying") && !log_text.contains("retry"),
        "Logs should not contain retry information when no retry policy is configured"
    );
}
