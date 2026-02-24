use execute::{CommandConfig, EnvConfig};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Environment Variable Configuration Demo ===\n");

    // 示例 1: 设置环境变量
    println!("1. Setting environment variables:");
    let env = EnvConfig::new()
        .set("MY_VAR", "Hello from Rust!")
        .set("ANOTHER_VAR", "42");

    let config = CommandConfig::new(
        "sh",
        vec![
            "-c".to_string(),
            "echo MY_VAR=$MY_VAR, ANOTHER_VAR=$ANOTHER_VAR".to_string(),
        ],
    )
    .with_env(env);

    let result = execute::execute_command_with_context(&config, 1)?;
    println!("Output: {}", String::from_utf8_lossy(&result.stdout));

    // 示例 2: 清除环境变量
    println!("\n2. Removing environment variables:");
    unsafe {
        std::env::set_var("TEMP_VAR", "This will be removed");
    }

    let env = EnvConfig::new().remove("TEMP_VAR");

    let config = CommandConfig::new("sh", vec![
        "-c".to_string(),
        "if [ -z \"$TEMP_VAR\" ]; then echo 'TEMP_VAR is not set'; else echo \"TEMP_VAR=$TEMP_VAR\"; fi".to_string(),
    ])
    .with_env(env);

    let result = execute::execute_command_with_context(&config, 2)?;
    println!("Output: {}", String::from_utf8_lossy(&result.stdout));

    unsafe {
        std::env::remove_var("TEMP_VAR");
    }

    // 示例 3: 不继承父进程环境变量
    println!("\n3. Not inheriting parent environment variables:");
    let env = EnvConfig::new()
        .no_inherit()
        .set("ONLY_VAR", "I'm the only one!");

    let config = CommandConfig::new(
        "sh",
        vec![
            "-c".to_string(),
            "echo 'Environment variables:'; printenv | wc -l; echo ONLY_VAR=$ONLY_VAR".to_string(),
        ],
    )
    .with_env(env);

    let result = execute::execute_command_with_context(&config, 3)?;
    println!("Output:\n{}", String::from_utf8_lossy(&result.stdout));

    // 示例 4: 继承并覆盖环境变量
    println!("\n4. Inheriting and overriding environment variables:");
    unsafe {
        std::env::set_var("OVERRIDE_VAR", "original value");
    }

    let env = EnvConfig::new()
        .set("OVERRIDE_VAR", "overridden value")
        .set("NEW_VAR", "new value");

    let config = CommandConfig::new(
        "sh",
        vec![
            "-c".to_string(),
            "echo OVERRIDE_VAR=$OVERRIDE_VAR, NEW_VAR=$NEW_VAR".to_string(),
        ],
    )
    .with_env(env);

    let result = execute::execute_command_with_context(&config, 4)?;
    println!("Output: {}", String::from_utf8_lossy(&result.stdout));

    unsafe {
        std::env::remove_var("OVERRIDE_VAR");
    }

    // 示例 5: 使用环境变量配置 PATH
    println!("\n5. Configuring PATH environment variable:");
    let env = EnvConfig::new().set("PATH", "/usr/local/bin:/usr/bin:/bin");

    let config = CommandConfig::new("sh", vec!["-c".to_string(), "echo PATH=$PATH".to_string()])
        .with_env(env);

    let result = execute::execute_command_with_context(&config, 5)?;
    println!("Output: {}", String::from_utf8_lossy(&result.stdout));

    println!("\n=== Demo completed successfully! ===");
    Ok(())
}
