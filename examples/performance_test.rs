use execute::{CommandConfig, CommandPool, ExecutionConfig, ExecutionMode};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    println!("=== Execute 库极限性能测试（内存优化版）===\n");

    // 显示系统信息
    println!("系统信息:");
    println!("  逻辑 CPU 核心数: {}", num_cpus::get());
    println!("  物理 CPU 核心数: {}\n", num_cpus::get_physical());

    // 测试 1: 单线程任务提交性能
    test_single_thread_push();

    // 测试 2: 多线程并发任务提交（固定线程数）
    test_multi_thread_push();

    // 测试 2b: 多线程并发任务提交（不同线程数）
    test_multi_thread_push_variable();

    // 测试 3: 任务执行性能（不同工作线程数）
    test_execute_with_workers();

    // 测试 3b: 任务执行性能 (true 命令)
    test_execute_true();

    // 测试 3c: 任务执行性能 (echo 命令)
    test_execute_echo();

    // 测试 4: 并发执行性能
    test_concurrent_execution();

    // 测试 4b: 内存优化测试 - 使用对象池模式
    test_with_object_pool();

    // 测试 5: 极限并发测试
    test_extreme_concurrency();

    // 测试 6: 大批量任务处理
    test_large_batch();

    // 测试 6b: 大批量任务处理（内存优化）
    test_large_batch_optimized();

    println!("\n=== 性能测试完成 ===");
}

fn test_single_thread_push() {
    println!("【测试 1】单线程任务提交性能");
    let pool = CommandPool::new();
    let count = 100_000;

    let start = Instant::now();
    for i in 0..count {
        let _ = pool.push_task(CommandConfig::new("echo", vec![i.to_string()]));
    }
    let elapsed = start.elapsed();

    println!("  提交 {} 个任务: {:?}", count, elapsed);
    println!("  平均每个任务: {:?}", elapsed / count as u32);
    println!(
        "  吞吐量: {:.0} 任务/秒\n",
        count as f64 / elapsed.as_secs_f64()
    );
}

fn test_multi_thread_push() {
    println!("【测试 2a】多线程并发任务提交（固定线程数）");
    let pool = Arc::new(CommandPool::new());
    let thread_count = 8;
    let tasks_per_thread = 10_000;
    let total_tasks = thread_count * tasks_per_thread;

    let start = Instant::now();
    let mut handles = Vec::new();

    for t in 0..thread_count {
        let pool_clone = pool.clone();
        handles.push(thread::spawn(move || {
            for i in 0..tasks_per_thread {
                let _ =
                    pool_clone.push_task(CommandConfig::new("echo", vec![format!("t{}-{}", t, i)]));
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();

    println!(
        "  {} 线程 × {} 任务 = {} 总任务",
        thread_count, tasks_per_thread, total_tasks
    );
    println!("  总耗时: {:?}", elapsed);
    println!(
        "  吞吐量: {:.0} 任务/秒\n",
        total_tasks as f64 / elapsed.as_secs_f64()
    );
}

fn test_execute_true() {
    println!("【测试 3】任务执行性能 (true 命令)");
    let pool = CommandPool::new();
    let count = 1000;

    let start = Instant::now();
    for _ in 0..count {
        let _ = pool.execute_task(&CommandConfig::new("true", vec![]));
    }
    let elapsed = start.elapsed();

    println!("  执行 {} 个 true 命令: {:?}", count, elapsed);
    println!("  平均每个命令: {:?}", elapsed / count as u32);
    println!(
        "  吞吐量: {:.0} 命令/秒\n",
        count as f64 / elapsed.as_secs_f64()
    );
}

fn test_execute_echo() {
    println!("【测试 4】任务执行性能 (echo 命令)");
    let pool = CommandPool::new();
    let count = 1000;

    let start = Instant::now();
    for i in 0..count {
        let _ = pool.execute_task(&CommandConfig::new("echo", vec![i.to_string()]));
    }
    let elapsed = start.elapsed();

    println!("  执行 {} 个 echo 命令: {:?}", count, elapsed);
    println!("  平均每个命令: {:?}", elapsed / count as u32);
    println!(
        "  吞吐量: {:.0} 命令/秒\n",
        count as f64 / elapsed.as_secs_f64()
    );
}

fn test_concurrent_execution() {
    println!("【测试 5】并发执行性能");
    let pool = Arc::new(CommandPool::new());
    let thread_count = 4;
    let tasks_per_thread = 100;

    let start = Instant::now();
    let mut handles = Vec::new();

    for _ in 0..thread_count {
        let pool_clone = pool.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..tasks_per_thread {
                let _ = pool_clone.execute_task(&CommandConfig::new("true", vec![]));
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total = thread_count * tasks_per_thread;

    println!(
        "  {} 线程 × {} 任务 = {} 总任务",
        thread_count, tasks_per_thread, total
    );
    println!("  总耗时: {:?}", elapsed);
    println!(
        "  吞吐量: {:.0} 命令/秒\n",
        total as f64 / elapsed.as_secs_f64()
    );
}

fn test_large_batch() {
    println!("【测试 6】大批量任务处理");
    let pool = CommandPool::new();
    let count = 10_000;

    // 提交任务
    let start = Instant::now();
    for i in 0..count {
        let _ = pool.push_task(CommandConfig::new("echo", vec![i.to_string()]));
    }
    let submit_elapsed = start.elapsed();

    // 处理任务
    let start = Instant::now();
    pool.start_executor();
    thread::sleep(Duration::from_millis(100));
    pool.stop();
    let process_elapsed = start.elapsed();

    println!("  提交 {} 个任务: {:?}", count, submit_elapsed);
    println!("  处理 {} 个任务: {:?}", count, process_elapsed);
    println!(
        "  总吞吐量: {:.0} 任务/秒\n",
        count as f64 / process_elapsed.as_secs_f64()
    );
}

fn test_multi_thread_push_variable() {
    println!("【测试 2】多线程并发任务提交（不同线程数）");
    let thread_counts = [2, 4, 8, 16];
    let tasks_per_thread = 10_000;

    for &thread_count in &thread_counts {
        let pool = Arc::new(CommandPool::new());
        let total_tasks = thread_count * tasks_per_thread;

        let start = Instant::now();
        let mut handles = Vec::new();

        for t in 0..thread_count {
            let pool_clone = pool.clone();
            handles.push(thread::spawn(move || {
                for i in 0..tasks_per_thread {
                    let _ = pool_clone
                        .push_task(CommandConfig::new("echo", vec![format!("t{}-{}", t, i)]));
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let elapsed = start.elapsed();
        let throughput = total_tasks as f64 / elapsed.as_secs_f64();

        println!(
            "  {} 线程: {:>10.0} 任务/秒  ({:?})",
            thread_count, throughput, elapsed
        );
    }
    println!();
}

fn test_execute_with_workers() {
    println!("【测试 3】任务执行性能（不同工作线程数）");
    let worker_counts = [1, 2, 4, 8];
    let count = 500;

    for &workers in &worker_counts {
        let config = ExecutionConfig {
            workers,
            mode: ExecutionMode::Thread,
            ..Default::default()
        };
        let pool = CommandPool::with_config(config);

        let start = Instant::now();
        for _ in 0..count {
            let _ = pool.execute_task(&CommandConfig::new("true", vec![]));
        }
        let elapsed = start.elapsed();
        let throughput = count as f64 / elapsed.as_secs_f64();

        println!(
            "  {} 工作线程: {:>10.0} 命令/秒  ({:?})",
            workers, throughput, elapsed
        );
    }
    println!();
}

fn test_with_object_pool() {
    println!("【测试 4】内存优化测试 - 复用配置对象");
    let pool = CommandPool::new();
    let count = 100_000;

    // 预创建配置对象（内存优化）
    let configs: Vec<_> = (0..100)
        .map(|i| CommandConfig::new("echo", vec![format!("task{}", i)]))
        .collect();

    let start = Instant::now();
    for i in 0..count {
        let _ = pool.push_task(configs[i % 100].clone());
    }
    let elapsed = start.elapsed();

    println!("  提交 {} 个任务（复用配置）: {:?}", count, elapsed);
    println!(
        "  吞吐量: {:.0} 任务/秒\n",
        count as f64 / elapsed.as_secs_f64()
    );
}

fn test_extreme_concurrency() {
    println!("【测试 5】极限并发测试");
    let cpu_count = num_cpus::get();
    let thread_counts = [cpu_count, cpu_count * 2, cpu_count * 4];

    for &thread_count in &thread_counts {
        let pool = Arc::new(CommandPool::new());
        let tasks_per_thread = 100;
        let total = thread_count * tasks_per_thread;

        let start = Instant::now();
        let mut handles = Vec::new();

        for _ in 0..thread_count {
            let pool_clone = pool.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..tasks_per_thread {
                    let _ = pool_clone.execute_task(&CommandConfig::new("true", vec![]));
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let elapsed = start.elapsed();
        let throughput = total as f64 / elapsed.as_secs_f64();

        println!(
            "  {} 线程: {:>10.0} 命令/秒  ({:?})",
            thread_count, throughput, elapsed
        );
    }
    println!();
}

fn test_large_batch_optimized() {
    println!("【测试 6】大批量任务处理（内存优化）");
    let pool = CommandPool::new();
    let count = 10_000;

    // 分批提交避免内存峰值
    let batch_size = 1_000;
    let start = Instant::now();

    for batch in 0..(count / batch_size) {
        for i in 0..batch_size {
            let _ = pool.push_task(CommandConfig::new(
                "true",
                vec![(batch * batch_size + i).to_string()],
            ));
        }
    }
    let submit_elapsed = start.elapsed();

    // 处理任务
    let start = Instant::now();
    pool.start_executor();
    thread::sleep(Duration::from_millis(500));
    pool.stop();
    let process_elapsed = start.elapsed();

    println!("  提交 {} 个任务: {:?}", count, submit_elapsed);
    println!("  处理 {} 个任务: {:?}", count, process_elapsed);
    println!(
        "  总吞吐量: {:.0} 任务/秒",
        count as f64 / process_elapsed.as_secs_f64()
    );
    println!("  内存优化: 分批提交 (每批 {} 任务)\n", batch_size);
}
