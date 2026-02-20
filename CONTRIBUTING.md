# 贡献指南

感谢您对 execute 项目的关注！我们欢迎各种形式的贡献，包括但不限于：

- 报告问题
- 提交功能请求
- 改进文档
- 提交代码修复
- 添加新功能

## 如何贡献

### 报告问题

如果您发现了 bug 或有功能建议，请通过 GitHub Issues 提交。提交时请包含：

1. 问题的清晰描述
2. 复现步骤（如果是 bug）
3. 期望的行为
4. 实际的行为
5. 环境信息（操作系统、Rust 版本等）
6. 相关的代码片段或错误日志

### 提交代码

1. **Fork 仓库** - 将项目 fork 到您的 GitHub 账户
2. **创建分支** - 从 `master` 分支创建您的功能分支
   ```bash
   git checkout -b feature/your-feature-name
   ```
3. **编写代码** - 遵循我们的代码规范
4. **运行测试** - 确保所有测试通过
   ```bash
   cargo test --all
   cargo clippy --all-targets --all-features -- -D warnings
   cargo fmt --all -- --check
   ```
5. **提交更改** - 使用清晰的提交信息
   ```bash
   git commit -m "feat: 添加新功能"
   ```
6. **推送分支** - 推送到您的 fork
   ```bash
   git push origin feature/your-feature-name
   ```
7. **创建 Pull Request** - 在 GitHub 上创建 PR

### 代码规范

- **格式化**：使用 `cargo fmt` 格式化代码
- **Lint**：确保 `cargo clippy` 没有警告
- **测试**：为新功能添加测试，确保所有测试通过
- **文档**：为公共 API 添加文档注释
- **提交信息**：遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范

#### 提交信息格式

```
<type>(<scope>): <subject>

<body>

<footer>
```

**类型 (type)**：
- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 仅文档更改
- `style`: 不影响代码含义的更改（格式化、分号等）
- `refactor`: 既不修复 bug 也不添加功能的代码更改
- `perf`: 提升性能的代码更改
- `test`: 添加或修正测试
- `chore`: 构建过程或辅助工具的更改

**示例**：
```
feat: 添加 pipeline 功能

- 支持命令管道执行
- 添加 PipelineBuilder 链式调用
- 包含完整测试
```

### 开发环境

**要求**：
- Rust 1.70+ 
- Cargo

**构建**：
```bash
cargo build --release
```

**运行测试**：
```bash
cargo test --all
```

**运行示例**：
```bash
cargo run --example tokio_integration
```

### 项目结构

```
src/
├── backend.rs      # 执行后端抽象层
├── config.rs       # 命令配置
├── error.rs        # 错误类型
├── executor.rs     # 执行器 trait 和实现
├── lib.rs          # 库入口
├── main.rs         # 可执行文件入口
├── pipeline.rs     # 管道功能
├── pool.rs         # 命令池（Mutex 版本）
├── pool_seg.rs     # 命令池（无锁版本）
├── process_pool.rs # 进程池实现
├── semaphore.rs    # 信号量实现
├── task_handle.rs  # 任务结果获取
└── task_status.rs  # 任务状态追踪

tests/              # 集成测试
examples/           # 示例代码
benches/            # 性能基准测试
```

## 代码审查

所有提交都需要通过代码审查。维护者会：

- 检查代码质量和风格
- 确保测试覆盖
- 验证文档完整性
- 确认 CI 检查通过

## 发布流程

1. 更新 `CHANGELOG.md`
2. 更新 `Cargo.toml` 中的版本号
3. 创建 git tag
4. 发布到 crates.io

## 行为准则

本项目遵循 [行为准则](CODE_OF_CONDUCT.md)。参与本项目即表示您同意遵守这些条款。

## 许可证

通过贡献代码，您同意您的贡献将在 MIT 许可证下发布。

## 联系方式

如有问题，请通过 GitHub Issues 联系我们。

再次感谢您的贡献！
