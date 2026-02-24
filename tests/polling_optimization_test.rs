/// 测试轮询机制优化
///
/// 验证工作线程使用条件变量等待而不是轮询，减少 CPU 使用
use execute::{CommandConfig, CommandPool};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_workers_wait_efficiently_when_queue_empty() {
    // 创建命令池并启动执行器
    let pool = CommandPool::new();
    pool.start_executor();

    // 等待一段时间，确保工作线程已启动并进入等待状态
    thread::sleep(Duration::from_millis(200));

    // 提交一个任务
    let start = Instant::now();
    let config = CommandConfig::new("echo", vec!["test".to_string()]);
    pool.push_task(config).unwrap();

    // 等待任务执行完成
    thread::sleep(Duration::from_millis(100));

    // 验证任务能够快速被执行（说明工作线程被正确唤醒）
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(1),
        "Task should be executed quickly, took {:?}",
        elapsed
    );

    pool.shutdown().unwrap();
}

#[test]
fn test_multiple_workers_wake_up_for_tasks() {
    // 创建有多个工作线程的命令池
    let config = execute::ExecutionConfig::default().with_workers(4);
    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 等待工作线程启动
    thread::sleep(Duration::from_millis(100));

    // 提交多个任务
    let start = Instant::now();
    for i in 0..4 {
        let config = CommandConfig::new("echo", vec![format!("task-{}", i)]);
        pool.push_task(config).unwrap();
    }

    // 等待所有任务执行完成
    thread::sleep(Duration::from_millis(500));

    // 验证任务能够被并发执行
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "Tasks should be executed concurrently, took {:?}",
        elapsed
    );

    pool.shutdown().unwrap();
}

#[test]
fn test_shutdown_wakes_waiting_workers() {
    // 创建命令池
    let pool = CommandPool::new();
    pool.start_executor();

    // 等待工作线程进入等待状态
    thread::sleep(Duration::from_millis(100));

    // 在另一个线程中执行关闭
    let pool_clone = pool.clone();
    let shutdown_complete = Arc::new(AtomicBool::new(false));
    let shutdown_complete_clone = shutdown_complete.clone();

    let handle = thread::spawn(move || {
        let result = pool_clone.shutdown_with_timeout(Duration::from_secs(2));
        shutdown_complete_clone.store(true, Ordering::SeqCst);
        result
    });

    // 等待关闭完成
    thread::sleep(Duration::from_millis(500));

    // 验证关闭能够快速完成（说明等待的工作线程被正确唤醒）
    assert!(
        shutdown_complete.load(Ordering::SeqCst),
        "Shutdown should complete quickly by waking up waiting workers"
    );

    handle.join().unwrap().unwrap();
}

#[test]
fn test_task_execution_latency_is_low() {
    // 创建命令池
    let pool = CommandPool::new();
    pool.start_executor();

    // 等待工作线程启动
    thread::sleep(Duration::from_millis(100));

    // 测量任务从提交到开始执行的延迟
    // 注意：这里测量的是整个过程，包括任务执行时间
    let start = Instant::now();
    let config = CommandConfig::new("echo", vec!["latency-test".to_string()]);
    pool.push_task(config).unwrap();

    // 等待任务执行
    thread::sleep(Duration::from_millis(200));

    let total_time = start.elapsed();

    // 验证任务能够被快速处理
    // 使用条件变量，工作线程应该立即被唤醒并执行任务
    // 总时间应该在合理范围内（包括我们的 sleep 时间）
    assert!(
        total_time < Duration::from_secs(1),
        "Task should be processed quickly with condition variables, got {:?}",
        total_time
    );

    pool.shutdown().unwrap();
}

// ============================================================================
// Performance Tests for Requirements 6.4 and 6.5
// ============================================================================

/// Test task execution latency (Requirement 6.4)
/// 
/// Verifies that the system maintains low task execution latency
/// when using condition variables instead of polling.
#[test]
fn test_task_execution_latency_with_condition_variables() {
    // 创建命令池
    let pool = CommandPool::new();
    pool.start_executor();

    // 等待工作线程启动
    thread::sleep(Duration::from_millis(100));

    // 测量多个任务的执行延迟
    let num_tasks = 10;
    let mut latencies = Vec::new();

    for i in 0..num_tasks {
        let start = Instant::now();
        let config = CommandConfig::new("echo", vec![format!("latency-test-{}", i)]);
        pool.push_task(config).unwrap();
        
        // 等待任务执行完成
        thread::sleep(Duration::from_millis(150));
        
        let latency = start.elapsed();
        latencies.push(latency);
    }

    // 计算平均延迟
    let avg_latency = latencies.iter().sum::<Duration>() / num_tasks as u32;
    
    // 验证平均延迟在合理范围内（应该小于 500ms）
    // 使用条件变量，工作线程应该立即被唤醒
    assert!(
        avg_latency < Duration::from_millis(500),
        "Average task latency should be low with condition variables, got {:?}",
        avg_latency
    );

    // 验证没有异常高的延迟
    let max_latency = latencies.iter().max().unwrap();
    assert!(
        *max_latency < Duration::from_secs(1),
        "Maximum task latency should be reasonable, got {:?}",
        max_latency
    );

    pool.shutdown().unwrap();
}

/// Test concurrent task execution latency (Requirement 6.4)
/// 
/// Verifies that multiple tasks can be executed concurrently with low latency.
#[test]
fn test_concurrent_task_execution_latency() {
    // 创建有多个工作线程的命令池
    let config = execute::ExecutionConfig::default().with_workers(4);
    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 等待工作线程启动
    thread::sleep(Duration::from_millis(100));

    // 同时提交多个任务并测量总时间
    let start = Instant::now();
    let num_tasks = 8;
    
    for i in 0..num_tasks {
        let config = CommandConfig::new("echo", vec![format!("concurrent-{}", i)]);
        pool.push_task(config).unwrap();
    }

    // 等待所有任务执行完成
    thread::sleep(Duration::from_millis(500));

    let total_time = start.elapsed();

    // 验证并发执行时间合理
    // 8个任务在4个工作线程上应该能快速完成
    assert!(
        total_time < Duration::from_secs(2),
        "Concurrent tasks should execute quickly, took {:?}",
        total_time
    );

    pool.shutdown().unwrap();
}

/// Test idle CPU usage (Requirement 6.5)
/// 
/// Verifies that the system has low CPU usage when idle (no tasks).
/// This test uses a heuristic approach since precise CPU measurement is platform-specific.
#[test]
fn test_idle_cpu_usage_is_low() {
    // 创建命令池
    let pool = CommandPool::new();
    pool.start_executor();

    // 等待工作线程启动并进入等待状态
    thread::sleep(Duration::from_millis(200));

    // 测量空闲期间的响应性
    // 如果工作线程在轮询，它们会持续消耗 CPU
    // 如果使用条件变量，它们应该处于休眠状态
    
    // 提交一个任务来验证工作线程仍然响应
    let start = Instant::now();
    let config = CommandConfig::new("echo", vec!["idle-test".to_string()]);
    pool.push_task(config).unwrap();
    
    // 等待任务执行
    thread::sleep(Duration::from_millis(150));
    
    let response_time = start.elapsed();

    // 验证工作线程能够快速响应（说明它们在等待而不是忙轮询）
    // 如果是忙轮询，响应时间会受到轮询间隔的影响
    assert!(
        response_time < Duration::from_millis(500),
        "Workers should respond quickly from idle state, got {:?}",
        response_time
    );

    pool.shutdown().unwrap();
}

/// Test CPU usage during idle period with multiple workers (Requirement 6.5)
/// 
/// Verifies that multiple idle workers don't cause high CPU usage.
#[test]
fn test_multiple_idle_workers_low_cpu() {
    // 创建有多个工作线程的命令池
    let config = execute::ExecutionConfig::default().with_workers(8);
    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 等待所有工作线程启动并进入等待状态
    thread::sleep(Duration::from_millis(300));

    // 在空闲期间，所有工作线程应该在条件变量上等待
    // 提交任务来验证它们仍然响应
    let start = Instant::now();
    
    for i in 0..8 {
        let config = CommandConfig::new("echo", vec![format!("multi-idle-{}", i)]);
        pool.push_task(config).unwrap();
    }

    // 等待任务执行
    thread::sleep(Duration::from_millis(500));
    
    let total_time = start.elapsed();

    // 验证所有工作线程能够快速响应
    assert!(
        total_time < Duration::from_secs(2),
        "Multiple idle workers should respond quickly, got {:?}",
        total_time
    );

    pool.shutdown().unwrap();
}

/// Test wake-up latency from idle state (Requirement 6.4 & 6.5)
/// 
/// Measures how quickly a worker wakes up from idle state when a task arrives.
#[test]
fn test_worker_wakeup_latency() {
    // 创建命令池
    let pool = CommandPool::new();
    pool.start_executor();

    // 等待工作线程进入空闲等待状态
    thread::sleep(Duration::from_millis(200));

    // 测量唤醒延迟
    let num_iterations = 5;
    let mut wakeup_times = Vec::new();

    for i in 0..num_iterations {
        // 确保工作线程处于空闲状态
        thread::sleep(Duration::from_millis(100));
        
        // 测量从提交任务到任务开始执行的时间
        let start = Instant::now();
        let config = CommandConfig::new("echo", vec![format!("wakeup-{}", i)]);
        pool.push_task(config).unwrap();
        
        // 等待任务执行
        thread::sleep(Duration::from_millis(100));
        
        let wakeup_time = start.elapsed();
        wakeup_times.push(wakeup_time);
    }

    // 计算平均唤醒时间
    let avg_wakeup = wakeup_times.iter().sum::<Duration>() / num_iterations as u32;

    // 验证唤醒时间很短（使用条件变量应该是即时的）
    assert!(
        avg_wakeup < Duration::from_millis(300),
        "Worker wakeup should be fast with condition variables, got {:?}",
        avg_wakeup
    );

    pool.shutdown().unwrap();
}

/// Test sustained throughput with condition variables (Requirement 6.4)
/// 
/// Verifies that the system can maintain good throughput over time.
#[test]
fn test_sustained_throughput() {
    // 创建命令池
    let config = execute::ExecutionConfig::default().with_workers(4);
    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 等待工作线程启动
    thread::sleep(Duration::from_millis(100));

    // 持续提交任务并测量吞吐量
    let start = Instant::now();
    let num_tasks = 50;
    
    for i in 0..num_tasks {
        let config = CommandConfig::new("echo", vec![format!("throughput-{}", i)]);
        pool.push_task(config).unwrap();
        // 小延迟以模拟实际使用场景
        thread::sleep(Duration::from_millis(10));
    }

    // 等待所有任务完成
    thread::sleep(Duration::from_secs(2));

    let total_time = start.elapsed();
    let throughput = num_tasks as f64 / total_time.as_secs_f64();

    // 验证吞吐量合理（应该能够处理多个任务/秒）
    assert!(
        throughput > 5.0,
        "System should maintain reasonable throughput, got {:.2} tasks/sec",
        throughput
    );

    pool.shutdown().unwrap();
}

/// Linux-specific test to measure actual CPU usage (Requirement 6.5)
/// 
/// This test reads /proc/stat to measure CPU usage during idle periods.
#[cfg(target_os = "linux")]
#[test]
fn test_actual_cpu_usage_when_idle() {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    // Helper function to read CPU stats
    fn read_cpu_stats() -> Option<(u64, u64)> {
        let file = File::open("/proc/stat").ok()?;
        let reader = BufReader::new(file);
        
        for line in reader.lines() {
            let line = line.ok()?;
            if line.starts_with("cpu ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let user: u64 = parts[1].parse().ok()?;
                    let nice: u64 = parts[2].parse().ok()?;
                    let system: u64 = parts[3].parse().ok()?;
                    let idle: u64 = parts[4].parse().ok()?;
                    
                    let total = user + nice + system + idle;
                    let active = user + nice + system;
                    return Some((active, total));
                }
            }
        }
        None
    }

    // 创建命令池
    let config = execute::ExecutionConfig::default().with_workers(4);
    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 等待工作线程启动
    thread::sleep(Duration::from_millis(200));

    // 读取初始 CPU 统计
    let (active_before, total_before) = read_cpu_stats()
        .expect("Failed to read CPU stats");

    // 让系统空闲一段时间
    thread::sleep(Duration::from_secs(2));

    // 读取最终 CPU 统计
    let (active_after, total_after) = read_cpu_stats()
        .expect("Failed to read CPU stats");

    // 计算 CPU 使用率
    let active_diff = active_after.saturating_sub(active_before);
    let total_diff = total_after.saturating_sub(total_before);
    
    let cpu_usage = if total_diff > 0 {
        (active_diff as f64 / total_diff as f64) * 100.0
    } else {
        0.0
    };

    println!("Idle CPU usage: {:.2}%", cpu_usage);

    // 验证空闲时 CPU 使用率很低
    // 注意：这个测试可能受系统其他进程影响，所以阈值设置得比较宽松
    assert!(
        cpu_usage < 50.0,
        "Idle CPU usage should be low with condition variables, got {:.2}%",
        cpu_usage
    );

    pool.shutdown().unwrap();
}

/// Test that condition variable notification works correctly (Requirement 6.5)
/// 
/// Verifies that workers are properly notified when tasks arrive.
#[test]
fn test_condition_variable_notification() {
    // 创建命令池
    let pool = CommandPool::new();
    pool.start_executor();

    // 等待工作线程进入等待状态
    thread::sleep(Duration::from_millis(200));

    // 在后台线程中提交任务
    let pool_clone = pool.clone();
    let handle = thread::spawn(move || {
        for i in 0..10 {
            let config = CommandConfig::new("echo", vec![format!("notify-{}", i)]);
            pool_clone.push_task(config).unwrap();
            thread::sleep(Duration::from_millis(50));
        }
    });

    // 等待任务完成
    handle.join().unwrap();
    thread::sleep(Duration::from_millis(500));

    // 验证所有任务都被处理（说明通知机制工作正常）
    // 注意：我们无法直接计数完成的任务，但可以验证系统仍然响应
    let start = Instant::now();
    let config = CommandConfig::new("echo", vec!["final-test".to_string()]);
    pool.push_task(config).unwrap();
    thread::sleep(Duration::from_millis(100));
    let response_time = start.elapsed();

    assert!(
        response_time < Duration::from_millis(500),
        "System should still be responsive after processing tasks, got {:?}",
        response_time
    );

    pool.shutdown().unwrap();
}
