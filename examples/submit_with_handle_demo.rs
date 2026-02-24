/// 演示 submit() 方法返回 TaskHandle 的功能
///
/// 此示例展示如何：
/// 1. 提交任务并获取 TaskHandle
/// 2. 使用 TaskHandle 等待任务完成
/// 3. 检查任务状态
/// 4. 取消任务
use execute::{CommandConfig, CommandPool};
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== 任务句柄演示 ===\n");

    // 示例 1: 提交任务并等待结果
    println!("1. 提交任务并等待结果:");
    {
        let pool = CommandPool::new();
        pool.start_executor();

        // 提交任务，获取句柄
        let handle = pool
            .push_task(CommandConfig::new(
                "echo",
                vec!["Hello, World!".to_string()],
            ))
            .expect("Failed to submit task");

        println!("   任务已提交，ID: {}", handle.id());
        println!("   初始状态: {:?}", handle.state());

        // 等待任务完成
        match handle.wait() {
            Ok(output) => {
                println!("   任务完成！");
                println!("   输出: {}", String::from_utf8_lossy(&output.stdout));
                println!("   最终状态: {:?}", handle.state());
            }
            Err(e) => {
                println!("   任务失败: {:?}", e);
            }
        }

        pool.shutdown().expect("Failed to shutdown");
    }
    println!();

    // 示例 2: 取消队列中的任务
    println!("2. 取消队列中的任务:");
    {
        let pool = CommandPool::new();
        // 不启动执行器，任务会保持在队列中

        // 提交一个长时间运行的任务
        let handle = pool
            .push_task(CommandConfig::new("sleep", vec!["10".to_string()]))
            .expect("Failed to submit task");

        println!("   任务已提交，ID: {}", handle.id());
        println!("   当前状态: {:?}", handle.state());

        // 取消任务
        match handle.cancel() {
            Ok(_) => {
                println!("   任务已取消");
                println!("   取消后状态: {:?}", handle.state());
                println!("   is_cancelled(): {}", handle.is_cancelled());
            }
            Err(e) => {
                println!("   取消失败: {:?}", e);
            }
        }
    }
    println!();

    // 示例 3: 多个任务并发执行
    println!("3. 多个任务并发执行:");
    {
        let pool = CommandPool::new();
        pool.start_executor();

        // 提交多个任务
        let mut handles = Vec::new();
        for i in 1..=5 {
            let handle = pool
                .push_task(CommandConfig::new("echo", vec![format!("Task {}", i)]))
                .expect("Failed to submit task");
            println!("   提交任务 {}, ID: {}", i, handle.id());
            handles.push(handle);
        }

        println!("\n   等待所有任务完成...");

        // 等待所有任务完成
        for (i, handle) in handles.into_iter().enumerate() {
            match handle.wait() {
                Ok(output) => {
                    println!(
                        "   任务 {} 完成: {}",
                        i + 1,
                        String::from_utf8_lossy(&output.stdout).trim()
                    );
                }
                Err(e) => {
                    println!("   任务 {} 失败: {:?}", i + 1, e);
                }
            }
        }

        pool.shutdown().expect("Failed to shutdown");
    }
    println!();

    // 示例 4: 检查任务状态
    println!("4. 检查任务状态:");
    {
        let pool = CommandPool::new();
        pool.start_executor();

        // 提交一个快速任务
        let handle = pool
            .push_task(CommandConfig::new("true", vec![]))
            .expect("Failed to submit task");

        println!("   任务 ID: {}", handle.id());
        println!("   初始状态: {:?}", handle.state());

        // 等待一小段时间让任务执行
        thread::sleep(Duration::from_millis(50));

        // 检查任务是否完成（非阻塞）
        match handle.is_done() {
            Ok(true) => {
                println!("   任务已完成");
                println!("   最终状态: {:?}", handle.state());
            }
            Ok(false) => {
                println!("   任务仍在执行");
            }
            Err(e) => {
                println!("   检查状态失败: {:?}", e);
            }
        }

        pool.shutdown().expect("Failed to shutdown");
    }
    println!();

    // 示例 5: 尝试获取结果（非阻塞）
    println!("5. 尝试获取结果（非阻塞）:");
    {
        let pool = CommandPool::new();
        pool.start_executor();

        let handle = pool
            .push_task(CommandConfig::new("echo", vec!["Non-blocking".to_string()]))
            .expect("Failed to submit task");

        println!("   任务 ID: {}", handle.id());

        // 尝试获取结果（非阻塞）
        for i in 0..5 {
            match handle.try_get() {
                Ok(Some(output)) => {
                    println!(
                        "   第 {} 次尝试: 任务完成，输出: {}",
                        i + 1,
                        String::from_utf8_lossy(&output.stdout).trim()
                    );
                    break;
                }
                Ok(None) => {
                    println!("   第 {} 次尝试: 任务尚未完成", i + 1);
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    println!("   第 {} 次尝试: 错误 {:?}", i + 1, e);
                    break;
                }
            }
        }

        pool.shutdown().expect("Failed to shutdown");
    }
    println!();

    println!("=== 演示完成 ===");
}
