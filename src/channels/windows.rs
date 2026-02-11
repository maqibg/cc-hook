use std::process::Command;
use crate::config::Config;

pub fn notify(title: &str, message: &str) {
    let msg = if message.chars().count() > 200 {
        format!("{}...", message.chars().take(200).collect::<String>())
    } else {
        message.to_string()
    };
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(&msg)
        .appname("cc-hook")
        .timeout(notify_rust::Timeout::Milliseconds(10000))
        .show();
}

/// 语音播报，从配置读取文本
pub fn speak(cfg: &Config, event: &str, notification_type: Option<&str>) {
    if !cfg.voice_enable { return; }
    let text = match event {
        "Stop" => &cfg.voice_stop,
        "Notification" => match notification_type {
            Some("permission_prompt") => &cfg.voice_permission,
            Some("idle_prompt") => &cfg.voice_idle,
            Some("elicitation_dialog") => &cfg.voice_elicitation,
            _ => &cfg.voice_default,
        },
        _ => return,
    };
    if text.is_empty() { return; }
    let script = format!(
        "Add-Type -AssemblyName System.Speech; (New-Object System.Speech.Synthesis.SpeechSynthesizer).Speak('{text}')"
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
