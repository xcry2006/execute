use execute::{CommandConfig, CommandPool, ExecutionConfig};
use std::time::Duration;

#[test]
fn test_command_pool_with_zombie_reaper() {
    // 创建带有僵尸进程清理器的命令池
    let config = ExecutionConfig::new()
        .with_workers(2)
        .with_zombie_reaper_interval(Duration::from_millis(100));

    let pool = CommandPool::with_config(config);

    // 启动执行器
    pool.start_executor();

    // 提交一些任务
    for i in 0..5 {
        let task = CommandConfig::new("echo", vec![format!("test {}", i)]);
        pool.push_task(task).unwrap();
    }

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(1));

    // 优雅关闭
    pool.shutdown().unwrap();
}

#[test]
fn test_command_pool_without_zombie_reaper() {
    // 创建不带僵尸进程清理器的命令池（默认）
    let pool = CommandPool::new();

    // 启动执行器
    pool.start_executor();

    // 提交一些任务
    for i in 0..5 {
        let task = CommandConfig::new("echo", vec![format!("test {}", i)]);
        pool.push_task(task).unwrap();
    }

    // 等待任务完成
    std::thread::sleep(Duration::from_secs(1));

    // 优雅关闭
    pool.shutdown().unwrap();
}

#[cfg(unix)]
#[test]
fn test_zombie_reaper_cleans_up_processes() {
    use std::process::Command;

    // 创建带有僵尸进程清理器的命令池
    let config = ExecutionConfig::new()
        .with_workers(2)
        .with_zombie_reaper_interval(Duration::from_millis(100));

    let pool = CommandPool::with_config(config);
    pool.start_executor();

    // 提交一些快速完成的任务，这些任务会产生子进程
    for i in 0..10 {
        let task = CommandConfig::new(
            "sh",
            vec!["-c".to_string(), format!("echo 'Task {}' && exit 0", i)],
        );
        pool.push_task(task).unwrap();
    }

    // 等待任务完成和清理
    std::thread::sleep(Duration::from_secs(2));

    // 检查是否有僵尸进程
    let output = Command::new("ps")
        .arg("aux")
        .output()
        .expect("Failed to execute ps");

    let output_str = String::from_utf8_lossy(&output.stdout);
    let zombie_count = output_str
        .lines()
        .filter(|line| line.contains("<defunct>") || line.contains("Z"))
        .count();

    // 僵尸进程应该被清理，数量应该很少或为 0
    // 注意：在某些系统上可能会有短暂的僵尸进程，所以我们允许少量存在
    assert!(
        zombie_count < 5,
        "Too many zombie processes: {}",
        zombie_count
    );

    // 优雅关闭
    pool.shutdown().unwrap();
}
