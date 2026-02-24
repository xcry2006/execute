/// 任务取消机制演示
///
/// 此示例展示如何使用 TaskHandle、CancellationToken 和 TaskState
/// 来管理和取消任务执行。
use execute::{CancellationToken, TaskHandle, TaskState};
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== 任务取消机制演示 ===\n");

    // 示例 1: 创建和检查取消令牌
    println!("1. 创建取消令牌:");
    let cancel_token = CancellationToken::new();
    println!(
        "   初始状态: is_cancelled = {}",
        cancel_token.is_cancelled()
    );

    cancel_token.cancel();
    println!(
        "   调用 cancel() 后: is_cancelled = {}",
        cancel_token.is_cancelled()
    );
    println!();

    // 示例 2: 取消令牌在多个线程间共享
    println!("2. 取消令牌在多个线程间共享:");
    let cancel_token = CancellationToken::new();
    let token_clone = cancel_token.clone();

    let handle = thread::spawn(move || {
        println!("   工作线程: 开始执行任务...");
        for i in 0..10 {
            if token_clone.is_cancelled() {
                println!("   工作线程: 检测到取消信号，停止执行");
                return;
            }
            println!("   工作线程: 执行步骤 {}", i + 1);
            thread::sleep(Duration::from_millis(100));
        }
        println!("   工作线程: 任务完成");
    });

    // 主线程等待一段时间后取消任务
    thread::sleep(Duration::from_millis(350));
    println!("   主线程: 发送取消信号");
    cancel_token.cancel();

    handle.join().unwrap();
    println!();

    // 示例 3: TaskHandle 和 TaskState
    println!("3. TaskHandle 和 TaskState:");
    let (task_handle, result_sender) = TaskHandle::new(1);

    println!("   初始状态: {:?}", task_handle.state());
    println!("   任务 ID: {}", task_handle.id());
    println!("   是否取消: {}", task_handle.is_cancelled());
    println!();

    // 模拟任务状态变化
    println!("   模拟任务执行流程:");
    task_handle.set_state(TaskState::Running { pid: Some(12345) });
    println!("   -> 状态更新为: {:?}", task_handle.state());

    thread::sleep(Duration::from_millis(100));

    task_handle.set_state(TaskState::Completed);
    println!("   -> 状态更新为: {:?}", task_handle.state());
    println!();

    // 示例 4: 克隆 TaskHandle 共享状态
    println!("4. 克隆 TaskHandle 共享状态:");
    let (task_handle, _sender) = TaskHandle::new(2);
    let handle_clone = task_handle.clone();

    println!("   原始句柄状态: {:?}", task_handle.state());
    println!("   克隆句柄状态: {:?}", handle_clone.state());

    task_handle.set_state(TaskState::Running { pid: Some(67890) });
    println!("   更新原始句柄状态后:");
    println!("   原始句柄状态: {:?}", task_handle.state());
    println!("   克隆句柄状态: {:?}", handle_clone.state());

    task_handle.cancel_token().cancel();
    println!("   取消原始句柄后:");
    println!("   原始句柄 is_cancelled: {}", task_handle.is_cancelled());
    println!("   克隆句柄 is_cancelled: {}", handle_clone.is_cancelled());
    println!();

    // 示例 5: 使用 cancel() 方法取消任务
    println!("5. 使用 cancel() 方法取消任务:");

    // 取消队列中的任务
    println!("   a) 取消队列中的任务:");
    let (handle, _sender) = TaskHandle::new(3);
    println!("      初始状态: {:?}", handle.state());

    match handle.cancel() {
        Ok(_) => println!("      取消成功"),
        Err(e) => println!("      取消失败: {}", e),
    }
    println!("      取消后状态: {:?}", handle.state());
    println!("      is_cancelled: {}", handle.is_cancelled());
    println!();

    // 尝试取消已完成的任务
    println!("   b) 尝试取消已完成的任务:");
    let (handle, _sender) = TaskHandle::new(4);
    handle.set_state(TaskState::Completed);
    println!("      当前状态: {:?}", handle.state());

    match handle.cancel() {
        Ok(_) => println!("      取消成功"),
        Err(e) => println!("      取消失败: {}", e),
    }
    println!();

    // 尝试重复取消
    println!("   c) 尝试重复取消:");
    let (handle, _sender) = TaskHandle::new(5);
    println!("      第一次取消:");
    match handle.cancel() {
        Ok(_) => println!("        成功"),
        Err(e) => println!("        失败: {}", e),
    }

    println!("      第二次取消:");
    match handle.cancel() {
        Ok(_) => println!("        成功"),
        Err(e) => println!("        失败: {}", e),
    }
    println!();

    // 取消正在运行的任务（模拟）
    #[cfg(unix)]
    {
        println!("   d) 取消正在运行的任务:");
        use std::process::Command;

        // 启动一个长时间运行的进程
        let mut child = Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("Failed to spawn sleep process");

        let pid = child.id();
        println!("      启动进程 PID: {}", pid);

        let (handle, _sender) = TaskHandle::new(6);
        handle.set_state(TaskState::Running { pid: Some(pid) });
        println!("      任务状态: {:?}", handle.state());

        match handle.cancel() {
            Ok(_) => println!("      取消成功，进程已终止"),
            Err(e) => println!("      取消失败: {}", e),
        }

        // 验证进程已被终止
        thread::sleep(Duration::from_millis(200));
        match child.try_wait() {
            Ok(Some(status)) => println!("      进程已退出，状态: {:?}", status),
            Ok(None) => println!("      进程仍在运行"),
            Err(e) => println!("      检查进程状态失败: {}", e),
        }
        println!();
    }

    // 示例 6: TaskState 的不同状态
    println!("6. TaskState 的不同状态:");
    let states = vec![
        TaskState::Queued,
        TaskState::Running { pid: None },
        TaskState::Running { pid: Some(123) },
        TaskState::Completed,
        TaskState::Cancelled,
    ];

    for state in states {
        println!("   {:?}", state);
    }
    println!();

    println!("=== 演示完成 ===");

    // 清理未使用的 sender
    drop(result_sender);
}
