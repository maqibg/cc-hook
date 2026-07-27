mod channels;
mod config;
mod config_types;
mod event;
mod http;
mod legacy_config;
mod notification;
mod summarizer;

use config::Config;
use event::{EventKind, NotificationRequest, source_label};
use std::io::{Read, Seek, SeekFrom};
use std::time::{SystemTime, UNIX_EPOCH};

// 需大于 AI_TIMEOUT_MS 与渠道超时之和，否则 AI 还没返回整个进程就被砍掉
const HOOK_TIMEOUT_SECS: u64 = 200;
const TRANSCRIPT_TAIL_BYTES: u64 = 1024 * 1024;

#[derive(serde::Deserialize)]
struct HookInput {
    session_id: String,
    hook_event_name: String,
    transcript_path: Option<String>,
    notification_type: Option<String>,
    message: Option<String>,
    cwd: Option<String>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn timer_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("cc-hook-timers")
}

fn extract_last_assistant(path: &str) -> String {
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
        return String::new();
    };
    let start = length.saturating_sub(TRANSCRIPT_TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let mut content = String::from_utf8_lossy(&bytes).into_owned();
    if start > 0
        && let Some(index) = content.find('\n')
    {
        content = content[index + 1..].to_string();
    }
    for line in content.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }
        let Some(contents) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
            continue;
        };
        let texts: Vec<&str> = contents
            .iter()
            .filter(|c| c.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
            .collect();
        if !texts.is_empty() {
            return texts.join("\n");
        }
    }
    String::new()
}

fn get_duration(session_id: &str) -> Option<u64> {
    let file = timer_dir().join(timer_file_name(session_id));
    let start: u64 = std::fs::read_to_string(&file).ok()?.trim().parse().ok()?;
    let _ = std::fs::remove_file(file);
    Some(now_secs().saturating_sub(start))
}

fn save_timer(session_id: &str) {
    let dir = timer_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join(timer_file_name(session_id)),
        now_secs().to_string(),
    );
}

fn timer_file_name(session_id: &str) -> String {
    let sanitized: String = session_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(HOOK_TIMEOUT_SECS), run()).await;
    if result.is_err() {
        eprintln!("[cc-hook] 超时退出");
    }
}

async fn run() {
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() || raw.trim().is_empty() {
        return;
    }
    let Ok(input) = serde_json::from_str::<HookInput>(&raw) else {
        return;
    };

    let cfg = match Config::load("任务完成") {
        Ok(config) => config,
        Err(error) => {
            eprintln!("[cc-hook] {error}");
            return;
        }
    };
    if cfg.debug {
        eprintln!(
            "[cc-hook] 事件: {}, 会话: {}",
            input.hook_event_name, input.session_id
        );
    }

    match input.hook_event_name.as_str() {
        "UserPromptSubmit" => save_timer(&input.session_id),
        "Stop" => handle_stop(&cfg, &input).await,
        "Notification" => handle_notification(&cfg, &input).await,
        _ => {}
    }
}

async fn handle_stop(cfg: &Config, input: &HookInput) {
    // 计时文件由 UserPromptSubmit 写入；缺失说明本轮并非用户输入发起
    // （如后台子代理/后台任务完成后的自动唤醒轮），跳过所有通知
    let Some(duration) = get_duration(&input.session_id) else {
        if cfg.debug {
            eprintln!("[cc-hook] 非用户轮（无计时文件），跳过通知");
        }
        return;
    };
    if cfg.min_duration_seconds > 0 && duration < cfg.min_duration_seconds {
        return;
    }

    let content = input
        .transcript_path
        .as_deref()
        .map(extract_last_assistant)
        .unwrap_or_default();
    if content.is_empty() {
        return;
    }

    let source = source_label("Claude Code", input.cwd.as_deref(), Some(&input.session_id));
    let title = format!("{source} · 任务完成");
    let extra = Some(format!("耗时 {duration}s"));
    let report = notification::dispatch(
        cfg,
        NotificationRequest {
            event: EventKind::Complete,
            title,
            content,
            extra,
        },
    )
    .await;
    if cfg.debug {
        eprintln!(
            "[cc-hook] remote sent={}, failed={}, skipped={}",
            report.sent, report.failed, report.skipped
        );
    }
}

async fn handle_notification(cfg: &Config, input: &HookInput) {
    let (event, label) = match input.notification_type.as_deref() {
        Some("permission_prompt") => (EventKind::Confirm, "权限请求"),
        Some("idle_prompt") => (EventKind::Idle, "等待输入"),
        Some("auth_success") => (EventKind::Confirm, "认证成功"),
        Some("elicitation_dialog") => (EventKind::Elicitation, "MCP 输入"),
        Some(_) | None => (EventKind::Confirm, "通知"),
    };
    let source = source_label("Claude Code", input.cwd.as_deref(), Some(&input.session_id));
    let title = format!("{source} · {label}");
    let message = input.message.as_deref().unwrap_or("需要您的操作");
    let report = notification::dispatch(
        cfg,
        NotificationRequest {
            event,
            title,
            content: message.to_string(),
            extra: None,
        },
    )
    .await;
    if cfg.debug {
        eprintln!(
            "[cc-hook] remote sent={}, failed={}, skipped={}",
            report.sent, report.failed, report.skipped
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn timer_file_name_cannot_escape_timer_directory() {
        assert_eq!(timer_file_name("../session/path"), "___session_path");
    }

    #[test]
    fn transcript_reader_returns_last_assistant_text() {
        let path =
            std::env::temp_dir().join(format!("cc-hook-transcript-{}.jsonl", std::process::id()));
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(
            file,
            r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"first"}}]}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":"ignored"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"last"}}]}}}}"#
        )
        .unwrap();
        drop(file);
        assert_eq!(extract_last_assistant(path.to_str().unwrap()), "last");
        let _ = std::fs::remove_file(path);
    }
}
