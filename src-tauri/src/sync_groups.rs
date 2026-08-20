// Sync Groups — Execute commands simultaneously across multiple panes
// Inspired by tmux synchronize-panes, but with intelligent filtering

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SyncGroup {
    pub id: String,
    pub name: String,
    pub pane_ids: Vec<String>,
    pub workspace_id: String,
    pub enabled: bool,
    pub mode: SyncMode,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    /// All input is broadcast to all panes immediately
    Mirror,
    /// Input is queued and sent on explicit trigger
    Staged,
    /// Only send commands that match a pattern (e.g., cd, ls)
    Filtered,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SyncCommand {
    pub group_id: String,
    pub command: String,
    pub timestamp: String,
    pub targets: Vec<String>, // pane IDs
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SyncResult {
    pub group_id: String,
    pub command: String,
    pub success_count: u32,
    pub failed_count: u32,
    pub failures: Vec<SyncFailure>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SyncFailure {
    pub pane_id: String,
    pub error: String,
}

static SYNC_GROUPS: Lazy<Mutex<HashMap<String, SyncGroup>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Create a new sync group from selected panes
#[tauri::command]
pub fn create_sync_group(
    name: String,
    pane_ids: Vec<String>,
    workspace_id: String,
    mode: SyncMode,
) -> Result<SyncGroup, String> {
    if pane_ids.is_empty() {
        return Err("At least one pane must be in the sync group".to_string());
    }

    let id = Uuid::new_v4().to_string();
    let group = SyncGroup {
        id: id.clone(),
        name,
        pane_ids: pane_ids.clone(),
        workspace_id,
        enabled: true,
        mode,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut groups = SYNC_GROUPS.lock().map_err(|e| e.to_string())?;
    groups.insert(id.clone(), group.clone());

    log::info!(
        "[sync] Created group '{}' with {} panes (mode: {:?})",
        group.name,
        group.pane_ids.len(),
        group.mode
    );

    Ok(group)
}

/// Add a pane to an existing sync group
#[tauri::command]
pub fn add_pane_to_sync(group_id: String, pane_id: String) -> Result<(), String> {
    let mut groups = SYNC_GROUPS.lock().map_err(|e| e.to_string())?;
    let group = groups
        .get_mut(&group_id)
        .ok_or_else(|| format!("Sync group {} not found", group_id))?;

    if !group.pane_ids.contains(&pane_id) {
        group.pane_ids.push(pane_id.clone());
        log::info!("[sync] Added pane {} to group {}", pane_id, group.name);
    }

    Ok(())
}

/// Remove a pane from a sync group
#[tauri::command]
pub fn remove_pane_from_sync(group_id: String, pane_id: String) -> Result<(), String> {
    let mut groups = SYNC_GROUPS.lock().map_err(|e| e.to_string())?;
    
    {
        let group = groups
            .get_mut(&group_id)
            .ok_or_else(|| format!("Sync group {} not found", group_id))?;

        group.pane_ids.retain(|id| id != &pane_id);
        
        if group.pane_ids.is_empty() {
            drop(group);
            groups.remove(&group_id);
            log::info!("[sync] Removed empty group {}", group_id);
        } else {
            log::info!("[sync] Removed pane {} from group {}", pane_id, group.name);
        }
    };

    Ok(())
}

/// Enable or disable a sync group
#[tauri::command]
pub fn toggle_sync_group(group_id: String, enabled: bool) -> Result<(), String> {
    let mut groups = SYNC_GROUPS.lock().map_err(|e| e.to_string())?;
    let group = groups
        .get_mut(&group_id)
        .ok_or_else(|| format!("Sync group {} not found", group_id))?;

    group.enabled = enabled;
    log::info!("[sync] Group {} {}abled", group.name, if enabled { "en" } else { "dis" });

    Ok(())
}

/// Delete a sync group entirely
#[tauri::command]
pub fn delete_sync_group(group_id: String) -> Result<(), String> {
    let mut groups = SYNC_GROUPS.lock().map_err(|e| e.to_string())?;
    if let Some(group) = groups.remove(&group_id) {
        log::info!("[sync] Deleted group {}", group.name);
    }
    Ok(())
}

/// List all sync groups for a workspace
#[tauri::command]
pub fn list_sync_groups(workspace_id: Option<String>) -> Result<Vec<SyncGroup>, String> {
    let groups = SYNC_GROUPS.lock().map_err(|e| e.to_string())?;
    let mut result: Vec<SyncGroup> = groups
        .values()
        .filter(|g| workspace_id.as_ref().map_or(true, |ws| &g.workspace_id == ws))
        .cloned()
        .collect();

    result.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(result)
}

/// Broadcast a command to all panes in a sync group
#[tauri::command]
pub fn broadcast_to_sync_group(
    group_id: String,
    command: String,
) -> Result<SyncResult, String> {
    let groups = SYNC_GROUPS.lock().map_err(|e| e.to_string())?;
    let group = groups
        .get(&group_id)
        .ok_or_else(|| format!("Sync group {} not found", group_id))?;

    if !group.enabled {
        return Err(format!("Sync group '{}' is disabled", group.name));
    }

    let mut success_count = 0u32;
    let mut failed_count = 0u32;
    let mut failures = Vec::new();

    for pane_id in &group.pane_ids {
        match crate::pty_manager::write_pty(pane_id.clone(), command.clone()) {
            Ok(_) => success_count += 1,
            Err(e) => {
                failed_count += 1;
                failures.push(SyncFailure {
                    pane_id: pane_id.clone(),
                    error: e,
                });
            }
        }
    }

    log::info!(
        "[sync] Broadcast to '{}': {} succeeded, {} failed",
        group.name,
        success_count,
        failed_count
    );

    Ok(SyncResult {
        group_id,
        command,
        success_count,
        failed_count,
        failures,
    })
}

/// Send a staged command (for Staged mode groups)
#[tauri::command]
pub fn execute_staged_command(
    group_id: String,
    command: String,
    target_panes: Option<Vec<String>>,
) -> Result<SyncResult, String> {
    let groups = SYNC_GROUPS.lock().map_err(|e| e.to_string())?;
    let group = groups
        .get(&group_id)
        .ok_or_else(|| format!("Sync group {} not found", group_id))?;

    if group.mode != SyncMode::Staged {
        return Err("Group is not in staged mode".to_string());
    }

    let targets = target_panes.unwrap_or_else(|| group.pane_ids.clone());

    let mut success_count = 0u32;
    let mut failed_count = 0u32;
    let mut failures = Vec::new();

    for pane_id in &targets {
        match crate::pty_manager::write_pty(pane_id.clone(), command.clone()) {
            Ok(_) => success_count += 1,
            Err(e) => {
                failed_count += 1;
                failures.push(SyncFailure {
                    pane_id: pane_id.clone(),
                    error: e,
                });
            }
        }
    }

    Ok(SyncResult {
        group_id,
        command,
        success_count,
        failed_count,
        failures,
    })
}

/// Check if a pane belongs to any active sync group
pub fn is_pane_in_sync_group(pane_id: &str) -> bool {
    let groups = match SYNC_GROUPS.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };

    groups.values().any(|g| g.enabled && g.pane_ids.contains(&pane_id.to_string()))
}

/// Get the sync group(s) a pane belongs to
pub fn get_pane_sync_groups(pane_id: &str) -> Vec<SyncGroup> {
    let groups = match SYNC_GROUPS.lock() {
        Ok(g) => g,
        Err(_) => return vec![],
    };

    groups
        .values()
        .filter(|g| g.pane_ids.contains(&pane_id.to_string()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_sync_group() {
        let group = create_sync_group(
            "test-group".to_string(),
            vec!["pane-1".to_string(), "pane-2".to_string()],
            "ws-1".to_string(),
            SyncMode::Mirror,
        )
        .unwrap();

        assert_eq!(group.name, "test-group");
        assert_eq!(group.pane_ids.len(), 2);
        assert!(group.enabled);
        assert_eq!(group.mode, SyncMode::Mirror);
    }

    #[test]
    fn test_add_remove_pane() {
        let group = create_sync_group(
            "add-remove-test".to_string(),
            vec!["pane-1".to_string()],
            "ws-1".to_string(),
            SyncMode::Mirror,
        )
        .unwrap();

        add_pane_to_sync(group.id.clone(), "pane-3".to_string()).unwrap();
        {
            let groups = SYNC_GROUPS.lock().unwrap();
            let g = groups.get(&group.id).unwrap();
            assert_eq!(g.pane_ids.len(), 2);
        }

        remove_pane_from_sync(group.id.clone(), "pane-1".to_string()).unwrap();
        {
            let groups = SYNC_GROUPS.lock().unwrap();
            let g = groups.get(&group.id).unwrap();
            assert_eq!(g.pane_ids.len(), 1);
            assert!(!g.pane_ids.contains(&"pane-1".to_string()));
        }
    }

    #[test]
    fn test_toggle_sync_group() {
        let group = create_sync_group(
            "toggle-test".to_string(),
            vec!["pane-1".to_string()],
            "ws-1".to_string(),
            SyncMode::Mirror,
        )
        .unwrap();

        toggle_sync_group(group.id.clone(), false).unwrap();
        {
            let groups = SYNC_GROUPS.lock().unwrap();
            let g = groups.get(&group.id).unwrap();
            assert!(!g.enabled);
        }

        toggle_sync_group(group.id.clone(), true).unwrap();
        {
            let groups = SYNC_GROUPS.lock().unwrap();
            let g = groups.get(&group.id).unwrap();
            assert!(g.enabled);
        }
    }

    #[test]
    fn test_list_sync_groups() {
        create_sync_group(
            "list-test-1".to_string(),
            vec!["pane-1".to_string()],
            "ws-list".to_string(),
            SyncMode::Mirror,
        )
        .unwrap();

        let groups = list_sync_groups(Some("ws-list".to_string())).unwrap();
        assert!(!groups.is_empty());
        assert!(groups.iter().any(|g| g.name.starts_with("list-test")));
    }
}
