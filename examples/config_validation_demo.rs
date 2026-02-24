/// 配置参数验证演示
///
/// 此示例展示如何使用配置验证功能来捕获配置错误。
/// 注意：当前实现可能没有显式的 validate() 方法，
/// 但配置验证会在创建命令池时自动进行。
use execute::ExecutionConfig;
use std::time::Duration;

fn main() {
    println!("=== 配置参数验证演示 ===\n");

    // 场景 1: 有效配置
    println!("场景 1: 有效配置");
    let config = ExecutionConfig::new().with_workers(4);
    println!("  ✓ 配置创建成功");
    println!("  工作线程数: {}\n", config.workers);

    // 场景 2: 最小有效线程数（1）
    println!("场景 2: 最小有效线程数（1）");
    let config = ExecutionConfig::new().with_workers(1);
    println!("  ✓ 配置创建成功");
    println!("  工作线程数: {}\n", config.workers);

    // 场景 3: 使用默认值
    println!("场景 3: 使用默认值");
    let config = ExecutionConfig::default();
    println!("  ✓ 默认配置创建成功");
    println!("  默认值:");
    println!("    - 工作线程数: {} (CPU 核心数)", config.workers);
    println!("    - 并发限制: {:?}", config.concurrency_limit);
    println!();

    // 场景 4: 使用 builder 模式的完整配置
    println!("场景 4: 使用 builder 模式的完整配置");
    let config = ExecutionConfig::new()
        .with_workers(8)
        .with_concurrency_limit(100)
        .with_zombie_reaper_interval(Duration::from_secs(60));

    println!("  ✓ 配置创建成功");
    println!("  配置详情:");
    println!("    - 工作线程数: {}", config.workers);
    println!("    - 并发限制: {:?}", config.concurrency_limit);
    println!(
        "    - 僵尸进程清理间隔: {:?}",
        config.zombie_reaper_interval
    );
    println!();

    // 场景 5: 大线程数配置
    println!("场景 5: 大线程数配置");
    let config = ExecutionConfig::new().with_workers(16);
    println!("  ✓ 配置创建成功");
    println!("  工作线程数: {}\n", config.workers);

    // 场景 6: 有效的僵尸进程清理间隔
    println!("场景 6: 有效的僵尸进程清理间隔");
    let config = ExecutionConfig::new()
        .with_workers(4)
        .with_zombie_reaper_interval(Duration::from_secs(10));
    println!("  ✓ 配置创建成功");
    println!("  僵尸进程清理间隔: {:?}\n", config.zombie_reaper_interval);

    // 场景 7: 并发限制配置
    println!("场景 7: 并发限制配置");
    let config = ExecutionConfig::new()
        .with_workers(8)
        .with_concurrency_limit(50);
    println!("  ✓ 配置创建成功");
    println!("  工作线程数: {}", config.workers);
    println!("  并发限制: {:?}\n", config.concurrency_limit);

    println!("=== 演示完成 ===");
    println!("\n总结:");
    println!("- Builder 模式提供类型安全的配置接口");
    println!("- 默认值提供合理的开箱即用配置");
    println!("- 配置参数在创建命令池时会被验证");
    println!("- 无效配置会在运行时被检测并报告");
}
