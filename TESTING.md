# 测试文档

本文档说明项目的测试结构和运行方式。

## 测试分类

### 1. 单元测试 (Unit Tests)

位于 `src/` 目录下的 `#[cfg(test)]` 模块中，测试单个模块的功能。

```bash
cargo test --lib
```

主要测试文件：
- `src/executor.rs` - 执行器核心功能
- `src/pipeline.rs` - 管道功能
- `src/task_handle.rs` - 任务句柄
- `src/task_status.rs` - 任务状态
- `src/process_pool.rs` - 进程池

### 2. 集成测试 (Integration Tests)

位于 `tests/` 目录，测试多个模块的协作。

```bash
cargo test --test <test_name>
```

#### 2.1 配置相关测试

| 测试文件 | 说明 |
|---------|------|
| `config_tests.rs` | 基础配置测试 |
| `config_validation_test.rs` | 配置验证测试 |
| `config_validation_error_message_property_test.rs` | 配置错误消息属性测试 |
| `command_config_timeout_test.rs` | 命令配置超时测试 |

#### 2.2 执行相关测试

| 测试文件 | 说明 |
|---------|------|
| `pool_tests.rs` | 命令池基础功能测试 |
| `retry_execution_test.rs` | 重试执行测试 |
| `retry_integration_test.rs` | 重试集成测试 |
| `retry_behavior_property_test.rs` | 重试行为属性测试 |
| `retry_log_property_test.rs` | 重试日志属性测试 |
| `retry_strategy_test.rs` | 重试策略测试 |

#### 2.3 超时相关测试

| 测试文件 | 说明 |
|---------|------|
| `timeout_config_test.rs` | 超时配置测试 |
| `separated_timeout_test.rs` | 分离超时测试 |
| `timeout_error_property_test.rs` | 超时错误属性测试 |
| `timeout_type_distinction_property_test.rs` | 超时类型区分属性测试 |

#### 2.4 环境变量测试

| 测试文件 | 说明 |
|---------|------|
| `env_config_test.rs` | 环境配置测试 |
| `env_var_passing_property_test.rs` | 环境变量传递属性测试 |
| `env_var_clearing_property_test.rs` | 环境变量清除属性测试 |

#### 2.5 任务管理测试

| 测试文件 | 说明 |
|---------|------|
| `task_handle_integration_test.rs` | 任务句柄集成测试 |
| `task_cancellation_error_test.rs` | 任务取消错误测试 |
| `task_cancellation_effectiveness_property_test.rs` | 任务取消效果属性测试 |

#### 2.6 资源限制测试

| 测试文件 | 说明 |
|---------|------|
| `resource_limits_test.rs` | 资源限制测试 |
| `memory_limit_termination_property_test.rs` | 内存限制终止属性测试 |
| `output_size_limit_property_test.rs` | 输出大小限制属性测试 |

#### 2.7 指标和日志测试

| 测试文件 | 说明 |
|---------|------|
| `metrics_test.rs` | 指标测试 |
| `simple_metrics_test.rs` | 简单指标测试 |
| `metrics_accuracy_property_test.rs` | 指标准确性属性测试 |
| `execution_time_stats_property_test.rs` | 执行时间统计属性测试 |
| `log_integrity_property_test.rs` | 日志完整性属性测试 |
| `log_level_filtering_property_test.rs` | 日志级别过滤属性测试 |

#### 2.8 关闭和清理测试

| 测试文件 | 说明 |
|---------|------|
| `shutdown_tests.rs` | 关闭测试 |
| `graceful_shutdown_wait_property_test.rs` | 优雅关闭等待属性测试 |
| `shutdown_reject_tasks_property_test.rs` | 关闭拒绝任务属性测试 |
| `zombie_reaper_integration.rs` | 僵尸进程回收集成测试 |
| `zombie_reaper_property_test.rs` | 僵尸进程回收属性测试 |

#### 2.9 健康检查测试

| 测试文件 | 说明 |
|---------|------|
| `health_check_test.rs` | 健康检查测试 |
| `health_check_accuracy_property_test.rs` | 健康检查准确性属性测试 |
| `health_status_classification_property_test.rs` | 健康状态分类属性测试 |

#### 2.10 其他测试

| 测试文件 | 说明 |
|---------|------|
| `hook_integration_test.rs` | 钩子集成测试 |
| `error_context_property_test.rs` | 错误上下文属性测试 |
| `polling_optimization_test.rs` | 轮询优化测试 |
| `pool_seg_stop_property_test.rs` | 分段池停止属性测试 |
| `phase2_verification.rs` | 第二阶段验证 |
| `success_rate_calculation_property_test.rs` | 成功率计算属性测试 |

### 3. 基准测试 (Benchmarks)

位于 `benches/` 目录，用于性能测试。

```bash
cargo bench
```

| 测试文件 | 说明 |
|---------|------|
| `command_pool_bench.rs` | 命令池性能基准测试 |

### 4. 文档测试 (Doc Tests)

位于代码文档中的示例代码。

```bash
cargo test --doc
```

## 运行测试

### 运行所有测试

```bash
cargo test --all
```

### 运行特定测试

```bash
# 运行单元测试
cargo test --lib

# 运行特定集成测试
cargo test --test pool_tests

# 运行包含特定名称的测试
cargo test pipeline

# 运行并显示输出
cargo test --all -- --nocapture
```

### 运行属性测试（Property Tests）

属性测试使用 `proptest` 库，会自动生成大量随机输入进行测试。

```bash
# 运行所有属性测试
cargo test --all prop_

# 运行特定属性测试
cargo test --test config_validation_error_message_property_test
```

## 测试覆盖率

项目使用多种测试类型确保代码质量：

- **单元测试**：覆盖核心功能
- **集成测试**：验证模块间协作
- **属性测试**：验证不变量和边界条件
- **基准测试**：确保性能不下降

## CI 检查

在提交代码前，请确保通过以下检查：

```bash
# 构建所有目标
cargo build --all-targets

# 运行所有测试
cargo test --all

# 运行 Clippy
cargo clippy --all-targets --all-features -- -D warnings

# 检查格式化
cargo fmt --all -- --check
```

## 添加新测试

### 单元测试

在源文件中添加 `#[cfg(test)]` 模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_feature() {
        // 测试代码
    }
}
```

### 集成测试

在 `tests/` 目录创建新文件：

```rust
// tests/my_feature_test.rs
use execute::{CommandPool, CommandConfig};

#[test]
fn test_my_feature() {
    // 测试代码
}
```

### 属性测试

使用 `proptest` 创建属性测试：

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_property(input in 1..100u32) {
        // 属性测试代码
    }
}
```

## 测试数据

- `.proptest-regressions` 文件：存储属性测试的失败案例，用于回归测试
- 这些文件应该提交到版本控制

## 注意事项

1. **超时设置**：某些测试涉及超时，可能需要较长时间运行
2. **并发测试**：部分测试使用多线程，可能受系统负载影响
3. **平台差异**：某些测试只在特定平台运行（如 `#[cfg(unix)]`）
4. **资源限制**：资源限制测试可能需要特定权限
