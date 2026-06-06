// Wave AI Bridge — connects wmux to your existing Wave AI shell
//
// Three modes:
// 1. Filesystem bridge: watch a dir for request/response JSON files
// 2. CLI bridge: wmux commands Wave AI can call
// 3. HTTP API bridge: localhost REST endpoints (Phase 3)
//
// Your Wave AI writes a request → wmux processes → writes response
// Wave AI reads the response and acts on it

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaveBridgeStatus {
    pub mode: String,       // "filesystem", "cli", "http", "disabled"
    pub bridge_dir: Option<String>,
    pub api_port: Option<u16>,
    pub connected: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaveRequest {
    pub id: String,
    pub command: String,
    pub params: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaveResponse {
    pub id: String,
    pub success: bool,
    pub data: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TerminalCapture {
    pub pane_id: String,
    pub workspace: String,
    pub agent: Option<String>,
    pub status: String,
    pub last_lines: Vec<String>,
}

// ===== Commands =====

#[tauri::command]
pub fn wave_bridge_status() -> Result<WaveBridgeStatus, String> {
    // Phase 2: read from config file or env var
    Ok(WaveBridgeStatus {
        mode: "disabled".to_string(),
        bridge_dir: None,
        api_port: None,
        connected: false,
    })
}

#[tauri::command]
pub fn wave_send_command(
    workspace_id: String,
    pane_id: Option<String>,
    command: String,
) -> Result<String, String> {
    // Phase 2: actually send to the target pane's PTY
    log::info!(
        "[wave] Send command to {} (pane {:?}): {}",
        workspace_id,
        pane_id,
        command
    );
    Ok("sent".to_string())
}

#[tauri::command]
pub fn wave_ask_all(query: String, lines: Option<usize>) -> Result<Vec<TerminalCapture>, String> {
    let _lines = lines.unwrap_or(50);

    // Phase 2: capture scrollback from all active PTYs
    // For now return the workspace list as terminal captures
    let workspaces = crate::workspace::list_workspaces_internal();

    let captures: Vec<TerminalCapture> = workspaces
        .iter()
        .map(|ws| {
            let agent_name = ws.agents.first().map(|a| a.name.clone());
            let agent_status = ws
                .agents
                .first()
                .map(|a| a.status.clone())
                .unwrap_or_else(|| "idle".to_string());

            TerminalCapture {
                pane_id: format!("{}-main", ws.id),
                workspace: ws.name.clone(),
                agent: agent_name,
                status: agent_status,
                last_lines: vec![format!("(query: {})", query)],
            }
        })
        .collect();

    log::info!("[wave] Ask all ({} workspaces): {}", captures.len(), query);
    Ok(captures)
}

// ===== Filesystem Bridge =====
// Phase 2: implement with notify crate
//
// pub async fn start_filesystem_bridge(bridge_dir: &str, app_handle: tauri::AppHandle) {
//     use notify::{Watcher, RecursiveMode, recommended_watcher};
//     use std::sync::mpsc;
//
//     let (tx, rx) = mpsc::channel();
//     let mut watcher = recommended_watcher(tx).unwrap();
//     watcher.watch(bridge_dir.as_ref(), RecursiveMode::NonRecursive).unwrap();
//
//     loop {
//         match rx.recv() {
//             Ok(event) => {
//                 // Check for new req-*.json files
//                 // Process the request
//                 // Write res-*.json response
//             }
//             Err(e) => log::error!("[wave] Watch error: {}", e),
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_status() {
        let status = wave_bridge_status().unwrap();
        assert_eq!(status.mode, "disabled");
    }

    #[test]
    fn test_wave_request_serialization() {
        let req = WaveRequest {
            id: "req-123".to_string(),
            command: "scan_terminals".to_string(),
            params: serde_json::json!({"workspace": "kucoin-lane", "lines": 50}),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("scan_terminals"));
    }
}
