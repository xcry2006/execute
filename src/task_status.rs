use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// 等待中（在队列中）
    Pending,
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
        }
    }
}

/// 任务 ID 生成器
pub struct TaskIdGenerator {
    counter: AtomicU64,
}

impl TaskIdGenerator {
    /// 创建新的任务 ID 生成器
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(1),
        }
    }

    /// 生成下一个任务 ID
    pub fn next_id(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for TaskIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// 任务状态追踪器
pub struct TaskStatusTracker {
    statuses: Arc<Mutex<HashMap<u64, TaskStatus>>>,
}

impl TaskStatusTracker {
    /// 创建新的任务状态追踪器
    pub fn new() -> Self {
        Self {
            statuses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册新任务
    pub fn register(&self, task_id: u64) {
        let mut statuses = self.statuses.lock().unwrap();
        statuses.insert(task_id, TaskStatus::Pending);
    }

    /// 更新任务状态
    pub fn update(&self, task_id: u64, status: TaskStatus) {
        let mut statuses = self.statuses.lock().unwrap();
        statuses.insert(task_id, status);
    }

    /// 获取任务状态
    pub fn get(&self, task_id: u64) -> Option<TaskStatus> {
        let statuses = self.statuses.lock().unwrap();
        statuses.get(&task_id).copied()
    }

    /// 移除任务状态
    pub fn remove(&self, task_id: u64) -> Option<TaskStatus> {
        let mut statuses = self.statuses.lock().unwrap();
        statuses.remove(&task_id)
    }

    /// 获取所有任务状态
    pub fn get_all(&self) -> HashMap<u64, TaskStatus> {
        let statuses = self.statuses.lock().unwrap();
        statuses.clone()
    }

    /// 获取指定状态的任务数量
    pub fn count_by_status(&self, status: TaskStatus) -> usize {
        let statuses = self.statuses.lock().unwrap();
        statuses.values().filter(|&&s| s == status).count()
    }

    /// 清空所有任务状态
    pub fn clear(&self) {
        let mut statuses = self.statuses.lock().unwrap();
        statuses.clear();
    }
}

impl Default for TaskStatusTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for TaskStatusTracker {
    fn clone(&self) -> Self {
        Self {
            statuses: Arc::clone(&self.statuses),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_generator_generates_unique_ids() {
        let generator = TaskIdGenerator::new();
        let id1 = generator.next_id();
        let id2 = generator.next_id();
        let id3 = generator.next_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn task_status_tracker_registers_and_updates() {
        let tracker = TaskStatusTracker::new();

        tracker.register(1);
        assert_eq!(tracker.get(1), Some(TaskStatus::Pending));

        tracker.update(1, TaskStatus::Running);
        assert_eq!(tracker.get(1), Some(TaskStatus::Running));

        tracker.update(1, TaskStatus::Completed);
        assert_eq!(tracker.get(1), Some(TaskStatus::Completed));
    }

    #[test]
    fn task_status_tracker_counts_by_status() {
        let tracker = TaskStatusTracker::new();

        tracker.register(1);
        tracker.register(2);
        tracker.register(3);
        tracker.update(2, TaskStatus::Running);
        tracker.update(3, TaskStatus::Completed);

        assert_eq!(tracker.count_by_status(TaskStatus::Pending), 1);
        assert_eq!(tracker.count_by_status(TaskStatus::Running), 1);
        assert_eq!(tracker.count_by_status(TaskStatus::Completed), 1);
        assert_eq!(tracker.count_by_status(TaskStatus::Failed), 0);
    }

    #[test]
    fn task_status_display() {
        assert_eq!(format!("{}", TaskStatus::Pending), "pending");
        assert_eq!(format!("{}", TaskStatus::Running), "running");
        assert_eq!(format!("{}", TaskStatus::Completed), "completed");
        assert_eq!(format!("{}", TaskStatus::Failed), "failed");
    }
}
