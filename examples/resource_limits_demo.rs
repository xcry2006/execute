use execute::{CommandConfig, ResourceLimits, execute_command_with_context};
use std::time::Duration;

fn main() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== Resource Limits Demo ===\n");

    // 示例 1: 限制输出大小
    println!("1. Testing output size limit:");
    let limits = ResourceLimits::new().with_max_output_size(50); // 限制为 50 字节

    let config = CommandConfig::new(
        "echo",
        vec!["This is a long output that will be truncated".to_string()],
    )
    .with_resource_limits(limits)
    .with_timeout(Duration::from_secs(5));

    match execute_command_with_context(&config, 1) {
        Ok(output) => {
            println!("   Command executed successfully");
            println!("   Output length: {} bytes", output.stdout.len());
            println!("   Output: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("   Command failed: {}", e);
        }
    }

    println!();

    // 示例 2: 无限制的命令
    println!("2. Testing without limits:");
    let config = CommandConfig::new("echo", vec!["Hello, World!".to_string()])
        .with_timeout(Duration::from_secs(5));

    match execute_command_with_context(&config, 2) {
        Ok(output) => {
            println!("   Command executed successfully");
            println!("   Output: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("   Command failed: {}", e);
        }
    }

    println!();

    // 示例 3: 同时设置输出和内存限制
    println!("3. Testing with both output and memory limits:");
    let limits = ResourceLimits::new()
        .with_max_output_size(1024) // 1 KB
        .with_max_memory(100 * 1024 * 1024); // 100 MB

    let config = CommandConfig::new("ls", vec!["-la".to_string()])
        .with_resource_limits(limits)
        .with_timeout(Duration::from_secs(5));

    match execute_command_with_context(&config, 3) {
        Ok(output) => {
            println!("   Command executed successfully");
            println!("   Output length: {} bytes", output.stdout.len());
            println!("   Exit code: {:?}", output.status.code());
        }
        Err(e) => {
            println!("   Command failed: {}", e);
        }
    }

    println!("\n=== Demo Complete ===");
}
