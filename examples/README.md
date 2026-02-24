# 示例程序

本目录包含展示命令池库各项功能的示例程序。所有示例都可以使用 `cargo run --example <示例名>` 运行。

## 综合示例

### comprehensive_demo.rs
**综合功能演示** - 展示所有主要功能的完整示例

运行：`cargo run --example comprehensive_demo`

包含功能：
- 结构化日志和追踪
- 指标收集
- 健康检查
- 优雅关闭
- 错误重试
- 超时控制
- 任务取消
- 环境变量
- 资源限制
- 僵尸进程清理
- 性能分析钩子

## Phase 1: 基础功能

### logging_demo.rs
**日志系统演示** - 展示结构化日志和追踪功能

运行：`cargo run --example logging_demo`

功能：
- 配置日志级别
- 任务生命周期日志
- 结构化日志输出
- 指标集成

相关需求：需求 1 (结构化日志和追踪)

### graceful_shutdown.rs
**优雅关闭演示** - 展示如何优雅地关闭命令池

运行：`cargo run --example graceful_shutdown`

功能：
- 配置关闭超时
- 等待任务完成
- 关闭后拒绝新任务
- 资源清理

相关需求：需求 2 (优雅关闭机制)

### error_context_demo.rs
**错误上下文演示** - 展示丰富的错误信息

运行：`cargo run --example error_context_demo`

功能：
- 详细的错误上下文
- 区分不同错误类型
- 超时错误详情
- 命令执行失败信息

相关需求：需求 3 (错误上下文增强)

### config_validation_demo.rs
**配置验证演示** - 展示配置参数验证

运行：`cargo run --example config_validation_demo`

功能：
- 线程数验证
- 队列容量验证
- 超时值验证
- 系统限制检查
- 清晰的错误消息

相关需求：需求 5 (配置参数验证)

## Phase 2: 监控和资源管理

### metrics_demo.rs
**指标收集演示** - 展示完整的指标收集系统

运行：`cargo run --example metrics_demo`

功能：
- 任务计数统计
- 执行时间分析
- 成功率计算
- 实时监控
- 百分位数统计

相关需求：需求 7 (指标收集系统)

### health_check_demo.rs
**健康检查演示** - 展示系统健康状态监控

运行：`cargo run --example health_check_demo`

功能：
- 工作线程状态检查
- 队列使用率监控
- 长时间运行任务检测
- 健康状态分类

相关需求：需求 10 (健康检查接口)

### resource_limits_demo.rs
**资源限制演示** - 展示资源使用限制

运行：`cargo run --example resource_limits_demo`

功能：
- 输出大小限制
- 内存使用限制
- 资源超限处理

相关需求：需求 8 (资源限制)

### zombie_reaper_demo.rs
**僵尸进程清理演示** - 展示自动清理僵尸进程

运行：`cargo run --example zombie_reaper_demo`

功能：
- 定期检查僵尸进程
- 自动回收进程
- 关闭时清理

相关需求：需求 9 (僵尸进程清理)

## Phase 3: 高级功能

### retry_strategy_demo.rs
**重试策略演示** - 展示不同的重试策略

运行：`cargo run --example retry_strategy_demo`

功能：
- 固定间隔重试
- 指数退避重试
- 重试次数限制

相关需求：需求 11 (错误重试机制)

### retry_execution_demo.rs
**重试执行演示** - 展示重试机制的执行过程

运行：`cargo run --example retry_execution_demo`

功能：
- 重试执行流程
- 重试日志记录
- 失败处理

相关需求：需求 11 (错误重试机制)

### retry_integration_demo.rs
**重试集成演示** - 展示重试机制与命令池的集成

运行：`cargo run --example retry_integration_demo`

功能：
- CommandPool 重试集成
- CommandPoolSeg 重试集成
- 指标准确性验证

相关需求：需求 11 (错误重试机制)

### separated_timeout_demo.rs
**分离超时演示** - 展示启动和执行超时的分离控制

运行：`cargo run --example separated_timeout_demo`

功能：
- 启动超时配置
- 执行超时配置
- 超时类型区分

相关需求：需求 12 (超时粒度控制)

### timeout_config_demo.rs
**超时配置演示** - 展示超时配置的详细用法

运行：`cargo run --example timeout_config_demo`

功能：
- TimeoutConfig 配置
- 不同超时场景
- 超时错误处理

相关需求：需求 12 (超时粒度控制)

### task_cancellation_demo.rs
**任务取消演示** - 展示任务取消机制

运行：`cargo run --example task_cancellation_demo`

功能：
- 取消队列中的任务
- 取消执行中的任务
- TaskHandle 使用
- 取消状态查询

相关需求：需求 13 (任务取消机制)

### submit_with_handle_demo.rs
**任务句柄演示** - 展示 TaskHandle 的使用

运行：`cargo run --example submit_with_handle_demo`

功能：
- 提交任务并获取句柄
- 等待任务完成
- 查询任务状态

相关需求：需求 13 (任务取消机制)

### env_config_demo.rs
**环境变量演示** - 展示环境变量配置

运行：`cargo run --example env_config_demo`

功能：
- 设置环境变量
- 清除环境变量
- 继承父进程环境
- 环境变量传递验证

相关需求：需求 14 (环境变量支持)

### hooks_demo.rs
**性能钩子演示** - 展示性能分析钩子

运行：`cargo run --example hooks_demo`

功能：
- before_execute 钩子
- after_execute 钩子
- 自定义性能监控
- 钩子隔离性

相关需求：需求 15 (性能分析钩子)

### hook_demo.rs
**钩子基础演示** - 展示钩子的基本用法

运行：`cargo run --example hook_demo`

功能：
- ExecutionHook trait 实现
- 钩子注册
- 执行上下文访问

相关需求：需求 15 (性能分析钩子)

## 其他示例

### tokio_integration.rs
**Tokio 集成示例** - 展示与 Tokio 异步运行时的集成

运行：`cargo run --example tokio_integration`

功能：
- 异步任务提交
- 异步等待完成
- 与 Tokio 生态系统集成

## 运行所有示例

你可以使用以下命令查看所有可用示例：

```bash
cargo run --example
```

运行特定示例：

```bash
cargo run --example comprehensive_demo
cargo run --example metrics_demo
cargo run --example health_check_demo
# ... 等等
```

## 示例分类

### 按功能分类

**可观测性**：
- logging_demo.rs
- metrics_demo.rs
- health_check_demo.rs

**可靠性**：
- graceful_shutdown.rs
- error_context_demo.rs
- retry_strategy_demo.rs
- retry_execution_demo.rs
- retry_integration_demo.rs

**资源管理**：
- resource_limits_demo.rs
- zombie_reaper_demo.rs

**任务控制**：
- task_cancellation_demo.rs
- submit_with_handle_demo.rs
- timeout_config_demo.rs
- separated_timeout_demo.rs

**配置和环境**：
- config_validation_demo.rs
- env_config_demo.rs

**扩展性**：
- hooks_demo.rs
- hook_demo.rs

**综合**：
- comprehensive_demo.rs

### 按实施阶段分类

**Phase 1 (高优先级)**：
- logging_demo.rs
- graceful_shutdown.rs
- error_context_demo.rs
- config_validation_demo.rs

**Phase 2 (中优先级)**：
- metrics_demo.rs
- health_check_demo.rs
- resource_limits_demo.rs
- zombie_reaper_demo.rs

**Phase 3 (低优先级)**：
- retry_strategy_demo.rs
- retry_execution_demo.rs
- retry_integration_demo.rs
- timeout_config_demo.rs
- separated_timeout_demo.rs
- task_cancellation_demo.rs
- submit_with_handle_demo.rs
- env_config_demo.rs
- hooks_demo.rs
- hook_demo.rs

## 学习路径

如果你是新用户，建议按以下顺序学习示例：

1. **comprehensive_demo.rs** - 了解所有功能的概览
2. **logging_demo.rs** - 学习日志系统
3. **metrics_demo.rs** - 学习指标收集
4. **health_check_demo.rs** - 学习健康检查
5. **graceful_shutdown.rs** - 学习优雅关闭
6. **error_context_demo.rs** - 学习错误处理
7. **retry_integration_demo.rs** - 学习重试机制
8. **task_cancellation_demo.rs** - 学习任务取消
9. 根据需要探索其他特定功能的示例

## 贡献

如果你想添加新的示例，请确保：

1. 示例代码清晰易懂
2. 包含详细的注释说明
3. 展示实际使用场景
4. 在本 README 中添加相应的文档
5. 遵循现有示例的代码风格

## 问题反馈

如果你在运行示例时遇到问题，或者有改进建议，请提交 issue。
