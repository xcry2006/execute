# Separated Timeout Logic Implementation

## Overview

This document describes the implementation of task 15.3: separated timeout logic for the command execution system. The implementation provides fine-grained control over command startup and execution timeouts, enabling better error diagnosis and resource management.

## Implementation Details

### Core Function: `execute_with_timeouts()`

Located in `src/executor.rs`, this function implements separated timeout handling:

```rust
pub fn execute_with_timeouts(
    config: &CommandConfig,
    task_id: u64,
) -> Result<Output, CommandError>
```

#### Key Features

1. **Spawn Timeout**: Limits the time allowed for process creation
   - Monitors the time taken to spawn the child process
   - Returns `CommandError::Timeout` if spawn takes too long
   - Helps identify system resource constraints

2. **Execution Timeout**: Limits the total command execution time
   - Uses `wait_timeout` to monitor process execution
   - Terminates the process if execution exceeds the limit
   - Returns `CommandError::Timeout` with clear error context

3. **Fallback Behavior**: 
   - If no `TimeoutConfig` is provided, falls back to `execute_command_with_context()`
   - Maintains backward compatibility with existing code

4. **Integration with Other Features**:
   - Works with resource limits (output size, memory)
   - Integrates with memory monitoring
   - Supports retry logic when combined with `execute_with_retry()`

### Error Handling

The function returns clear error types:

- **Spawn Timeout**: When process creation exceeds `spawn_timeout`
  ```rust
  CommandError::Timeout {
      context: ErrorContext,
      configured_timeout: spawn_timeout,
      actual_duration: spawn_duration,
  }
  ```

- **Execution Timeout**: When process execution exceeds `execution_timeout`
  ```rust
  CommandError::Timeout {
      context: ErrorContext,
      configured_timeout: execution_timeout,
      actual_duration: elapsed_time,
  }
  ```

### Configuration

Timeouts are configured using `TimeoutConfig`:

```rust
let timeout_config = TimeoutConfig::new()
    .with_spawn_timeout(Duration::from_secs(5))
    .with_execution_timeout(Duration::from_secs(30));

let config = CommandConfig::new("command", vec![])
    .with_timeouts(timeout_config);
```

## Integration Points

### 1. Retry Logic Integration

The `execute_with_retry()` function now uses `execute_with_timeouts()` when a `TimeoutConfig` is present:

```rust
let execution_result = if config.timeout_config().is_some() {
    execute_with_timeouts(config, task_id)
} else {
    execute_command_with_context(config, task_id)
};
```

### 2. Public API Export

The function is exported in `src/lib.rs`:

```rust
pub use executor::{
    execute_command_with_context, 
    execute_with_retry, 
    execute_with_timeouts,
    CommandExecutor, 
    StdCommandExecutor
};
```

## Testing

### Unit Tests

Located in `tests/separated_timeout_test.rs`:

1. **test_execute_with_spawn_timeout_fast_spawn**: Verifies fast spawning doesn't trigger timeout
2. **test_execute_with_execution_timeout**: Verifies execution timeout works correctly
3. **test_execute_with_only_execution_timeout**: Tests execution-only timeout
4. **test_execute_with_only_spawn_timeout**: Tests spawn-only timeout
5. **test_execute_without_timeout_config_fallback**: Verifies fallback behavior
6. **test_execute_with_both_timeouts**: Tests both timeouts together
7. **test_execution_timeout_kills_process**: Verifies process termination on timeout

All tests pass successfully:
```
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured
```

### Example Program

Located in `examples/separated_timeout_demo.rs`, demonstrates:

1. Commands with both spawn and execution timeouts
2. Commands with only execution timeout
3. Commands that timeout during execution
4. Understanding spawn vs execution timeout differences
5. Better error diagnosis with separated timeouts
6. Practical use cases

## Benefits

### 1. Precise Control
- Separate control over process creation and execution phases
- Better resource management

### 2. Better Error Diagnosis
- Distinguish between spawn failures and execution timeouts
- Identify whether issues are system-related or command-related

### 3. Improved Reliability
- Prevent hanging on process creation
- Ensure timely termination of long-running processes

### 4. Backward Compatibility
- Existing code without `TimeoutConfig` continues to work
- Graceful fallback to standard execution

## Requirements Validation

This implementation validates the following requirements:

- **Requirement 12.1**: ✓ System supports configuring spawn timeout
- **Requirement 12.2**: ✓ System supports configuring execution timeout
- **Requirement 12.3**: ✓ Spawn timeout cancels startup and returns timeout error
- **Requirement 12.4**: ✓ Execution timeout terminates process and returns timeout error
- **Requirement 12.5**: ✓ Error messages distinguish between spawn and execution timeouts

## Design Properties

This implementation supports the following design property:

**Property 19: Timeout Type Distinction**
- For any timeout error, the system clearly distinguishes between spawn timeout and execution timeout
- Error context includes the configured timeout value and actual duration
- Validates requirements 12.3, 12.4, 12.5

## Usage Examples

### Basic Usage

```rust
use execute::{CommandConfig, TimeoutConfig, execute_with_timeouts};
use std::time::Duration;

let timeout_config = TimeoutConfig::new()
    .with_spawn_timeout(Duration::from_secs(5))
    .with_execution_timeout(Duration::from_secs(30));

let config = CommandConfig::new("my-command", vec![])
    .with_timeouts(timeout_config);

match execute_with_timeouts(&config, 1) {
    Ok(output) => println!("Success: {:?}", output),
    Err(e) => eprintln!("Error: {}", e),
}
```

### With Retry Logic

```rust
use execute::{CommandConfig, TimeoutConfig, RetryPolicy, RetryStrategy, execute_with_retry};
use std::time::Duration;

let timeout_config = TimeoutConfig::new()
    .with_execution_timeout(Duration::from_secs(10));

let retry_policy = RetryPolicy::new(
    3,
    RetryStrategy::FixedInterval(Duration::from_secs(1))
);

let config = CommandConfig::new("flaky-command", vec![])
    .with_timeouts(timeout_config)
    .with_retry(retry_policy);

let result = execute_with_retry(&config, 1);
```

## Future Enhancements

Potential improvements for future iterations:

1. **Async Support**: Add async version using tokio timeouts
2. **Metrics Integration**: Track spawn and execution timeout rates separately
3. **Adaptive Timeouts**: Automatically adjust timeouts based on historical data
4. **Platform-Specific Optimizations**: Better spawn timeout detection on different platforms

## Conclusion

The separated timeout logic implementation provides precise control over command execution phases, enabling better error diagnosis and resource management. The implementation is well-tested, documented, and maintains backward compatibility with existing code.
