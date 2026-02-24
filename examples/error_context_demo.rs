/// 错误上下文演示
///
/// 此示例展示如何使用新的 CommandError 和 ErrorContext 类型
/// 来获取更详细的错误信息。
use execute::{CommandConfig, CommandError, execute_command_with_context};
use std::time::Duration;

fn main() {
    println!("=== 错误上下文演示 ===\n");

    // 示例 1: 命令不存在 - SpawnFailed 错误
    println!("1. 尝试执行不存在的命令:");
    let config = CommandConfig::new("nonexistent_command_xyz", vec![]);
    match execute_command_with_context(&config, 1) {
        Ok(_) => println!("   命令成功执行"),
        Err(e) => {
            println!(
                "   错误类型: {}",
                match e {
                    CommandError::SpawnFailed { .. } => "SpawnFailed",
                    CommandError::ExecutionFailed { .. } => "ExecutionFailed",
                    CommandError::Timeout { .. } => "Timeout",
                }
            );
            println!("   错误详情: {}", e);
        }
    }

    println!();

    // 示例 2: 命令超时 - Timeout 错误
    #[cfg(unix)]
    {
        println!("2. 尝试执行超时的命令:");
        let config = CommandConfig::new("sleep", vec!["5".to_string()])
            .with_timeout(Duration::from_millis(100))
            .with_working_dir("/tmp");

        match execute_command_with_context(&config, 2) {
            Ok(_) => println!("   命令成功执行"),
            Err(e) => {
                println!(
                    "   错误类型: {}",
                    match e {
                        CommandError::SpawnFailed { .. } => "SpawnFailed",
                        CommandError::ExecutionFailed { .. } => "ExecutionFailed",
                        CommandError::Timeout { .. } => "Timeout",
                    }
                );
                println!("   错误详情: {}", e);

                // 提取超时信息
                if let CommandError::Timeout {
                    configured_timeout,
                    actual_duration,
                    ..
                } = e
                {
                    println!("   配置的超时: {:?}", configured_timeout);
                    println!("   实际执行时长: {:?}", actual_duration);
                }
            }
        }
    }

    println!();

    // 示例 3: 成功执行的命令
    #[cfg(unix)]
    {
        println!("3. 执行成功的命令:");
        let config = CommandConfig::new("echo", vec!["Hello, World!".to_string()]);
        match execute_command_with_context(&config, 3) {
            Ok(output) => {
                println!("   命令成功执行");
                println!("   退出码: {}", output.status.code().unwrap_or(-1));
                println!(
                    "   输出: {}",
                    String::from_utf8_lossy(&output.stdout).trim()
                );
            }
            Err(e) => println!("   错误: {}", e),
        }
    }

    println!("\n=== 演示完成 ===");
}
