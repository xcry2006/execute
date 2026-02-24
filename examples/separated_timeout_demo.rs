use execute::{CommandConfig, TimeoutConfig, execute_with_timeouts};
use std::time::Duration;

fn main() {
    println!("=== Separated Timeout Logic Demo ===\n");

    // Example 1: Command with both spawn and execution timeouts
    println!("Example 1: Command with both spawn and execution timeouts");
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(5))
        .with_execution_timeout(Duration::from_secs(10));

    let config =
        CommandConfig::new("echo", vec!["Hello, World!".to_string()]).with_timeouts(timeout_config);

    match execute_with_timeouts(&config, 1) {
        Ok(output) => {
            println!("  ✓ Command succeeded");
            println!("  Output: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("  ✗ Command failed: {}", e);
        }
    }
    println!();

    // Example 2: Command with only execution timeout
    println!("Example 2: Command with only execution timeout");
    let timeout_config = TimeoutConfig::new().with_execution_timeout(Duration::from_secs(2));

    let config =
        CommandConfig::new("echo", vec!["Quick command".to_string()]).with_timeouts(timeout_config);

    match execute_with_timeouts(&config, 2) {
        Ok(output) => {
            println!("  ✓ Command succeeded");
            println!("  Output: {}", String::from_utf8_lossy(&output.stdout));
        }
        Err(e) => {
            println!("  ✗ Command failed: {}", e);
        }
    }
    println!();

    // Example 3: Command that times out during execution
    println!("Example 3: Command that times out during execution");
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(5))
        .with_execution_timeout(Duration::from_millis(100));

    let config = CommandConfig::new("sleep", vec!["5".to_string()]).with_timeouts(timeout_config);

    let start = std::time::Instant::now();
    match execute_with_timeouts(&config, 3) {
        Ok(_) => {
            println!("  ✗ Command should have timed out");
        }
        Err(e) => {
            let elapsed = start.elapsed();
            println!("  ✓ Command timed out as expected");
            println!("  Error: {}", e);
            println!("  Elapsed time: {:?}", elapsed);
        }
    }
    println!();

    // Example 4: Demonstrating the difference between spawn and execution timeout
    println!("Example 4: Understanding spawn vs execution timeout");
    println!("  Spawn timeout: Limits the time to START the process");
    println!("  Execution timeout: Limits the TOTAL time the process runs");
    println!("  Both timeouts work independently and provide precise control");
    println!();

    // Example 5: Using separated timeouts for better error diagnosis
    println!("Example 5: Better error diagnosis with separated timeouts");
    println!("  With separated timeouts, you can distinguish between:");
    println!("  - Process creation issues (spawn timeout)");
    println!("  - Long-running process issues (execution timeout)");
    println!("  This helps identify whether the problem is:");
    println!("  - System resource constraints (spawn)");
    println!("  - Command logic or performance (execution)");
    println!();

    // Example 6: Practical use case - network command with timeouts
    println!("Example 6: Practical use case - listing files with timeout");
    let timeout_config = TimeoutConfig::new()
        .with_spawn_timeout(Duration::from_secs(2))
        .with_execution_timeout(Duration::from_secs(5));

    let config = CommandConfig::new("ls", vec!["-la".to_string()]).with_timeouts(timeout_config);

    match execute_with_timeouts(&config, 6) {
        Ok(output) => {
            println!("  ✓ Command succeeded");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let line_count = stdout.lines().count();
            println!("  Listed {} items", line_count);
        }
        Err(e) => {
            println!("  ✗ Command failed: {}", e);
        }
    }
    println!();

    println!("=== Demo Complete ===");
}
