# 轮询机制优化

## 概述

本文档描述了 CommandPool 中轮询机制的优化实现，该优化使用条件变量替代固定间隔轮询，显著降低了空闲时的 CPU 使用率。

## 问题背景

在优化之前，工作线程使用固定间隔轮询（polling）来检查任务队列：

```rust
// 旧实现（伪代码）
loop {
    if let Some(task) = queue.try_pop() {
        execute(task);
    }
    thread::sleep(interval);  // 固定间隔轮询
}
```

这种方式存在以下问题：

1. **CPU 浪费**：即使队列为空，工作线程也会定期唤醒检查
2. **延迟增加**：任务提交后需要等待下一个轮询周期才能被执行
3. **资源效率低**：多个工作线程同时轮询会造成不必要的上下文切换

## 优化方案

### 核心思想

使用条件变量（Condition Variable）实现事件驱动的任务分发：

- 当队列为空时，工作线程阻塞等待
- 当新任务提交时，通知等待的工作线程
- 当关闭时，唤醒所有等待的工作线程

### 实现细节

#### 1. 任务队列结构

```rust
pub struct CommandPool {
    // 使用 Mutex 保护队列，Condvar 用于通知
    tasks: Arc<(Mutex<VecDeque<CommandConfig>>, Condvar)>,
    // ... 其他字段
}
```

#### 2. 阻塞式 pop_task

```rust
pub fn pop_task(&self) -> Option<CommandConfig> {
    let (lock, cvar) = &*self.tasks;
    let mut tasks = lock.lock().unwrap();
    
    loop {
        // 尝试获取任务
        if let Some(task) = tasks.pop_front() {
            cvar.notify_one();  // 通知等待队列空位的线程
            return Some(task);
        }
        
        // 如果正在关闭且队列为空，返回 None
        if self.shutdown_flag.load(Ordering::SeqCst) {
            return None;
        }
        
        // 队列为空且未关闭，等待新任务
        tasks = cvar.wait(tasks).unwrap();
    }
}
```

#### 3. 通知机制

```rust
pub fn push_task(&self, task: CommandConfig) -> Result<(), SubmitError> {
    let (lock, cvar) = &*self.tasks;
    let mut tasks = lock.lock().unwrap();
    
    tasks.push_back(task);
    cvar.notify_one();  // 唤醒一个等待的工作线程
    Ok(())
}
```

#### 4. 工作线程循环

```rust
fn worker_loop(pool: CommandPool) {
    while pool.running.load(Ordering::SeqCst) && !pool.shutdown_flag.load(Ordering::SeqCst) {
        // pop_task 会阻塞等待，不需要 sleep
        if let Some(task) = pool.pop_task() {
            if !pool.running.load(Ordering::SeqCst) || pool.shutdown_flag.load(Ordering::SeqCst) {
                break;
            }
            let _ = pool.execute_task(&task);
        } else {
            // pop_task 返回 None 表示正在关闭
            break;
        }
    }
}
```

## 性能改进

### CPU 使用率

- **优化前**：空闲时 CPU 使用率约 1-5%（取决于轮询间隔和工作线程数）
- **优化后**：空闲时 CPU 使用率接近 0%

### 任务执行延迟

- **优化前**：平均延迟为轮询间隔的一半（例如 100ms 间隔 → 50ms 平均延迟）
- **优化后**：延迟接近 0（工作线程立即被唤醒）

### 响应性

- **优化前**：关闭时需要等待所有工作线程完成当前轮询周期
- **优化后**：关闭时立即唤醒所有等待的工作线程

## 验证测试

实现了以下测试来验证优化效果：

1. **test_workers_wait_efficiently_when_queue_empty**
   - 验证工作线程在队列为空时能够高效等待

2. **test_multiple_workers_wake_up_for_tasks**
   - 验证多个工作线程能够被正确唤醒并并发执行任务

3. **test_shutdown_wakes_waiting_workers**
   - 验证关闭时能够快速唤醒所有等待的工作线程

4. **test_task_execution_latency_is_low**
   - 验证任务执行延迟很低

## 兼容性

该优化完全向后兼容：

- API 没有变化
- 行为保持一致（除了性能改进）
- 所有现有测试继续通过

## 相关需求

该实现满足以下需求：

- **需求 6.1**：工作线程使用条件变量等待而不是轮询
- **需求 6.2**：新任务提交时通知等待的工作线程
- **需求 6.3**：工作线程被唤醒时检查队列并获取任务
- **需求 6.4**：保持与当前实现相同的任务执行延迟
- **需求 6.5**：减少空闲时的 CPU 使用率

## 未来改进

可能的进一步优化方向：

1. **优先级队列**：支持任务优先级，高优先级任务优先执行
2. **工作窃取**：实现工作窃取算法，提高负载均衡
3. **自适应线程池**：根据负载动态调整工作线程数量
4. **批量通知**：批量提交任务时使用 `notify_all()` 而不是多次 `notify_one()`

## 参考资料

- [Rust 标准库 Condvar 文档](https://doc.rust-lang.org/std/sync/struct.Condvar.html)
- [条件变量最佳实践](https://en.wikipedia.org/wiki/Monitor_(synchronization))
- [生产者-消费者模式](https://en.wikipedia.org/wiki/Producer%E2%80%93consumer_problem)
