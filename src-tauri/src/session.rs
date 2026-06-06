// Session Manager — save/restore workspace layouts, scrollback, and agent resume
// Saves to: %LOCALAPPDATA%\wmux\session.json  (Windows)
//           ~/.local/share/wmux/session.json   (Linux)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionSnapshot {
    pub version: u32,
    pub timestamp: String,
    pub workspaces: Vec<WorkspaceSnapshot>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkspaceSnapshot {
    pub name: String,
    pub directory: String,
    pub git_branch: Option<String>,
    pub panes: Vec<PaneSnapshot>,
    pub browser_urls: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PaneSnapshot {
    pub cwd: String,
    pub split_direction: String,
    pub size_ratio: f32,
    pub agent_resume: Option<AgentResumeInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentResumeInfo {
    pub agent_name: String,
    pub session_id: Option<String>,
    pub resume_command: String,
}

fn session_dir() -> PathBuf {
    // Windows: C:\Users\<user>\AppData\Local\wmux
    // Linux:   ~/.local/share/wmux
    if cfg!(target_os = "windows") {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("wmux")
    } else {
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".local")
                    .join("share")
            })
            .join("wmux")
    }
}

fn session_path() -> PathBuf {
    session_dir().join("session.json")
}

#[tauri::command]
pub fn save_session() -> Result<String, String> {
    // Gather current state from workspace manager
    let workspaces = crate::workspace::list_workspaces_internal();

    let snapshot = SessionSnapshot {
        version: 1,
        timestamp: chrono::Utc::now().to_rfc3339(),
        workspaces: workspaces
            .iter()
            .map(|ws| WorkspaceSnapshot {
                name: ws.name.clone(),
                directory: ws.directory.clone(),
                git_branch: ws.git_branch.clone(),
                panes: vec![], // Phase 2: capture actual pane layout
                browser_urls: vec![],
            })
            .collect(),
    };

    let dir = session_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir failed: {}", e))?;

    let path = session_path();
    let json =
        serde_json::to_string_pretty(&snapshot).map_err(|e| format!("serialize failed: {}", e))?;

    std::fs::write(&path, &json).map_err(|e| format!("write failed: {}", e))?;

    log::info!("[session] Saved to {:?}", path);
    Ok(format!("Session saved ({} workspaces)", snapshot.workspaces.len()))
}

#[tauri::command]
pub fn load_session() -> Result<Option<SessionSnapshot>, String> {
    let path = session_path();

    if !path.exists() {
        return Ok(None);
    }

    let json = std::fs::read_to_string(&path).map_err(|e| format!("read failed: {}", e))?;

    let snapshot: SessionSnapshot =
        serde_json::from_str(&json).map_err(|e| format!("parse failed: {}", e))?;

    log::info!(
        "[session] Loaded from {:?} ({} workspaces)",
        path,
        snapshot.workspaces.len()
    );
    Ok(Some(snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_snapshot_serialization() {
        let snapshot = SessionSnapshot {
            version: 1,
            timestamp: "2026-06-06T00:00:00Z".to_string(),
            workspaces: vec![WorkspaceSnapshot {
                name: "test".to_string(),
                directory: "/tmp".to_string(),
                git_branch: Some("main".to_string()),
                panes: vec![],
                browser_urls: vec![],
            }],
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: SessionSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.workspaces.len(), 1);
        assert_eq!(parsed.workspaces[0].name, "test");
    }
}
