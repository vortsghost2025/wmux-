use once_cell::sync::Lazy;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::notifications;
use crate::workspace;

#[cfg(target_os = "windows")]
const DEFAULT_SHELL: &str = "powershell.exe";
#[cfg(not(target_os = "windows"))]
const DEFAULT_SHELL: &str = "/bin/bash";

const READ_BUF_SIZE: usize = 4096;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PtyInfo {
    pub id: String,
    pub workspace_id: String,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    pub alive: bool,
    pub shell: String,
}

#[derive(Clone, Serialize)]
struct PtyDataPayload {
    pty_id: String,
    data: String,
}

#[derive(Clone, Serialize)]
struct PtyExitPayload {
    pty_id: String,
    code: i32,
}

#[derive(Clone, Serialize)]
struct AgentWaitingPayload {
    pty_id: String,
    workspace_id: String,
    title: String,
    body: String,
}

struct PtyEntry {
    info: PtyInfo,
    writer: Option<Box<dyn Write + Send>>,
    master: Option<Box<dyn MasterPty + Send>>,
    child: Arc<Mutex<Option<Box<dyn Child + Send + Sync>>>>,
    child_pid: u32,
}

static PTYS: Lazy<Mutex<HashMap<String, PtyEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[tauri::command]
pub fn create_pty(
    app: AppHandle,
    workspace_id: String,
    cwd: String,
    cols: u16,
    rows: u16,
) -> Result<PtyInfo, String> {
    let id = Uuid::new_v4().to_string();

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty failed: {}", e))?;

    let mut cmd = CommandBuilder::new(DEFAULT_SHELL);
    cmd.cwd(&cwd);
    cmd.env("WMUX", "1");
    cmd.env("WMUX_PANE_ID", &id);
    cmd.env("TERM", "xterm-256color");

    let child: Box<dyn Child + Send + Sync> = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn {} failed: {}", DEFAULT_SHELL, e))?;

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("try_clone_reader failed: {}", e))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("take_writer failed: {}", e))?;

    let info = PtyInfo {
        id: id.clone(),
        workspace_id: workspace_id.clone(),
        cwd,
        cols,
        rows,
        alive: true,
        shell: DEFAULT_SHELL.to_string(),
    };

    let child_pid: u32 = child.process_id().unwrap_or(0);

    let child_arc: Arc<Mutex<Option<Box<dyn Child + Send + Sync>>>> =
        Arc::new(Mutex::new(Some(child)));
    let child_for_thread = Arc::clone(&child_arc);

    let app_for_thread = app.clone();
    let id_for_thread = id.clone();
    let ws_for_thread = workspace_id.clone();
    std::thread::spawn(move || {
        reader_thread(app_for_thread, id_for_thread, ws_for_thread, reader, child_for_thread);
    });

    let entry = PtyEntry {
        info: info.clone(),
        writer: Some(writer),
        master: Some(pair.master),
        child: child_arc,
        child_pid,
    };

    {
        let mut ptys = PTYS.lock().map_err(|e| e.to_string())?;
        ptys.insert(id.clone(), entry);
    }

    log::info!(
        "[pty] spawned {} ({} {}x{} cwd={})",
        id,
        DEFAULT_SHELL,
        cols,
        rows,
        info.cwd
    );
    Ok(info)
}

fn reader_thread(
    app: AppHandle,
    pty_id: String,
    workspace_id: String,
    mut reader: Box<dyn Read + Send>,
    child: Arc<Mutex<Option<Box<dyn Child + Send + Sync>>>>,
) {
    let mut buf = [0u8; READ_BUF_SIZE];
    let mut total_bytes: usize = 0;

    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total_bytes += n;
                let data = String::from_utf8_lossy(&buf[..n]).into_owned();

                workspace::upsert_agent_from_output(&workspace_id, &data);

                if let Some((title, body)) = parse_osc(&buf[..n]) {
                        notifications::push_notification(
                            workspace_id.clone(),
                            title.clone(),
                            body.clone(),
                            "normal".to_string(),
                        );
                        let _ = app.emit(
                            "agent_waiting",
                            AgentWaitingPayload {
                                pty_id: pty_id.clone(),
                                workspace_id: workspace_id.clone(),
                                title,
                                body,
                        },
                    );
                }

                let _ = app.emit(
                    "pty_data",
                    PtyDataPayload {
                        pty_id: pty_id.clone(),
                        data,
                    },
                );
            }
            Err(e) => {
                log::debug!("[pty] reader error for {}: {}", pty_id, e);
                break;
            }
        }
    }

    let code: i32 = {
        let mut guard = child.lock().unwrap();
        match guard.take() {
            Some(mut c) => match c.try_wait() {
                Ok(Some(status)) => status.exit_code() as i32,
                _ => c.wait().map(|s| s.exit_code() as i32).unwrap_or(-1),
            },
            None => -1,
        }
    };

    let was_tracked = {
        let Ok(mut ptys) = PTYS.lock() else { return };
        match ptys.get_mut(&pty_id) {
            Some(entry) => {
                entry.info.alive = false;
                true
            }
            None => false,
        }
    };
    if was_tracked {
        let _ = app.emit(
            "pty_exit",
            PtyExitPayload {
                pty_id: pty_id.clone(),
                code,
            },
        );
    }
    log::info!(
        "[pty] {} exited code={} ({} bytes read)",
        pty_id,
        code,
        total_bytes
    );
}

#[tauri::command]
pub fn write_pty(pty_id: String, data: String) -> Result<(), String> {
    let mut ptys = PTYS.lock().map_err(|e| e.to_string())?;
    let entry = ptys
        .get_mut(&pty_id)
        .ok_or_else(|| format!("PTY {} not found", pty_id))?;
    if !entry.info.alive {
        return Err(format!("PTY {} is not alive", pty_id));
    }
    let writer = entry
        .writer
        .as_mut()
        .ok_or_else(|| format!("PTY {} writer is closed", pty_id))?;
    writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("write failed: {}", e))?;
    let _ = writer.flush();
    log::trace!("[pty] write {}: {} bytes", pty_id, data.len());
    Ok(())
}

#[tauri::command]
pub fn resize_pty(pty_id: String, cols: u16, rows: u16) -> Result<(), String> {
    let mut ptys = PTYS.lock().map_err(|e| e.to_string())?;
    let entry = ptys
        .get_mut(&pty_id)
        .ok_or_else(|| format!("PTY {} not found", pty_id))?;
    entry.info.cols = cols;
    entry.info.rows = rows;
    let master = entry
        .master
        .as_ref()
        .ok_or_else(|| format!("PTY {} master is closed", pty_id))?;
    master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("resize failed: {}", e))?;
    log::debug!("[pty] resize {}: {}x{}", pty_id, cols, rows);
    Ok(())
}

#[tauri::command]
pub fn close_pty(pty_id: String) -> Result<(), String> {
    let entry = {
        let mut ptys = PTYS.lock().map_err(|e| e.to_string())?;
        ptys.remove(&pty_id)
            .ok_or_else(|| format!("PTY {} not found", pty_id))?
    };

    {
        let mut guard = entry.child.lock().unwrap();
        if let Some(c) = guard.as_mut() {
            let _ = c.kill();
        }
    }
    drop(entry.writer);
    drop(entry.master);

    log::info!("[pty] closed {}", pty_id);
    Ok(())
}

pub fn kill_all() {
    let entries: Vec<PtyEntry> = {
        let Ok(mut ptys) = PTYS.lock() else { return };
        ptys.drain().map(|(_, e)| e).collect()
    };
    let count = entries.len();
    for entry in entries {
        if let Ok(mut guard) = entry.child.lock() {
            if let Some(c) = guard.as_mut() {
                let _ = c.kill();
            }
        }
        drop(entry.writer);
        drop(entry.master);
    }
    log::info!("[pty] killed all ({} ptys)", count);
}

#[tauri::command]
pub fn list_ptys() -> Result<Vec<PtyInfo>, String> {
    let ptys = PTYS.lock().map_err(|e| e.to_string())?;
    Ok(ptys.values().map(|e| e.info.clone()).collect())
}

pub fn get_workspace_pids(workspace_id: &str) -> Vec<u32> {
    let Ok(ptys) = PTYS.lock() else { return Vec::new() };
    ptys.values()
        .filter(|e| e.info.workspace_id == workspace_id && e.info.alive && e.child_pid != 0)
        .map(|e| e.child_pid)
        .collect()
}

pub fn parse_osc(data: &[u8]) -> Option<(String, String)> {
    let text = String::from_utf8_lossy(data);

    if let Some(start) = text.find("\x1b]9;") {
        if let Some(end) = text[start..].find('\x07') {
            let msg = &text[start + 4..start + end];
            return Some(("Terminal".to_string(), msg.to_string()));
        }
    }

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
    fn test_pty_info_serde() {
    let info = PtyInfo {
        id: "test-id".to_string(),
        workspace_id: "ws-test".to_string(),
        cwd: "C:\\test".to_string(),
        cols: 80,
        rows: 24,
        alive: true,
        shell: "powershell.exe".to_string(),
    };
        let json = serde_json::to_string(&info).unwrap();
        let back: PtyInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "test-id");
        assert_eq!(back.cols, 80);
        assert_eq!(back.rows, 24);
        assert!(back.alive);
    }

    #[test]
    fn test_kill_all_empty() {
        kill_all();
        let ptys = PTYS.lock().unwrap();
        assert!(ptys.is_empty());
    }
}
