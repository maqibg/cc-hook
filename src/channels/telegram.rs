use crate::config::Channel;

/// MarkdownV2 转义
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        if "_*[]()~`>#+-=|{}.!\\".contains(c) { out.push('\\'); }
        out.push(c);
    }
    out
}

/// 代码块内转义（只转义 ` 和 \）
fn escape_code(s: &str) -> String {
    s.replace('\\', "\\\\").replace('`', "\\`")
}

pub async fn send(client: &reqwest::Client, ch: &Channel, title: &str, summary: &str, raw: Option<&str>, extra: Option<&str>) -> Result<(), String> {
    let token = ch.token.as_deref().ok_or("missing token")?;
    let chat_id = ch.chat_id.as_deref().ok_or("missing chat_id")?;

    let quoted = escape(summary).replace('\n', "\n>");
    let mut text = format!("*{}*\n\n*AI 摘要：*\n>{}", escape(title), quoted);
    if let Some(r) = raw {
        let truncated: String = r.chars().take(500).collect();
        text.push_str(&format!("\n\n*原始输出：*\n```\n{}\n```", escape_code(&truncated)));
    }
    if let Some(e) = extra {
        text.push_str(&format!("\n\n{}", escape(e)));
    }

    let resp = client.post(format!("https://api.telegram.org/bot{token}/sendMessage"))
        .json(&serde_json::json!({ "chat_id": chat_id, "text": text, "parse_mode": "MarkdownV2" }))
        .send().await.map_err(|e| e.to_string())?;

    let data = resp.json::<serde_json::Value>().await.map_err(|e| e.to_string())?;
    if data.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return Err(format!("API error: {data}"));
    }
    Ok(())
}
