# 安全政策

## 支持的版本

| 版本 | 支持状态 |
|------|----------|
| 0.1.x | ✅ 当前支持 |

## 报告安全漏洞

如果您发现了安全漏洞，请 **不要** 通过 GitHub Issues 公开报告。

相反，请通过以下方式私下报告：

1. **发送邮件** - 发送邮件至 [维护者邮箱]（请替换为实际邮箱）
2. **主题** - 使用 "[SECURITY] execute 安全漏洞报告" 作为主题
3. **内容** - 包含以下信息：
   - 漏洞的详细描述
   - 复现步骤
   - 可能的影响
   - 建议的修复方案（如果有）

## 响应流程

1. **确认** - 我们将在 48 小时内确认收到您的报告
2. **评估** - 我们将在 7 天内评估漏洞的严重性
3. **修复** - 我们将开发修复方案，并与您协调披露时间
4. **披露** - 修复后，我们将发布安全公告并致谢报告者

## 安全最佳实践

使用本库时，请遵循以下安全建议：

### 命令注入防护

- **永远不要** 直接将用户输入拼接到命令中
- 使用 `CommandConfig` 的 `args` 字段传递参数，而不是字符串拼接

```rust
// ❌ 不安全 - 存在命令注入风险
let user_input = "; rm -rf /";
let cmd = format!("echo {}", user_input);

// ✅ 安全 - 参数正确转义
let config = CommandConfig::new("echo", vec![user_input.to_string()]);
```

### 超时设置

- 始终为可能长时间运行的命令设置超时
- 避免使用无限制的超时（`None`）处理不受信任的输入

```rust
// ✅ 推荐 - 设置合理的超时
let config = CommandConfig::new("process_data", vec![file_path])
    .with_timeout(Duration::from_secs(30));
```

### 工作目录限制

- 限制命令的工作目录，防止访问敏感文件

```rust
// ✅ 推荐 - 限制在特定目录
let config = CommandConfig::new("script.sh", vec![])
    .with_working_dir("/safe/directory");
```

### 并发控制

- 使用信号量限制并发执行数量，防止资源耗尽攻击

```rust
// ✅ 推荐 - 限制并发数
let config = ExecutionConfig::new()
    .with_concurrency_limit(10);
```

## 已知限制

- 进程池模式使用 IPC 通信，理论上存在序列化攻击风险（建议使用可信命令）
- Pipeline 功能将命令输出传递给下一个命令，注意数据验证

## 安全更新

安全更新将通过以下渠道发布：

1. GitHub Security Advisories
2. 项目 Releases 页面
3. crates.io 更新

建议订阅仓库通知以及时获取安全更新。

## 致谢

感谢以下安全研究人员对项目安全的贡献：

- [待添加]

---

最后更新：2024年
