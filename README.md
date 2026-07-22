# cc-hook

Claude Code Hook 通知系统 — 单 .exe，零依赖，冷启动 ~5ms。

## 功能

- **AI 摘要** — OpenAI 兼容 API 生成中文摘要，失败自动降级本地算法
- **多渠道通知** — Telegram / 飞书 Webhook / Windows 桌面
- **语音播报** — Windows TTS，不同事件不同语音，文本可自定义
- **多实例** — 可同时配置多个 Telegram 和飞书实例
- **代理支持** — HTTP / SOCKS5
- **10 秒超时保护** — 不会阻塞 Claude Code

## 快速开始

**方式一：下载 Release**

1. 从 [Releases](https://github.com/maqibg/cc-hook/releases) 下载 `cc-hook-windows-x64.zip`
2. 解压得到 `cc-hook.exe` 和 `notifications.example.json`
3. 将 `notifications.example.json` 复制为 `notifications.json`，填入实际配置
4. 在 `~/.claude/settings.json` 中添加 hooks 配置（见下方）

**方式二：源码编译**

```powershell
git clone https://github.com/maqibg/cc-hook.git
Set-Location cc-hook
cargo build --release
Copy-Item notifications.example.json target/release/notifications.json
# 编辑 target/release/notifications.json 填入实际配置
```

## settings.json 配置

将以下内容添加到 `~/.claude/settings.json`，路径替换为实际 exe 位置：

```json
{
  "hooks": {
    "Stop": [{"hooks": [{"type": "command", "command": "/path/to/cc-hook.exe"}]}],
    "Notification": [{"hooks": [{"type": "command", "command": "/path/to/cc-hook.exe"}]}],
    "UserPromptSubmit": [{"hooks": [{"type": "command", "command": "/path/to/cc-hook.exe"}]}]
  }
}
```

事件说明：

| 事件 | 触发时机 | 行为 |
|------|----------|------|
| `Stop` | 任务完成 | AI 摘要 + 全渠道通知 + 语音"任务完成" |
| `Notification` | 权限请求/等待输入 | 全渠道通知 + 对应语音播报 |
| `UserPromptSubmit` | 用户发送消息 | 记录时间戳（用于计算耗时） |

## .env 配置

新版配置使用 `notifications.json`，放在 exe 同目录；模板为 `notifications.example.json`。

新版配置存在时不会读取 `.env`。旧 `.env` 仍可继续使用，但仅在没有 `notifications.json` 时生效。

新版远程通知的总开关 `enabled` 和 Telegram/飞书/AI 各自开关默认关闭；本地桌面通知和语音由 `local` 独立控制。`message.include_raw` 默认关闭，避免把完整 assistant 输出发送到远程渠道。

Telegram、飞书和 AI 可以分别设置 `proxy_url`、`timeout_ms`，每个渠道支持多个实例。AI 只对 `complete` 事件生成摘要，异常时自动使用本地摘要；单个远程实例失败不影响其他实例或 Claude Code。

旧版 `.env` 配置说明仍见 `.env.example`。

**基础配置：**

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `DEBUG` | 调试日志（输出到 stderr） | `false` |
| `MIN_DURATION` | 最小通知时长（秒），低于不通知 | `0` |
| `HTTPS_PROXY` | 代理地址（Telegram 需要） | 空 |

**AI 摘要：**

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `AI_ENABLE` | 启用 AI 摘要 | `true` |
| `AI_API_KEY` | API Key | - |
| `AI_BASE_URL` | API 地址（自动补全 /v1） | `https://api.deepseek.com` |
| `AI_MODEL` | 模型名 | `deepseek-chat` |
| `AI_MAX_WORDS` | 摘要字数上限 | `500` |
| `AI_SYSTEM_PROMPT` | AI 系统提示词（定义角色和格式） | 内置默认 |
| `AI_USER_PROMPT` | AI 用户提示词（`{max_words}`/`{content}` 自动替换） | 内置默认 |

**Windows 通知与语音：**

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `WIN_NOTIFY_ENABLE` | 桌面通知 | `true` |
| `VOICE_ENABLE` | 语音播报 | `true` |
| `VOICE_STOP` | 任务完成语音 | `任务完成` |
| `VOICE_PERMISSION` | 权限请求语音 | `需要权限确认` |
| `VOICE_IDLE` | 等待输入语音 | `等待你的输入` |
| `VOICE_ELICITATION` | MCP 输入语音 | `需要输入信息` |
| `VOICE_DEFAULT` | 其他通知语音 | `需要你的操作` |

**渠道配置**（前缀索引，可配多个实例）：

```env
# Telegram：TG_1_*、TG_2_*...
TG_1_ENABLE=true
TG_1_NAME=通知1
TG_1_TOKEN=<从 @BotFather 获取>
TG_1_CHAT_ID=<从 @userinfobot 获取>

# 飞书：FS_1_*、FS_2_*...
FS_1_ENABLE=true
FS_1_NAME=飞书通知
FS_1_WEBHOOK_URL=<飞书群自定义机器人 Webhook URL>
```

## 项目结构

```
src/
├── main.rs              # Claude hook adapter、计时与 transcript 读取
├── config.rs            # notifications.json + 旧 .env 兼容
├── config_types.rs      # 配置 schema 与默认值
├── legacy_config.rs     # 旧 .env 迁移解析
├── event.rs             # 统一事件与消息模型
├── http.rs              # HTTP client 与安全响应解析
├── notification.rs      # 本地/远程分发与结果汇总
├── summarizer.rs        # AI 摘要 + 本地降级
└── channels/
    ├── mod.rs
    ├── telegram.rs      # Telegram HTML 推送
    ├── feishu.rs        # 飞书 Webhook 卡片推送
    └── windows.rs       # Windows 桌面通知 + 语音播报
```

## 故障排查

设置 `DEBUG=true` 查看详细日志：

```powershell
$env:DEBUG = 'true'
'{"session_id":"test","hook_event_name":"Notification","message":"测试"}' | & .\cc-hook.exe
```

通知配置 schema 和远程发送语义参考了 HelloAGENTS；详见 `NOTICE`。

- **Telegram 不工作** — 检查 `HTTPS_PROXY`、Token、Chat ID，确认已给 Bot 发过消息
- **AI 摘要不工作** — 检查 `AI_API_KEY` 和 `AI_BASE_URL`，失败会自动降级本地摘要
- **语音不播报** — 确认 `VOICE_ENABLE=true`，仅支持 Windows

## License

MIT
