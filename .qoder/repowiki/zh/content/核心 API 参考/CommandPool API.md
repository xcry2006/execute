# CommandPool API

<cite>
**本文引用的文件**
- [src/lib.rs](file://src/lib.rs)
- [src/pool.rs](file://src/pool.rs)
- [src/config.rs](file://src/config.rs)
- [src/error.rs](file://src/error.rs)
- [src/execution_mode.rs](file://src/execution_mode.rs)
- [src/executor.rs](file://src/executor.rs)
- [src/thread_executor.rs](file://src/thread_executor.rs)
- [src/semaphore.rs](file://src/semaphore.rs)
- [src/pool_seg.rs](file://src/pool_seg.rs)
- [examples/tokio_integration.rs](file://examples/tokio_integration.rs)
- [tests/pool_tests.rs](file://tests/pool_tests.rs)
- [benches/command_pool_bench.rs](file://benches/command_pool_bench.rs)
- [README.md](file://README.md)
- [Cargo.toml](file://Cargo.toml)
</cite>

## 目录
1. [简介](#简介)
2. [项目结构](#项目结构)
3. [核心组件](#核心组件)
4. [架构总览](#架构总览)
5. [详细组件分析](#详细组件分析)
6. [依赖关系分析](#依赖关系分析)
7. [性能考量](#性能考量)
8. [故障排查指南](#故障排查指南)
9. [结论](#结论)
10. [附录](#附录)

## 简介
本文件为 CommandPool API 的权威参考文档，面向使用者与维护者，系统性阐述 CommandPool 结构体的公共接口、数据模型、线程安全机制与内部工作原理，并提供使用示例、性能建议与最佳实践。CommandPool 提供命令队列与后台执行器，支持多线程与多进程两种执行模式，亦可通过自定义执行器扩展至异步运行时（如 Tokio）。

## 项目结构
- 库入口导出：通过 lib.rs 统一导出对外 API（CommandPool、ExecutionConfig、ExecutionMode、CommandExecutor 等）。
- 核心实现：pool.rs 定义 CommandPool；executor.rs 定义执行器接口与标准实现；thread_executor.rs 提供线程模式执行器；execution_mode.rs 定义执行模式与配置；config.rs 定义命令配置；error.rs 定义错误类型；semaphore.rs 提供并发限制信号量；pool_seg.rs 提供无锁队列变体。
- 示例与测试：examples/tokio_integration.rs 展示自定义执行器集成；tests/pool_tests.rs 验证基本行为；benches/command_pool_bench.rs 提供基准测试。

```mermaid
graph TB
subgraph "库模块"
L["lib.rs<br/>统一导出"]
P["pool.rs<br/>CommandPool"]
PS["pool_seg.rs<br/>无锁变体"]
E["executor.rs<br/>执行器接口/实现"]
TE["thread_executor.rs<br/>线程模式执行器"]
EM["execution_mode.rs<br/>执行模式/配置"]
C["config.rs<br/>命令配置"]
ER["error.rs<br/>错误类型"]
S["semaphore.rs<br/>信号量"]
end
L --> P
L --> PS
L --> E
L --> TE
L --> EM
L --> C
L --> ER
L --> S
P --> E
P --> TE
P --> EM
P --> C
P --> ER
P --> S
PS --> E
PS --> EM
PS --> C
PS --> ER
PS --> S
```

图表来源
- [src/lib.rs](file://src/lib.rs#L1-L17)
- [src/pool.rs](file://src/pool.rs#L1-L296)
- [src/pool_seg.rs](file://src/pool_seg.rs#L1-L157)
- [src/executor.rs](file://src/executor.rs#L1-L100)
- [src/thread_executor.rs](file://src/thread_executor.rs#L1-L148)
- [src/execution_mode.rs](file://src/execution_mode.rs#L1-L70)
- [src/config.rs](file://src/config.rs#L1-L109)
- [src/error.rs](file://src/error.rs#L1-L18)
- [src/semaphore.rs](file://src/semaphore.rs#L1-L53)

章节来源
- [src/lib.rs](file://src/lib.rs#L1-L17)
- [README.md](file://README.md#L1-L60)

## 核心组件
- CommandPool：命令池，基于 Arc<Mutex<VecDeque<CommandConfig>>> 实现，支持多线程安全的任务入队与出队，提供多种启动执行器的方法。
- CommandConfig：命令配置，描述要执行的程序、参数、工作目录与超时。
- ExecutionMode/ExecutionConfig：执行模式与配置，支持 Thread 与 Process 两种模式及工作线程/进程数、并发限制等。
- CommandExecutor：执行器接口，允许注入自定义执行器（如 Tokio）。
- ThreadModeExecutor：线程模式专用执行器，将任务提交到线程池执行。
- ExecuteError：执行过程中的错误类型，包含 IO 错误、超时与子进程错误。
- Semaphore：基于 Mutex+Condvar 的简单计数信号量，用于限制外部进程并发数。

章节来源
- [src/pool.rs](file://src/pool.rs#L13-L296)
- [src/config.rs](file://src/config.rs#L19-L109)
- [src/execution_mode.rs](file://src/execution_mode.rs#L5-L70)
- [src/executor.rs](file://src/executor.rs#L5-L70)
- [src/thread_executor.rs](file://src/thread_executor.rs#L16-L148)
- [src/error.rs](file://src/error.rs#L7-L18)
- [src/semaphore.rs](file://src/semaphore.rs#L7-L53)

## 架构总览
CommandPool 的核心流程：
- 生产者将 CommandConfig 推入池（push_task），消费者线程周期性轮询（pop_task）并执行。
- 执行策略根据 ExecutionConfig.mode 决定：
  - Process：多进程模式，每个任务在独立子进程中执行，可选并发限制。
  - Thread：多线程模式，任务被转发到线程模式执行器，由线程池调度。
- 自定义执行器：通过 CommandExecutor trait 注入任意运行时（如 Tokio），实现异步执行与超时控制。

```mermaid
sequenceDiagram
participant Producer as "生产者"
participant Pool as "CommandPool"
participant Exec as "执行器线程"
participant Proc as "子进程/线程"
Producer->>Pool : "push_task(CommandConfig)"
Note over Pool : "Arc<Mutex<VecDeque>> 入队"
loop "轮询循环"
Exec->>Pool : "pop_task()"
alt "有任务"
Pool-->>Exec : "Some(CommandConfig)"
alt "Process 模式"
Exec->>Proc : "execute_command(...) 或自定义执行器"
Proc-->>Exec : "Output/错误"
else "Thread 模式"
Exec->>Proc : "execute_command(...)"
Proc-->>Exec : "Output/错误"
end
else "无任务"
Exec->>Exec : "sleep(interval)"
end
end
```

图表来源
- [src/pool.rs](file://src/pool.rs#L134-L288)
- [src/executor.rs](file://src/executor.rs#L26-L70)
- [src/thread_executor.rs](file://src/thread_executor.rs#L112-L137)

## 详细组件分析

### CommandPool 结构体与公共方法
- 结构体字段
  - tasks: Arc<Mutex<VecDeque<CommandConfig>>>，存储待执行任务。
  - exec_config: ExecutionConfig，执行模式与并发参数。
  - thread_executor: Option<Arc<ThreadModeExecutor>>，线程模式专用执行器。
- 关键方法
  - new(): 使用默认 ExecutionConfig 创建池。
  - with_config(config): 使用指定 ExecutionConfig 创建池。
  - execution_mode(): 返回当前执行模式。
  - execution_config(): 返回执行配置引用。
  - push_task(task): 将任务推入队尾。
  - pop_task(): 从队头弹出任务，若空返回 None。
  - is_empty(): 判断池是否为空。
  - start_executor(interval): 根据模式启动执行器。
  - start_executor_with_workers(interval, workers): 固定工作线程数的 Process 模式执行器。
  - start_executor_with_workers_and_limit(interval, workers, limit): 带并发限制的 Process 模式执行器。
  - execute_task(config): 同步执行单个任务（Process 模式）。
  - start_executor_with_executor/with_workers_and_executor/with_executor_and_limit: 使用自定义执行器的变体。

章节来源
- [src/pool.rs](file://src/pool.rs#L36-L289)

#### 方法详解与使用说明

- new()
  - 功能：创建默认的命令池（多进程模式）。
  - 参数：无。
  - 返回：CommandPool 实例。
  - 注意事项：默认工作线程数来自系统可用并行度。
  - 示例路径：[README 快速开始示例](file://README.md#L28-L37)

- with_config(config)
  - 功能：使用指定 ExecutionConfig 创建命令池。
  - 参数：ExecutionConfig。
  - 返回：CommandPool 实例。
  - 注意事项：当模式为 Thread 时，会初始化线程模式执行器。
  - 示例路径：[测试用例：线程模式创建](file://tests/pool_tests.rs#L38-L43)

- execution_mode()
  - 功能：查询当前执行模式。
  - 参数：无。
  - 返回：ExecutionMode。
  - 示例路径：[测试用例：默认模式为 Process](file://tests/pool_tests.rs#L32-L36)

- execution_config()
  - 功能：获取执行配置引用。
  - 参数：无。
  - 返回：&ExecutionConfig。
  - 示例路径：[测试用例：配置构建器模式](file://tests/pool_tests.rs#L79-L89)

- push_task(task)
  - 功能：将任务推入队尾。
  - 参数：CommandConfig。
  - 返回：无。
  - 线程安全：通过 Mutex 保护 VecDeque。
  - 示例路径：[README 快速开始示例](file://README.md#L30-L37)

- pop_task()
  - 功能：从队头弹出任务。
  - 参数：无。
  - 返回：Option<CommandConfig>。
  - 线程安全：通过 Mutex 保护 VecDeque。
  - 示例路径：[单元测试：push/pop/is_empty](file://tests/pool_tests.rs#L5-L16)

- is_empty()
  - 功能：判断池是否为空。
  - 参数：无。
  - 返回：bool。
  - 线程安全：通过 Mutex 保护 VecDeque。
  - 示例路径：[单元测试：push/pop/is_empty](file://tests/pool_tests.rs#L5-L16)

- start_executor(interval)
  - 功能：根据 ExecutionConfig.mode 启动执行器。
  - 参数：Duration。
  - 返回：无。
  - 线程安全：内部使用 Arc/Clone 传递共享状态。
  - 示例路径：[README 快速开始示例](file://README.md#L30-L37)

- start_executor_with_workers(interval, workers)
  - 功能：固定工作线程数的 Process 模式执行器。
  - 参数：Duration, usize。
  - 返回：无。
  - 示例路径：[Tokio 集成示例](file://examples/tokio_integration.rs#L42-L61)

- start_executor_with_workers_and_limit(interval, workers, limit)
  - 功能：带并发限制的 Process 模式执行器。
  - 参数：Duration, usize, usize。
  - 返回：无。
  - 并发限制：使用 Semaphore 控制外部进程数量。
  - 示例路径：[Tokio 集成示例](file://examples/tokio_integration.rs#L55-L56)

- execute_task(config)
  - 功能：同步执行单个任务（Process 模式）。
  - 参数：&CommandConfig。
  - 返回：Result<Output, ExecuteError>。
  - 超时处理：使用 wait-timeout crate 在当前线程等待，避免额外线程。
  - 示例路径：[基准测试：execute_true](file://benches/command_pool_bench.rs#L39-L49)

- start_executor_with_executor/with_workers_and_executor/with_executor_and_limit
  - 功能：使用自定义 CommandExecutor 启动执行器。
  - 参数：Duration, Arc<E>（E: CommandExecutor）。
  - 返回：无。
  - 适用场景：集成 Tokio、async-std 等异步运行时。
  - 示例路径：[Tokio 集成示例](file://examples/tokio_integration.rs#L10-L40)

#### 线程安全与内部机制
- 互斥保护：VecDeque 通过 Arc<Mutex<_>> 保护，保证多线程安全的 push/pop。
- 线程模式：当模式为 Thread 时，CommandPool 内部持有 ThreadModeExecutor，任务被提交到线程池执行。
- 并发限制：通过 Semaphore 控制外部进程并发数，避免系统资源耗尽。
- 超时等待：execute_command 使用 wait-timeout 在当前线程等待，减少额外线程开销。

章节来源
- [src/pool.rs](file://src/pool.rs#L134-L288)
- [src/executor.rs](file://src/executor.rs#L26-L70)
- [src/thread_executor.rs](file://src/thread_executor.rs#L112-L137)
- [src/semaphore.rs](file://src/semaphore.rs#L16-L52)

#### 使用示例与最佳实践
- 创建命令池与添加任务
  - 参考：[README 快速开始示例](file://README.md#L28-L37)
- 使用线程模式
  - 参考：[测试用例：线程模式创建](file://tests/pool_tests.rs#L38-L43)
- 使用自定义执行器（Tokio）
  - 参考：[Tokio 集成示例](file://examples/tokio_integration.rs#L42-L61)
- 最佳实践
  - 选择合适的执行模式：CPU 密集型任务优先考虑 Thread 模式；I/O 密集型任务可考虑 Process 模式或自定义异步执行器。
  - 合理设置 workers：根据 CPU 核心数与任务特性调整工作线程数。
  - 并发限制：为外部进程设置合理的并发上限，避免系统资源耗尽。
  - 超时控制：为命令配置超时，防止长时间阻塞。

章节来源
- [README.md](file://README.md#L15-L49)
- [examples/tokio_integration.rs](file://examples/tokio_integration.rs#L42-L61)
- [tests/pool_tests.rs](file://tests/pool_tests.rs#L38-L64)

### 数据模型与类图
```mermaid
classDiagram
class CommandPool {
+new() CommandPool
+with_config(config) CommandPool
+execution_mode() ExecutionMode
+execution_config() &ExecutionConfig
+push_task(task) void
+pop_task() Option~CommandConfig~
+is_empty() bool
+start_executor(interval) void
+start_executor_with_workers(interval, workers) void
+start_executor_with_workers_and_limit(interval, workers, limit) void
+execute_task(config) Result~Output, ExecuteError~
+start_executor_with_executor(interval, executor) void
+start_executor_with_workers_and_executor(interval, workers, executor) void
+start_executor_with_executor_and_limit(interval, workers, limit, executor) void
}
class CommandConfig {
+new(program, args) CommandConfig
+with_working_dir(dir) CommandConfig
+with_timeout(timeout) CommandConfig
+program() &str
+args() &[String]
+working_dir() Option<&str>
+timeout() Option<Duration>
}
class ExecutionMode {
<<enumeration>>
+Thread
+Process
}
class ExecutionConfig {
+mode ExecutionMode
+workers usize
+concurrency_limit Option<usize>
+new() ExecutionConfig
+with_mode(mode) ExecutionConfig
+with_workers(workers) ExecutionConfig
+with_concurrency_limit(limit) ExecutionConfig
}
class CommandExecutor {
<<trait>>
+execute(config) Result~Output, ExecuteError~
}
class StdCommandExecutor {
+execute(config) Result~Output, ExecuteError~
}
class ThreadModeExecutor {
+new(workers) ThreadModeExecutor
+start(interval) void
+submit(config) void
}
class ExecuteError {
<<enumeration>>
+Io(io : : Error)
+Timeout(Duration)
+Child(String)
}
class Semaphore {
+new(permits) Semaphore
+acquire() void
+acquire_guard() SemaphoreGuard
}
CommandPool --> ExecutionConfig : "使用"
CommandPool --> ExecutionMode : "使用"
CommandPool --> CommandConfig : "存储/执行"
CommandPool --> CommandExecutor : "可选自定义"
CommandPool --> ThreadModeExecutor : "线程模式"
CommandExecutor <|.. StdCommandExecutor : "实现"
CommandPool --> ExecuteError : "返回"
CommandPool --> Semaphore : "并发限制"
```

图表来源
- [src/pool.rs](file://src/pool.rs#L13-L296)
- [src/config.rs](file://src/config.rs#L19-L109)
- [src/execution_mode.rs](file://src/execution_mode.rs#L5-L70)
- [src/executor.rs](file://src/executor.rs#L5-L24)
- [src/thread_executor.rs](file://src/thread_executor.rs#L112-L137)
- [src/error.rs](file://src/error.rs#L7-L18)
- [src/semaphore.rs](file://src/semaphore.rs#L7-L53)

## 依赖关系分析
- 内部依赖
  - pool.rs 依赖 config.rs、error.rs、execution_mode.rs、executor.rs、semaphore.rs、thread_executor.rs。
  - executor.rs 依赖 config.rs、error.rs。
  - thread_executor.rs 依赖 config.rs、error.rs。
  - pool_seg.rs 依赖 crossbeam_queue::SegQueue、config.rs、executor.rs、semaphore.rs。
- 外部依赖
  - wait-timeout：用于在当前线程等待子进程并处理超时。
  - crossbeam-queue：用于无锁队列变体。
  - thiserror：用于错误派生。

```mermaid
graph LR
P["pool.rs"] --> C["config.rs"]
P --> ER["error.rs"]
P --> EM["execution_mode.rs"]
P --> E["executor.rs"]
P --> S["semaphore.rs"]
P --> TE["thread_executor.rs"]
E --> C
E --> ER
TE --> C
TE --> ER
PS["pool_seg.rs"] --> C
PS --> E
PS --> S
```

图表来源
- [src/pool.rs](file://src/pool.rs#L1-L12)
- [src/pool_seg.rs](file://src/pool_seg.rs#L1-L10)
- [src/executor.rs](file://src/executor.rs#L1-L4)
- [src/thread_executor.rs](file://src/thread_executor.rs#L1-L6)
- [src/config.rs](file://src/config.rs#L1)
- [src/error.rs](file://src/error.rs#L1)
- [src/semaphore.rs](file://src/semaphore.rs#L1)

章节来源
- [Cargo.toml](file://Cargo.toml#L6-L12)
- [src/pool.rs](file://src/pool.rs#L1-L12)
- [src/pool_seg.rs](file://src/pool_seg.rs#L1-L10)

## 性能考量
- 队列选择
  - CommandPool：基于 Mutex<VecDeque>，适用于通用场景，简单可靠。
  - CommandPoolSeg：基于 crossbeam_queue::SegQueue，多生产者场景下吞吐更高，减少锁竞争。
- 执行模式
  - Process 模式：每个任务在独立子进程中执行，隔离性强，但进程创建/销毁开销较大。
  - Thread 模式：共享内存，线程切换开销低，适合计算密集型或需要共享状态的任务。
- 并发限制
  - 使用 Semaphore 控制外部进程并发数，避免系统资源耗尽。
- 超时等待
  - execute_command 使用 wait-timeout 在当前线程等待，避免为每个任务生成额外等待线程，降低系统开销。
- 基准测试
  - 提供 push/pop、execute 等基准，可用于评估不同配置下的性能表现。

章节来源
- [README.md](file://README.md#L8-L13)
- [src/pool_seg.rs](file://src/pool_seg.rs#L11-L15)
- [src/executor.rs](file://src/executor.rs#L26-L70)
- [benches/command_pool_bench.rs](file://benches/command_pool_bench.rs#L1-L93)

## 故障排查指南
- 常见错误类型
  - ExecuteError::Io：IO 错误，通常由 spawn/wait 等系统调用失败引起。
  - ExecuteError::Timeout：命令执行超时，检查超时设置与命令本身耗时。
  - ExecuteError::Child：子进程状态异常，检查命令返回码与日志。
- 排查步骤
  - 检查命令是否存在且可执行。
  - 检查工作目录与参数是否正确。
  - 调整超时时间或并发限制。
  - 使用 is_empty() 与 pop_task() 验证队列状态。
- 相关实现
  - 错误类型定义与派生：参见 [error.rs](file://src/error.rs#L7-L18)。
  - 超时等待与错误处理：参见 [executor.rs](file://src/executor.rs#L26-L70)。

章节来源
- [src/error.rs](file://src/error.rs#L7-L18)
- [src/executor.rs](file://src/executor.rs#L26-L70)
- [tests/pool_tests.rs](file://tests/pool_tests.rs#L5-L16)

## 结论
CommandPool 提供了简洁而强大的命令队列与执行框架，支持多线程与多进程两种执行模式，并可通过自定义执行器扩展到异步运行时。其线程安全设计、并发限制与超时控制使其在实际工程中具备良好的稳定性与可扩展性。建议根据任务特性选择合适的执行模式与并发参数，并结合基准测试持续优化性能。

## 附录
- API 一览
  - new(), with_config(), execution_mode(), execution_config(), push_task(), pop_task(), is_empty(), start_executor(), start_executor_with_workers(), start_executor_with_workers_and_limit(), execute_task(), start_executor_with_executor()/with_workers_and_executor()/with_executor_and_limit()
- 相关文件
  - [README.md](file://README.md#L1-L60)
  - [examples/tokio_integration.rs](file://examples/tokio_integration.rs#L1-L62)
  - [tests/pool_tests.rs](file://tests/pool_tests.rs#L1-L90)
  - [benches/command_pool_bench.rs](file://benches/command_pool_bench.rs#L1-L93)