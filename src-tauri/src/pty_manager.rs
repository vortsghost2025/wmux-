// PTY Manager — spawns and manages pseudo-terminals
// Uses conpty on Windows, Unix PTY on Linux/Mac via portable-pty
//
// Current: stub implementation for UI development
// Phase 2: uncomment portable-pty code, add to Cargo.toml deps

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[cfg(target_os = "windows")]
const DEFAULT_SHELL: &str = "powershell.exe";
#[cfg(not(target_os = "windows"))]
const DEFAULT_SHELL: &str = "/bin/bash";

// ===== Types =====

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PtyInfo {
    pub id: String,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    pub alive: bool,
    pub shell: String,
}

// ===== Global State =====

static PTYS: Lazy<Mutex<HashMap<String, PtyInfo>>> = Lazy::new(|| Mutex::new(HashMap::new()));

// ===== Commands =====

#[tauri::command]
pub fn create_pty(cwd: String, cols: u16, rows: u16) -> Result<PtyInfo, String> {
    let id = Uuid::new_v4().to_string();

    // === Phase 2: Real PTY (uncomment when portable-pty is added) ===
    //
    // use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    //
    // let pty_system = native_pty_system();
    // let pair = pty_system.openpty(PtySize {
    //     rows, cols, pixel_width: 0, pixel_height: 0,
    // }).map_err(|e| format!("Failed to open PTY: {}", e))?;
    //
    // let mut cmd = CommandBuilder::new(DEFAULT_SHELL);
    // cmd.cwd(&cwd);
    // cmd.env("WMUX_PANE_ID", &id);
    // cmd.env("WMUX", "1");
    // cmd.env("TERM", "xterm-256color");
    //
    // let child = pair.slave.spawn_command(cmd)
    //     .map_err(|e| format!("Failed to spawn: {}", e))?;

    let info = PtyInfo {
        id: id.clone(),
        cwd,
        cols,
        rows,
        alive: true,
        shell: DEFAULT_SHELL.to_string(),
    };

    let mut ptys = PTYS.lock().map_err(|e| e.to_string())?;
    ptys.insert(id.clone(), info.clone());

    log::info!("[pty] Spawned {} ({} {}x{})", id, DEFAULT_SHELL, cols, rows);
    Ok(info)
}

#[tauri::command]
pub fn write_pty(pty_id: String, data: String) -> Result<(), String> {
    let ptys = PTYS.lock().map_err(|e| e.to_string())?;
    let pty = ptys
        .get(&pty_id)
        .ok_or_else(|| format!("PTY {} not found", pty_id))?;

    if !pty.alive {
        return Err(format!("PTY {} is not alive", pty_id));
    }

    // Phase 2: writer.write_all(data.as_bytes())
    log::debug!("[pty] Write to {}: {} bytes", pty_id, data.len());
    Ok(())
}

#[tauri::command]
pub fn resize_pty(pty_id: String, cols: u16, rows: u16) -> Result<(), String> {
    let mut ptys = PTYS.lock().map_err(|e| e.to_string())?;
    let pty = ptys
        .get_mut(&pty_id)
        .ok_or_else(|| format!("PTY {} not found", pty_id))?;

    pty.cols = cols;
    pty.rows = rows;

    // Phase 2: pair.master.resize(PtySize { rows, cols, ... })
    log::debug!("[pty] Resize {}: {}x{}", pty_id, cols, rows);
    Ok(())
}

#[tauri::command]
pub fn close_pty(pty_id: String) -> Result<(), String> {
    let mut ptys = PTYS.lock().map_err(|e| e.to_string())?;
    ptys.remove(&pty_id)
        .ok_or_else(|| format!("PTY {} not found", pty_id))?;

    log::info!("[pty] Closed {}", pty_id);
    Ok(())
}

#[tauri::command]
pub fn list_ptys() -> Result<Vec<PtyInfo>, String> {
    let ptys = PTYS.lock().map_err(|e| e.to_string())?;
    Ok(ptys.values().cloned().collect())
}

// ===== OSC Parser =====
// Detects notification sequences from terminal output:
// OSC 9 (iTerm2 growl), OSC 99 (kitty), OSC 777 (rxvt)
// Also detects agent "waiting for input" patterns

pub fn parse_osc(data: &[u8]) -> Option<(String, String)> {
    let text = String::from_utf8_lossy(data);

    // OSC 9 ; <message> ST
    if let Some(start) = text.find("\x1b]9;") {
        if let Some(end) = text[start..].find('\x07') {
            let msg = &text[start + 4..start + end];
            return Some(("Terminal".to_string(), msg.to_string()));
        }
    }

    // OSC 777 ; notify ; <title> ; <body> ST
    if let Some(start) = text.find("\x1b]777;notify;") {
        if let Some(end) = text[start..].find('\x07') {
            let payload = &text[start + 13..start + end];
            let parts: Vec<&str> = payload.splitn(2, ';').collect();
            return Some((
                parts.first().unwrap_or(&"").to_string(),
                parts.get(1).unwrap_or(&"").to_string(),
            ));
        }
    }

    // Agent waiting heuristics
    let waiting_patterns = [
        "Waiting for your input",
        "waiting for input",
        "needs your approval",
        "Press Enter to continue",
        "? (y/n)",
    ];
    for pattern in &waiting_patterns {
        if text.contains(pattern) {
            return Some((
                "Agent Waiting".to_string(),
                "Agent needs your input".to_string(),
            ));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_osc9() {
        let data = b"\x1b]9;Build complete\x07";
        let result = parse_osc(data);
        assert!(result.is_some());
        let (title, body) = result.unwrap();
        assert_eq!(title, "Terminal");
        assert_eq!(body, "Build complete");
    }

    #[test]
    fn test_parse_agent_waiting() {
        let data = b"Claude is Waiting for your input...";
        let result = parse_osc(data);
        assert!(result.is_some());
        let (title, _) = result.unwrap();
        assert_eq!(title, "Agent Waiting");
    }

    #[test]
    fn test_parse_no_match() {
        let data = b"regular terminal output";
        assert!(parse_osc(data).is_none());
    }

    #[test]
    fn test_create_and_list_pty() {
        let pty = create_pty("/tmp".to_string(), 80, 24).unwrap();
        assert!(pty.alive);
        assert_eq!(pty.cols, 80);
        assert_eq!(pty.rows, 24);
    }
}
