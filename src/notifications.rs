use anyhow::Result;
use tracing::info;

pub fn send_notification(sender_name: &str, message_preview: &str) -> Result<()> {
    info!(
        "Creating notification: sender={}, preview_len={}",
        sender_name,
        message_preview.len()
    );

    let preview = if message_preview.len() > 100 {
        format!("{}...", &message_preview[..100])
    } else {
        message_preview.to_string()
    };

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let script = format!(
            r#"display notification "{}" with title "Signal TUI" subtitle "{}" sound name "default""#,
            preview.replace('"', r#"\""#),
            sender_name.replace('"', r#"\""#)
        );

        Command::new("osascript").arg("-e").arg(&script).output()?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        use notify_rust::Notification;

        Notification::new()
            .summary(sender_name)
            .body(&preview)
            .appname("Signal TUI")
            .timeout(5000)
            .show()?;
    }

    Ok(())
}
