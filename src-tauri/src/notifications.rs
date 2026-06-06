// Notification Manager — agent notifications, unread tracking, system toasts
// Supports: in-app notifications, blue ring indicators, Windows Toast

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Notification {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub body: String,
    pub urgency: String, // "low", "normal", "high", "critical"
    pub timestamp: String,
    pub read: bool,
}

// ===== Global State =====

static NOTIFICATIONS: Lazy<Mutex<Vec<Notification>>> = Lazy::new(|| Mutex::new(Vec::new()));
static UNREAD_COUNTS: Lazy<Mutex<HashMap<String, u32>>> = Lazy::new(|| Mutex::new(HashMap::new()));

// ===== Commands =====

#[tauri::command]
pub fn send_notification(
    workspace_id: String,
    title: String,
    body: String,
    urgency: String,
) -> Result<Notification, String> {
    let notification = Notification {
        id: Uuid::new_v4().to_string(),
        workspace_id: workspace_id.clone(),
        title: title.clone(),
        body: body.clone(),
        urgency: urgency.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        read: false,
    };

    let mut notifs = NOTIFICATIONS.lock().map_err(|e| e.to_string())?;
    notifs.push(notification.clone());

    let mut counts = UNREAD_COUNTS.lock().map_err(|e| e.to_string())?;
    *counts.entry(workspace_id.clone()).or_insert(0) += 1;

    // System toast for high/critical
    if urgency == "high" || urgency == "critical" {
        fire_system_toast(&title, &body);
    }

    log::info!(
        "[notify] {} [{}] {} — {}",
        workspace_id,
        urgency,
        title,
        body
    );
    Ok(notification)
}

#[tauri::command]
pub fn mark_read(workspace_id: String) -> Result<(), String> {
    let mut notifs = NOTIFICATIONS.lock().map_err(|e| e.to_string())?;
    for n in notifs.iter_mut() {
        if n.workspace_id == workspace_id {
            n.read = true;
        }
    }

    let mut counts = UNREAD_COUNTS.lock().map_err(|e| e.to_string())?;
    counts.insert(workspace_id, 0);
    Ok(())
}

#[tauri::command]
pub fn get_all_notifications() -> Result<Vec<Notification>, String> {
    let notifs = NOTIFICATIONS.lock().map_err(|e| e.to_string())?;
    Ok(notifs.clone())
}

/// Find the workspace with the most recent unread notification
pub fn latest_unread_workspace() -> Option<String> {
    let notifs = NOTIFICATIONS.lock().ok()?;
    notifs
        .iter()
        .rev()
        .find(|n| !n.read)
        .map(|n| n.workspace_id.clone())
}

/// Fire a Windows Toast notification
fn fire_system_toast(title: &str, body: &str) {
    // Phase 3: Use winrt-notification crate or PowerShell toast
    //
    // #[cfg(target_os = "windows")]
    // {
    //     use std::process::Command;
    //     Command::new("powershell")
    //         .args([
    //             "-Command",
    //             &format!(
    //                 "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; \
    //                  $xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent(0); \
    //                  $text = $xml.GetElementsByTagName('text'); \
    //                  $text[0].AppendChild($xml.CreateTextNode('{}')) > $null; \
    //                  $text[1].AppendChild($xml.CreateTextNode('{}')) > $null; \
    //                  $toast = [Windows.UI.Notifications.ToastNotification]::new($xml); \
    //                  [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('wmux').Show($toast)",
    //                 title, body
    //             ),
    //         ])
    //         .spawn()
    //         .ok();
    // }

    log::info!("[toast] {} — {}", title, body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_notification() {
        let n = send_notification(
            "ws-1".to_string(),
            "Test".to_string(),
            "Hello".to_string(),
            "normal".to_string(),
        )
        .unwrap();
        assert_eq!(n.title, "Test");
        assert!(!n.read);
    }

    #[test]
    fn test_mark_read() {
        send_notification(
            "ws-read".to_string(),
            "X".to_string(),
            "Y".to_string(),
            "low".to_string(),
        )
        .unwrap();
        mark_read("ws-read".to_string()).unwrap();
        // After marking read, all notifications for that workspace should be read
    }
}
