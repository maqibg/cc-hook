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
