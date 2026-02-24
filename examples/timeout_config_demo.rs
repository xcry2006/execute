use execute::{CommandConfig, TimeoutConfig, TimeoutError};
use std::time::Duration;

fn main() {
    println!("=== TimeoutConfig Demo ===\n");

    // Example 1: Create a TimeoutConfig with both spawn and execution timeouts
    println!("Example 1: TimeoutConfig with both timeouts");
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(5))
        .with_execution_timeout(Duration::from_secs(30));

    println!("  Spawn timeout: {:?}", timeout_config.spawn_timeout());
    println!(
        "  Execution timeout: {:?}",
        timeout_config.execution_timeout()
    );
    println!();

    // Example 2: Create a TimeoutConfig with only execution timeout
    println!("Example 2: TimeoutConfig with only execution timeout");
    let timeout_config = TimeoutConfig::new().with_execution_timeout(Duration::from_secs(60));

    println!("  Spawn timeout: {:?}", timeout_config.spawn_timeout());
    println!(
        "  Execution timeout: {:?}",
        timeout_config.execution_timeout()
    );
    println!();

    // Example 3: Use TimeoutConfig with CommandConfig
    println!("Example 3: CommandConfig with TimeoutConfig");
    let cmd = CommandConfig::new("sleep", vec!["10".to_string()])
        .with_working_dir("/tmp")
        .with_timeouts(
            TimeoutConfig::new()
                .with_spawn_timeout(Duration::from_secs(2))
                .with_execution_timeout(Duration::from_secs(15)),
        );

    if let Some(config) = cmd.timeout_config() {
        println!("  Command: {} {:?}", cmd.program(), cmd.args());
        println!("  Working dir: {:?}", cmd.working_dir());
        println!("  Spawn timeout: {:?}", config.spawn_timeout());
        println!("  Execution timeout: {:?}", config.execution_timeout());
    }
    println!();

    // Example 4: TimeoutError types
    println!("Example 4: TimeoutError types");
    let spawn_error = TimeoutError::SpawnTimeout(Duration::from_secs(5));
    let exec_error = TimeoutError::ExecutionTimeout(Duration::from_secs(30));

    println!("  Spawn timeout error: {}", spawn_error);
    println!("  Execution timeout error: {}", exec_error);
    println!();

    // Example 5: Comparing timeout errors
    println!("Example 5: Comparing timeout errors");
    let error1 = TimeoutError::SpawnTimeout(Duration::from_secs(5));
    let error2 = TimeoutError::SpawnTimeout(Duration::from_secs(5));
    let error3 = TimeoutError::ExecutionTimeout(Duration::from_secs(5));

    println!("  error1 == error2: {}", error1 == error2);
    println!("  error1 == error3: {}", error1 == error3);
    println!();

    println!("=== Demo Complete ===");
}
