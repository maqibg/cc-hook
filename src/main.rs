mod config;
mod summarizer;
mod channels;

use config::Config;
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(serde::Deserialize)]
struct HookInput {
    session_id: String,
    hook_event_name: String,
    transcript_path: Option<String>,
    notification_type: Option<String>,
    message: Option<String>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn timer_dir() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_default().join(".claude").join("cc-hook-timers")
}

fn extract_last_assistant(path: &str) -> String {
    let Ok(content) = std::fs::read_to_string(path) else { return String::new() };
    for line in content.lines().rev() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") { continue; }
        let Some(contents) = v.pointer("/message/content").and_then(|c| c.as_array()) else { continue };
        let texts: Vec<&str> = contents.iter()
            .filter(|c| c.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
            .collect();
        if !texts.is_empty() { return texts.join("\n"); }
    }
    String::new()
}

fn get_duration(session_id: &str) -> Option<u64> {
    let file = timer_dir().join(session_id);
    let start: u64 = std::fs::read_to_string(file).ok()?.trim().parse().ok()?;
    Some(now_secs() - start)
}

fn save_timer(session_id: &str) {
    let dir = timer_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(session_id), now_secs().to_string());
}

fn now_time_str() -> String {
    let total = now_secs() + 8 * 3600; // UTC+8
    let h = (total % 86400) / 3600;
    let m = (total % 3600) / 60;
    format!("{:02}:{:02}", h, m)
}

async fn dispatch(cfg: &Config, client: &reqwest::Client, title: &str, summary: &str, raw: Option<&str>, extra: Option<&str>) {
    let mut tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    if cfg.win_notify_enable {
        let t = title.to_string();
        let s = summary.to_string();
        tasks.push(tokio::spawn(async move { channels::windows::notify(&t, &s); }));
    }

    for ch in &cfg.channels {
        let client = client.clone();
        let t = title.to_string();
        let s = summary.to_string();
        let r = raw.map(|s| s.to_string());
        let e = extra.map(|s| s.to_string());
        let ch = ch.clone();
        tasks.push(tokio::spawn(async move {
            let result = match ch.ch_type.as_str() {
                "telegram" => channels::telegram::send(&client, &ch, &t, &s, r.as_deref(), e.as_deref()).await,
                "feishu" => channels::feishu::send(&client, &ch, &t, &s, r.as_deref(), e.as_deref()).await,
                _ => Ok(()),
            };
            if let Err(e) = result { eprintln!("[cc-hook] {} {} 失败: {}", ch.ch_type, ch.name, e); }
        }));
    }

    for t in tasks { let _ = t.await; }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), run()).await;
    if result.is_err() { eprintln!("[cc-hook] 超时退出"); }
}

async fn run() {
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() || raw.trim().is_empty() { return; }
    let Ok(input) = serde_json::from_str::<HookInput>(&raw) else { return };

    let cfg = Config::load();
    if cfg.debug { eprintln!("[cc-hook] 事件: {}, 会话: {}", input.hook_event_name, input.session_id); }

    match input.hook_event_name.as_str() {
        "UserPromptSubmit" => save_timer(&input.session_id),
        "Stop" => handle_stop(&cfg, &input).await,
        "Notification" => handle_notification(&cfg, &input).await,
        _ => {}
    }
}

async fn handle_stop(cfg: &Config, input: &HookInput) {
    let duration = get_duration(&input.session_id);
    if cfg.min_duration > 0 {
        if let Some(d) = duration {
            if d < cfg.min_duration { return; }
        }
    }

    let content = input.transcript_path.as_deref().map(extract_last_assistant).unwrap_or_default();
    if content.is_empty() { return; }

    let proxy_client = config::build_http_client(&cfg.proxy);
    let ai_client = config::build_http_client("");
    let summary = summarizer::generate(cfg, &ai_client, &content).await;
    let title = format!("Claude Code 完成 ({})", now_time_str());
    let extra = duration.map(|d| format!("耗时 {}s", d));
    // 原始输出截取前 500 字符
    let raw: String = content.trim().chars().take(500).collect();
    dispatch(cfg, &proxy_client, &title, &summary, Some(&raw), extra.as_deref()).await;
}

async fn handle_notification(cfg: &Config, input: &HookInput) {
    let label = match input.notification_type.as_deref() {
        Some("permission_prompt") => "权限请求",
        Some("idle_prompt") => "等待输入",
        Some("auth_success") => "认证成功",
        Some("elicitation_dialog") => "MCP 输入",
        Some(other) => other,
        None => "通知",
    };
    let title = format!("Claude Code {label}");
    let message = input.message.as_deref().unwrap_or("需要您的操作");
    let client = config::build_http_client(&cfg.proxy);
    dispatch(cfg, &client, &title, message, None, None).await;
}
