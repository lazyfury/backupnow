use std::process::Command;

/// 发送系统通知（尽力而为，失败静默）
pub fn notify(title: &str, message: &str) {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification {} with title {}",
            applescript_string(&truncate(message, 220)),
            applescript_string(title)
        );
        let _ = Command::new("osascript").arg("-e").arg(script).output();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (title, message);
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() <= max {
        s
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}…")
    }
}

#[cfg(target_os = "macos")]
fn applescript_string(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
