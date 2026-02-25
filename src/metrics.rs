#![cfg(feature = "metrics")]

use hdrhistogram::Histogram;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// 指标收集器
///
/// 用于收集和聚合命令池的运行时指标，包括任务计数、执行时间统计等。
/// 所有计数器使用原子操作，保证线程安全且高性能。
///
/// # 示例
///
/// ```ignore
/// use execute::Metrics;
/// use std::time::Duration;
///
/// let metrics = Metrics::new();
///
/// // 记录任务提交
/// metrics.record_task_submitted();
///
/// // 记录任务完成
/// metrics.record_task_completed(Duration::from_millis(100));
///
/// // 获取指标快照
/// let snapshot = metrics.snapshot();
/// println!("Success rate: {:.2}%", snapshot.success_rate * 100.0);
/// println!("Average execution time: {:?}", snapshot.avg_execution_time);
/// ```
#[derive(Clone)]
pub struct Metrics {
    // 计数器
    pub(crate) tasks_submitted: Arc<AtomicU64>,
    pub(crate) tasks_completed: Arc<AtomicU64>,
    pub(crate) tasks_failed: Arc<AtomicU64>,
    pub(crate) tasks_cancelled: Arc<AtomicU64>,

    // 当前状态
    pub(crate) tasks_queued: Arc<AtomicUsize>,
    pub(crate) tasks_running: Arc<AtomicUsize>,

    // 执行时间统计
    execution_stats: Arc<RwLock<ExecutionStats>>,
}

impl Metrics {
    /// 创建新的指标收集器
    ///
    /// 所有计数器初始化为 0。
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::Metrics;
    ///
    /// let metrics = Metrics::new();
    /// let snapshot = metrics.snapshot();
    /// assert_eq!(snapshot.tasks_submitted, 0);
    /// ```
    pub fn new() -> Self {
        Self {
            tasks_submitted: Arc::new(AtomicU64::new(0)),
            tasks_completed: Arc::new(AtomicU64::new(0)),
            tasks_failed: Arc::new(AtomicU64::new(0)),
            tasks_cancelled: Arc::new(AtomicU64::new(0)),
            tasks_queued: Arc::new(AtomicUsize::new(0)),
            tasks_running: Arc::new(AtomicUsize::new(0)),
            execution_stats: Arc::new(RwLock::new(ExecutionStats::new())),
        }
    }

    /// 记录任务提交
    ///
    /// 增加已提交任务计数和队列中任务计数。
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::Metrics;
    ///
    /// let metrics = Metrics::new();
    /// metrics.record_task_submitted();
    /// let snapshot = metrics.snapshot();
    /// assert_eq!(snapshot.tasks_submitted, 1);
    /// assert_eq!(snapshot.tasks_queued, 1);
    /// ```
    pub fn record_task_submitted(&self) {
        self.tasks_submitted.fetch_add(1, Ordering::Relaxed);
        self.tasks_queued.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录任务开始执行
    ///
    /// 减少队列中任务计数，增加正在执行任务计数。
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::Metrics;
    ///
    /// let metrics = Metrics::new();
    /// metrics.record_task_submitted();
    /// metrics.record_task_started();
    /// let snapshot = metrics.snapshot();
    /// assert_eq!(snapshot.tasks_queued, 0);
    /// assert_eq!(snapshot.tasks_running, 1);
    /// ```
    pub fn record_task_started(&self) {
        self.tasks_queued.fetch_sub(1, Ordering::Relaxed);
        self.tasks_running.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录任务完成
    ///
    /// 增加已完成任务计数，减少正在执行任务计数，并记录执行时间。
    ///
    /// # 参数
    ///
    /// * `duration` - 任务执行时长
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::Metrics;
    /// use std::time::Duration;
    ///
    /// let metrics = Metrics::new();
    /// metrics.record_task_submitted();
    /// metrics.record_task_started();
    /// metrics.record_task_completed(Duration::from_millis(100));
    /// let snapshot = metrics.snapshot();
    /// assert_eq!(snapshot.tasks_completed, 1);
    /// assert_eq!(snapshot.tasks_running, 0);
    /// ```
    pub fn record_task_completed(&self, duration: Duration) {
        self.tasks_completed.fetch_add(1, Ordering::Relaxed);
        self.tasks_running.fetch_sub(1, Ordering::Relaxed);

        let mut stats = self.execution_stats.write().unwrap();
        stats.record(duration);
    }

    /// 记录任务失败
    ///
    /// 增加失败任务计数，减少正在执行任务计数，并记录执行时间。
    ///
    /// # 参数
    ///
    /// * `duration` - 任务执行时长（直到失败）
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::Metrics;
    /// use std::time::Duration;
    ///
    /// let metrics = Metrics::new();
    /// metrics.record_task_submitted();
    /// metrics.record_task_started();
    /// metrics.record_task_failed(Duration::from_millis(50));
    /// let snapshot = metrics.snapshot();
    /// assert_eq!(snapshot.tasks_failed, 1);
    /// assert_eq!(snapshot.tasks_running, 0);
    /// ```
    pub fn record_task_failed(&self, duration: Duration) {
        self.tasks_failed.fetch_add(1, Ordering::Relaxed);
        self.tasks_running.fetch_sub(1, Ordering::Relaxed);

        let mut stats = self.execution_stats.write().unwrap();
        stats.record(duration);
    }

    /// 记录任务取消
    ///
    /// 增加已取消任务计数，减少队列中任务计数。
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::Metrics;
    ///
    /// let metrics = Metrics::new();
    /// metrics.record_task_submitted();
    /// metrics.record_task_cancelled();
    /// let snapshot = metrics.snapshot();
    /// assert_eq!(snapshot.tasks_cancelled, 1);
    /// assert_eq!(snapshot.tasks_queued, 0);
    /// ```
    pub fn record_task_cancelled(&self) {
        self.tasks_cancelled.fetch_add(1, Ordering::Relaxed);
        self.tasks_queued.fetch_sub(1, Ordering::Relaxed);
    }

    /// 获取指标快照
    ///
    /// 返回当前时刻的指标快照，包括所有计数器和统计信息。
    /// 快照是一致的，但不保证原子性（不同字段可能来自不同时刻）。
    ///
    /// # 返回
    ///
    /// 返回 `MetricsSnapshot`，包含：
    /// - 任务计数（已提交、已完成、失败、取消、队列中、执行中）
    /// - 成功率
    /// - 执行时间统计（平均值、最小值、最大值、百分位数）
    ///
    /// # 示例
    ///
    /// ```
    /// use execute::Metrics;
    /// use std::time::Duration;
    ///
    /// let metrics = Metrics::new();
    /// metrics.record_task_submitted();
    /// metrics.record_task_started();
    /// metrics.record_task_completed(Duration::from_millis(100));
    ///
    /// let snapshot = metrics.snapshot();
    /// println!("Tasks submitted: {}", snapshot.tasks_submitted);
    /// println!("Tasks completed: {}", snapshot.tasks_completed);
    /// println!("Success rate: {:.2}%", snapshot.success_rate * 100.0);
    /// println!("Average execution time: {:?}", snapshot.avg_execution_time);
    /// println!("P95 execution time: {:?}", snapshot.p95_execution_time);
    /// ```
    pub fn snapshot(&self) -> MetricsSnapshot {
        let submitted = self.tasks_submitted.load(Ordering::Relaxed);
        let completed = self.tasks_completed.load(Ordering::Relaxed);
        let failed = self.tasks_failed.load(Ordering::Relaxed);

        let success_rate = if submitted > 0 {
            (completed as f64) / (submitted as f64)
        } else {
            0.0
        };

        let stats = self.execution_stats.read().unwrap();

        MetricsSnapshot {
            tasks_submitted: submitted,
            tasks_completed: completed,
            tasks_failed: failed,
            tasks_cancelled: self.tasks_cancelled.load(Ordering::Relaxed),
            tasks_queued: self.tasks_queued.load(Ordering::Relaxed),
            tasks_running: self.tasks_running.load(Ordering::Relaxed),
            success_rate,
            avg_execution_time: stats.avg(),
            min_execution_time: stats.min,
            max_execution_time: stats.max,
            p50_execution_time: stats.percentile(50.0),
            p95_execution_time: stats.percentile(95.0),
            p99_execution_time: stats.percentile(99.0),
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// 执行统计信息
struct ExecutionStats {
    count: u64,
    sum: Duration,
    min: Duration,
    max: Duration,
    // 使用 HDR Histogram 记录执行时间分布，用于计算百分位数
    // 记录的单位是微秒，范围从 1 微秒到 1 小时（3,600,000,000 微秒）
    histogram: Histogram<u64>,
}

impl ExecutionStats {
    fn new() -> Self {
        // 创建 histogram，范围从 1 微秒到 1 小时，精度为 3 位有效数字
        let histogram = Histogram::<u64>::new_with_bounds(1, 3_600_000_000, 3)
            .expect("Failed to create histogram");

        Self {
            count: 0,
            sum: Duration::ZERO,
            min: Duration::MAX,
            max: Duration::ZERO,
            histogram,
        }
    }

    fn record(&mut self, duration: Duration) {
        self.count += 1;
        self.sum += duration;
        self.min = self.min.min(duration);
        self.max = self.max.max(duration);

        // 将 duration 转换为微秒并记录到 histogram
        let micros = duration.as_micros() as u64;
        // 确保值在有效范围内
        let micros = micros.clamp(1, 3_600_000_000);
        let _ = self.histogram.record(micros);
    }

    fn avg(&self) -> Duration {
        if self.count > 0 {
            self.sum / self.count as u32
        } else {
            Duration::ZERO
        }
    }

    fn percentile(&self, percentile: f64) -> Duration {
        if self.count == 0 {
            return Duration::ZERO;
        }

        let micros = self.histogram.value_at_percentile(percentile);
        Duration::from_micros(micros)
    }
}

impl Clone for ExecutionStats {
    fn clone(&self) -> Self {
        // 创建新的 histogram 并复制数据
        let mut histogram = Histogram::<u64>::new_with_bounds(1, 3_600_000_000, 3)
            .expect("Failed to create histogram");

        // 复制 histogram 数据
        histogram
            .add(&self.histogram)
            .expect("Failed to clone histogram");

        Self {
            count: self.count,
            sum: self.sum,
            min: self.min,
            max: self.max,
            histogram,
        }
    }
}

/// 指标快照
///
/// 包含某一时刻的所有指标数据。
/// 用于监控系统性能和健康状态。
///
/// # 字段
///
/// * `tasks_submitted` - 已提交的任务总数
/// * `tasks_completed` - 已成功完成的任务总数
/// * `tasks_failed` - 失败的任务总数
/// * `tasks_cancelled` - 被取消的任务总数
/// * `tasks_queued` - 当前队列中的任务数
/// * `tasks_running` - 当前正在执行的任务数
/// * `success_rate` - 成功率（0.0 - 1.0）
/// * `avg_execution_time` - 平均执行时间
/// * `min_execution_time` - 最小执行时间
/// * `max_execution_time` - 最大执行时间
/// * `p50_execution_time` - 50% 百分位执行时间（中位数）
/// * `p95_execution_time` - 95% 百分位执行时间
/// * `p99_execution_time` - 99% 百分位执行时间
///
/// # 示例
///
/// ```ignore
/// use execute::Metrics;
///
/// let metrics = Metrics::new();
/// let snapshot = metrics.snapshot();
///
/// // 打印指标
/// println!("Tasks: submitted={}, completed={}, failed={}",
///     snapshot.tasks_submitted,
///     snapshot.tasks_completed,
///     snapshot.tasks_failed
/// );
/// println!("Success rate: {:.2}%", snapshot.success_rate * 100.0);
/// println!("Execution time: avg={:?}, p95={:?}, p99={:?}",
///     snapshot.avg_execution_time,
///     snapshot.p95_execution_time,
///     snapshot.p99_execution_time
/// );
/// ```
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub tasks_submitted: u64,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub tasks_cancelled: u64,
    pub tasks_queued: usize,
    pub tasks_running: usize,
    pub success_rate: f64,
    pub avg_execution_time: Duration,
    pub min_execution_time: Duration,
    pub max_execution_time: Duration,
    pub p50_execution_time: Duration,
    pub p95_execution_time: Duration,
    pub p99_execution_time: Duration,
}
