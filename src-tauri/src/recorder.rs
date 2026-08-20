// Command Recorder — Record and replay terminal sessions
// Like tmux capture-pane + script, but with intelligent filtering and metadata

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecordingSession {
    pub id: String,
    pub name: String,
    pub workspace_id: String,
    pub pane_ids: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub events: Vec<RecordedEvent>,
    pub total_events: u32,
    pub duration_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecordedEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String, // "input", "output", "resize", "command"
    pub pane_id: String,
    pub data: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReplayConfig {
    pub speed_multiplier: f64,   // 0.5x to 10x
    pub filter_patterns: Vec<String>, // Regex patterns to skip
    pub include_input: bool,
    pub include_output: bool,
    pub max_events: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReplayResult {
    pub session_id: String,
    pub events_replayed: u32,
    pub events_skipped: u32,
    pub duration_secs: f64,
    pub errors: Vec<String>,
}

static RECORDINGS: Lazy<Mutex<HashMap<String, RecordingSession>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Start recording a pane's activity
#[tauri::command]
pub fn start_recording(
    name: String,
    workspace_id: String,
    pane_ids: Vec<String>,
) -> Result<RecordingSession, String> {
    let id = Uuid::new_v4().to_string();

    let session = RecordingSession {
        id: id.clone(),
        name,
        workspace_id,
        pane_ids: pane_ids.clone(),
        started_at: Utc::now(),
        ended_at: None,
        events: Vec::with_capacity(1000),
        total_events: 0,
        duration_secs: None,
    };

    let mut recordings = RECORDINGS.lock().map_err(|e| e.to_string())?;
    recordings.insert(id.clone(), session.clone());

    log::info!(
        "[recorder] Started recording '{}' for {} panes",
        session.name,
        session.pane_ids.len()
    );

    Ok(session)
}

/// Stop recording and finalize the session
#[tauri::command]
pub fn stop_recording(session_id: String) -> Result<RecordingSession, String> {
    let mut recordings = RECORDINGS.lock().map_err(|e| e.to_string())?;

    let session = recordings
        .get_mut(&session_id)
        .ok_or_else(|| format!("Recording session {} not found", session_id))?;

    if session.ended_at.is_some() {
        return Err("Recording already stopped".to_string());
    }

    session.ended_at = Some(Utc::now());
    session.total_events = session.events.len() as u32;
    session.duration_secs = session
        .ended_at
        .map(|end| (end - session.started_at).num_seconds() as u64);

    let session_clone = session.clone();

    log::info!(
        "[recorder] Stopped '{}' after {} events ({}s)",
        session.name,
        session.total_events,
        session.duration_secs.unwrap_or(0)
    );

    Ok(session_clone)
}

/// Add an event to a recording session
pub fn record_event(session_id: &str, event: RecordedEvent) {
    let mut recordings = match RECORDINGS.lock() {
        Ok(r) => r,
        Err(_) => return,
    };

    if let Some(session) = recordings.get_mut(session_id) {
        if session.ended_at.is_none() {
            session.events.push(event);
        }
    }
}

/// List all recording sessions
#[tauri::command]
pub fn list_recordings(workspace_id: Option<String>) -> Result<Vec<RecordingSession>, String> {
    let recordings = RECORDINGS.lock().map_err(|e| e.to_string())?;

    let mut result: Vec<RecordingSession> = recordings
        .values()
        .filter(|s| workspace_id.as_ref().map_or(true, |ws| &s.workspace_id == ws))
        .cloned()
        .collect();

    result.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(result)
}

/// Get details of a specific recording
#[tauri::command]
pub fn get_recording(session_id: String) -> Result<RecordingSession, String> {
    let recordings = RECORDINGS.lock().map_err(|e| e.to_string())?;

    recordings
        .get(&session_id)
        .cloned()
        .ok_or_else(|| format!("Recording session {} not found", session_id))
}

/// Export recording to JSON file
#[tauri::command]
pub fn export_recording(session_id: String, path: String) -> Result<String, String> {
    let recordings = RECORDINGS.lock().map_err(|e| e.to_string())?;

    let session = recordings
        .get(&session_id)
        .ok_or_else(|| format!("Recording session {} not found", session_id))?;

    let json = serde_json::to_string_pretty(&session)
        .map_err(|e| format!("Serialization failed: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Write failed: {}", e))?;

    log::info!("[recorder] Exported '{}' to {}", session.name, path);
    Ok(path)
}

/// Import recording from JSON file
#[tauri::command]
pub fn import_recording(path: String) -> Result<RecordingSession, String> {
    let json = fs::read_to_string(&path).map_err(|e| format!("Read failed: {}", e))?;

    let mut session: RecordingSession = serde_json::from_str(&json)
        .map_err(|e| format!("Parse failed: {}", e))?;

    // Generate new ID to avoid conflicts
    session.id = Uuid::new_v4().to_string();

    let mut recordings = RECORDINGS.lock().map_err(|e| e.to_string())?;
    recordings.insert(session.id.clone(), session.clone());

    log::info!("[recorder] Imported '{}' from {}", session.name, path);
    Ok(session)
}

/// Delete a recording session
#[tauri::command]
pub fn delete_recording(session_id: String) -> Result<(), String> {
    let mut recordings = RECORDINGS.lock().map_err(|e| e.to_string())?;

    if let Some(session) = recordings.remove(&session_id) {
        log::info!("[recorder] Deleted '{}'", session.name);
    }

    Ok(())
}

/// Replay a recording with optional speed control
#[tauri::command]
pub fn replay_recording(
    session_id: String,
    config: ReplayConfig,
) -> Result<ReplayResult, String> {
    let recordings = RECORDINGS.lock().map_err(|e| e.to_string())?;

    let session = recordings
        .get(&session_id)
        .ok_or_else(|| format!("Recording session {} not found", session_id))?;

    let mut events_replayed = 0u32;
    let mut events_skipped = 0u32;
    let mut errors = Vec::new();

    let base_delay = std::time::Duration::from_secs_f64(1.0 / config.speed_multiplier);

    for event in &session.events {
        // Apply filters
        if let Some(max) = config.max_events {
            if events_replayed as usize >= max {
                break;
            }
        }

        // Check pattern filters
        let mut skip = false;
        for pattern in &config.filter_patterns {
            if event.data.contains(pattern) {
                skip = true;
                break;
            }
        }

        if skip {
            events_skipped += 1;
            continue;
        }

        // Filter by event type
        match event.event_type.as_str() {
            "input" if !config.include_input => {
                events_skipped += 1;
                continue;
            }
            "output" if !config.include_output => {
                events_skipped += 1;
                continue;
            }
            _ => {}
        }

        // Send to target pane
        if let Err(e) = crate::pty_manager::write_pty(event.pane_id.clone(), event.data.clone()) {
            errors.push(format!("Pane {} error: {}", event.pane_id, e));
        } else {
            events_replayed += 1;
        }

        // Simulate timing delay
        std::thread::sleep(base_delay);
    }

    Ok(ReplayResult {
        session_id,
        events_replayed,
        events_skipped,
        duration_secs: session.duration_secs.map(|s| s as f64).unwrap_or(0.0),
        errors,
    })
}

/// Record input to a specific session
pub fn record_input(session_id: &str, pane_id: &str, data: &str) {
    let event = RecordedEvent {
        timestamp: Utc::now(),
        event_type: "input".to_string(),
        pane_id: pane_id.to_string(),
        data: data.to_string(),
        metadata: None,
    };
    record_event(session_id, event);
}

/// Record output from a specific session
pub fn record_output(session_id: &str, pane_id: &str, data: &str) {
    let event = RecordedEvent {
        timestamp: Utc::now(),
        event_type: "output".to_string(),
        pane_id: pane_id.to_string(),
        data: data.to_string(),
        metadata: None,
    };
    record_event(session_id, event);
}

/// Save recording to disk automatically when stopped
pub fn auto_save_recording(session: &RecordingSession, save_dir: &PathBuf) -> Option<PathBuf> {
    if session.ended_at.is_none() {
        return None;
    }

    let filename = format!(
        "{}_{}.wmuxrec",
        session.started_at.format("%Y%m%d_%H%M%S"),
        session.name.replace(' ', "_")
    );

    let path = save_dir.join(&filename);

    match File::create(&path) {
        Ok(mut file) => {
            if let Ok(json) = serde_json::to_string(session) {
                let _ = file.write_all(json.as_bytes());
                log::info!("[recorder] Auto-saved to {:?}", path);
                Some(path)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_stop_recording() {
        let session = start_recording(
            "test-session".to_string(),
            "ws-1".to_string(),
            vec!["pane-1".to_string()],
        )
        .unwrap();

        assert_eq!(session.name, "test-session");
        assert!(session.ended_at.is_none());

        let stopped = stop_recording(session.id).unwrap();
        assert!(stopped.ended_at.is_some());
        assert_eq!(stopped.total_events, 0);
    }

    #[test]
    fn test_record_events() {
        let session = start_recording(
            "events-test".to_string(),
            "ws-1".to_string(),
            vec!["pane-1".to_string()],
        )
        .unwrap();

        record_event(
            &session.id,
            RecordedEvent {
                timestamp: Utc::now(),
                event_type: "input".to_string(),
                pane_id: "pane-1".to_string(),
                data: "ls -la".to_string(),
                metadata: None,
            },
        );

        record_event(
            &session.id,
            RecordedEvent {
                timestamp: Utc::now(),
                event_type: "output".to_string(),
                pane_id: "pane-1".to_string(),
                data: "total 0".to_string(),
                metadata: None,
            },
        );

        let retrieved = get_recording(session.id).unwrap();
        assert_eq!(retrieved.events.len(), 2);
    }

    #[test]
    fn test_list_recordings() {
        start_recording(
            "list-test".to_string(),
            "ws-list".to_string(),
            vec!["pane-1".to_string()],
        )
        .unwrap();

        let recordings = list_recordings(Some("ws-list".to_string())).unwrap();
        assert!(!recordings.is_empty());
    }
}
