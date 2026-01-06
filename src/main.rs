use std::thread;
use std::time::Duration;

use execute::{CommandConfig, CommandPool};

/// # 程序入口
///
/// 启动一个 `CommandPool` 并启动后台执行器，然后在另一个线程中向池中推入示例任务：
/// 1. 一个短命令 `echo`；
/// 2. 带工作目录和超时配置的 `echo`；
/// 3. 一个可能超时的 `sleep`（用于演示超时处理）。
///
/// # 返回
/// - `Ok(())`：主流程正常结束。
/// - `Err(ExecuteError)`：若在主流程中遇到不可恢复的错误则返回。
fn main() -> Result<(), execute::ExecuteError> {
    println!("[main] 启动命令池并启动执行器...");
    let command_pool = CommandPool::new();
    command_pool.start_executor(Duration::from_millis(500));
    
    let pool_clone = command_pool.clone();
    thread::spawn(move || {
        println!("[producer] 向池中推入第一个任务");
        let task1 = CommandConfig::new("echo", vec!["第一次任务执行".to_string()]);
        pool_clone.push_task(task1);
        
        thread::sleep(Duration::from_secs(2));
        
        println!("[producer] 向池中推入第二个任务");
        let task2 = CommandConfig::new("echo", vec!["第二次任务执行".to_string()])
            .with_working_dir(".")
            .with_timeout(Duration::from_secs(5));
        pool_clone.push_task(task2);
        
        thread::sleep(Duration::from_secs(2));
        
        println!("[producer] 向池中推入第三个任务（会超时）");
        let task3 =
            CommandConfig::new("sleep", vec!["20".to_string()]).with_timeout(Duration::from_secs(3));
        pool_clone.push_task(task3);
    });
    
    println!("[main] 等待所有任务执行完毕...");
    thread::sleep(Duration::from_secs(15));
    println!("[main] 程序结束");
    Ok(())
}
