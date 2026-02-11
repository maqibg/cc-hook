# cc-hook

Claude Code Hook 通知系统 — 单 .exe，零依赖，冷启动 ~5ms。

## 功能

- **AI 摘要**：OpenAI 兼容 API 生成中文摘要，失败自动降级本地算法
- **多渠道通知**：Telegram（MarkdownV2）/ 飞书 Webhook / Windows 桌面
- **多实例**：可同时配置多个 Telegram 和飞书实例
- **代理支持**：HTTP / SOCKS5（Telegram API 需要）
- **10 秒超时保护**：不会阻塞 Claude Code

## 快速开始

```bash
cd D:/Code/codeSpace/Notice/cc-hook

# 1. 编译
cargo build --release

# 2. 配置（将 .env.example 复制到 exe 同目录）
cp .env.example target/release/.env
# 编辑 target/release/.env 填入实际配置

# 3. 在 ~/.claude/settings.json 中添加 hooks 配置（见下方）

# 4. Claude Code 中执行 /hooks 批准，重启生效
```

## settings.json 配置

将以下内容添加到 `~/.claude/settings.json`：

```json
{
  "hooks": {
    "Stop": [{"hooks": [{"type": "command", "command": "D:/Code/codeSpace/Notice/cc-hook/target/release/cc-hook.exe"}]}],
    "Notification": [{"hooks": [{"type": "command", "command": "D:/Code/codeSpace/Notice/cc-hook/target/release/cc-hook.exe"}]}],
    "UserPromptSubmit": [{"hooks": [{"type": "command", "command": "D:/Code/codeSpace/Notice/cc-hook/target/release/cc-hook.exe"}]}]
  }
}
```

三个事件说明：
- `Stop` — 任务完成，生成 AI 摘要并推送通知
- `Notification` — 权限请求/等待输入时推送提醒
- `UserPromptSubmit` — 记录时间戳，用于计算任务耗时

## .env 配置

`.env` 文件放在 exe 同目录（`target/release/.env`），详见 `.env.example`。

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `DEBUG` | 调试日志（输出到 stderr） | `false` |
| `MIN_DURATION` | 最小通知时长（秒），低于不通知 | `0` |
| `HTTPS_PROXY` | 代理地址（Telegram 需要） | 空 |
| `AI_ENABLE` | 启用 AI 摘要 | `true` |
| `AI_API_KEY` | API Key | - |
| `AI_BASE_URL` | API 地址 | `https://api.deepseek.com` |
| `AI_MODEL` | 模型名 | `deepseek-chat` |
| `AI_MAX_WORDS` | 摘要最大字数 | `100` |
| `WIN_NOTIFY_ENABLE` | Windows 桌面通知 | `true` |

渠道配置使用前缀索引，可配多个实例：

```env
# Telegram 实例：TG_1_*、TG_2_*、TG_3_*...
TG_1_ENABLE=true
TG_1_NAME=通知1
TG_1_TOKEN=<从 @BotFather 获取>
TG_1_CHAT_ID=<从 @userinfobot 获取>

# 飞书实例：FS_1_*、FS_2_*...
FS_1_ENABLE=true
FS_1_NAME=飞书通知
FS_1_WEBHOOK_URL=<飞书群自定义机器人 Webhook URL>
```

## 项目结构

```
src/
├── main.rs              # 入口（读 stdin → 分发事件）
├── config.rs            # 配置解析（.env + 多实例渠道）
├── summarizer.rs        # AI 摘要 + 本地降级
└── channels/
    ├── mod.rs
    ├── telegram.rs      # Telegram MarkdownV2 推送
    ├── feishu.rs        # 飞书 Webhook 卡片推送
    └── windows.rs       # Windows 桌面通知
```

## 故障排查

设置 `DEBUG=true` 查看详细日志：

```bash
echo '{"session_id":"test","hook_event_name":"Notification","message":"测试"}' | DEBUG=true ./target/release/cc-hook.exe
```

### Telegram 不工作
1. 确认 `HTTPS_PROXY` 代理配置正确
2. 确认 Token 和 Chat ID 正确
3. 确认已给 Telegram 机器人发过消息激活对话

### AI 摘要不工作
1. 确认 `AI_API_KEY` 和 `AI_BASE_URL` 正确
2. 失败会自动降级为本地摘要，不影响通知

## License

MIT
