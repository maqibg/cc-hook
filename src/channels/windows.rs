use std::process::Command;

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

/// 语音播报，不同事件不同文本
pub fn speak(event: &str, notification_type: Option<&str>) {
    let text = match event {
        "Stop" => "任务完成",
        "Notification" => match notification_type {
            Some("permission_prompt") => "需要权限确认",
            Some("idle_prompt") => "等待你的输入",
            Some("elicitation_dialog") => "需要输入信息",
            _ => "需要你的操作",
        },
        _ => return,
    };
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
