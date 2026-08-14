// Workspace Manager — tracks workspaces, panes, git info, and agent status
// Each workspace maps to a project directory with its own set of terminal panes

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Mutex;
use uuid::Uuid;

fn detect_git_branch(directory: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(directory)
        .output()
        .ok()?;
    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() && branch != "HEAD" {
            return Some(branch);
        }
    }
    None
}

fn detect_agent_from_output(output: &str) -> Option<AgentInfo> {
    let lower = output.to_lowercase();
    if lower.contains("claude") || lower.contains("anthropic") {
        return Some(AgentInfo {
            name: "Claude".to_string(),
            status: "running".to_string(),
            pane_id: None,
            elapsed_secs: None,
        });
    }
    if lower.contains("kilo") {
        return Some(AgentInfo {
            name: "Kilo".to_string(),
            status: "running".to_string(),
            pane_id: None,
            elapsed_secs: None,
        });
    }
    if lower.contains("glm") || lower.contains("z.ai") {
        return Some(AgentInfo {
            name: "GLM".to_string(),
            status: "running".to_string(),
            pane_id: None,
            elapsed_secs: None,
        });
    }
    if lower.contains("copilot") || lower.contains("github copilot") {
        return Some(AgentInfo {
            name: "Copilot".to_string(),
            status: "running".to_string(),
            pane_id: None,
            elapsed_secs: None,
        });
    }
    if lower.contains("cursor") && lower.contains("agent") {
        return Some(AgentInfo {
            name: "Cursor".to_string(),
            status: "running".to_string(),
            pane_id: None,
            elapsed_secs: None,
        });
    }
    None
}

fn detect_listening_ports_internal(workspace_id: &str) -> Vec<u16> {
    let pids = crate::pty_manager::get_workspace_pids(workspace_id);
    if pids.is_empty() {
        return vec![];
    }

    let output = match Command::new("netstat").args(["-ano"]).output() {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    if !output.status.success() {
        return vec![];
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut ports = Vec::new();

    for line in text.lines() {
        if !line.contains("LISTENING") {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        let local_addr = parts[1];
        let pid_str = parts[4];
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !pids.contains(&pid) {
            continue;
        }
        if let Some(port_str) = local_addr.rsplit(':').next() {
            if let Ok(port) = port_str.parse::<u16>() {
                if port > 0 {
                    ports.push(port);
                }
            }
        }
    }

    ports.sort();
    ports.dedup();
    ports
}

// ===== Types =====

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub git_branch: Option<String>,
    pub git_pr: Option<PrInfo>,
    pub agents: Vec<AgentInfo>,
    pub listening_ports: Vec<u16>,
    pub is_active: bool,
    pub unread_count: u32,
    pub last_notification: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PrInfo {
    pub number: u32,
    pub status: String,
    pub checks: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentInfo {
    pub name: String,
    pub status: String, // "running", "waiting", "idle", "error"
    pub pane_id: Option<String>,
    pub elapsed_secs: Option<u64>,
}

// ===== Global State =====

static WORKSPACES: Lazy<Mutex<HashMap<String, Workspace>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

static ACTIVE_ID: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

// ===== Commands =====

#[tauri::command]
pub fn create_workspace(name: String, directory: String) -> Result<Workspace, String> {
    let id = Uuid::new_v4().to_string();
    let mut workspaces = WORKSPACES.lock().map_err(|e| e.to_string())?;
    let is_first = workspaces.is_empty();

    let git_branch = detect_git_branch(&directory);
    let ws = Workspace {
        id: id.clone(),
        name,
        directory,
        git_branch,
        git_pr: None,
        agents: vec![],
        listening_ports: vec![],
        is_active: is_first,
        unread_count: 0,
        last_notification: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    if is_first {
        let mut active = ACTIVE_ID.lock().map_err(|e| e.to_string())?;
        *active = Some(id.clone());
    }

    workspaces.insert(id.clone(), ws.clone());
    log::info!("[workspace] Created: {} ({})", ws.name, ws.id);
    Ok(ws)
}

/// Internal: list workspaces without Tauri command wrapper
pub fn get_active_workspace_id() -> Option<String> {
    ACTIVE_ID.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn list_workspaces_internal() -> Vec<Workspace> {
    let workspaces = WORKSPACES.lock().unwrap_or_else(|e| e.into_inner());
    let mut list: Vec<Workspace> = workspaces.values().cloned().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

#[tauri::command]
pub fn list_workspaces() -> Result<Vec<Workspace>, String> {
    Ok(list_workspaces_internal())
}

#[tauri::command]
pub fn get_workspace(workspace_id: String) -> Result<Workspace, String> {
    let workspaces = WORKSPACES.lock().map_err(|e| e.to_string())?;
    workspaces
        .get(&workspace_id)
        .cloned()
        .ok_or_else(|| format!("Workspace {} not found", workspace_id))
}

#[tauri::command]
pub fn switch_workspace(workspace_id: String) -> Result<(), String> {
    let mut workspaces = WORKSPACES.lock().map_err(|e| e.to_string())?;
    let mut active = ACTIVE_ID.lock().map_err(|e| e.to_string())?;

    if !workspaces.contains_key(&workspace_id) {
        return Err(format!("Workspace {} not found", workspace_id));
    }

    // Deactivate current
    if let Some(current_id) = active.as_ref() {
        if let Some(ws) = workspaces.get_mut(current_id) {
            ws.is_active = false;
        }
    }

    // Activate new
    if let Some(ws) = workspaces.get_mut(&workspace_id) {
        ws.is_active = true;
        ws.unread_count = 0;
        ws.git_branch = detect_git_branch(&ws.directory);
        ws.listening_ports = detect_listening_ports_internal(&workspace_id);
    }
    *active = Some(workspace_id);

    Ok(())
}

#[tauri::command]
pub fn rename_workspace(workspace_id: String, new_name: String) -> Result<(), String> {
    let mut workspaces = WORKSPACES.lock().map_err(|e| e.to_string())?;
    let ws = workspaces
        .get_mut(&workspace_id)
        .ok_or_else(|| format!("Workspace {} not found", workspace_id))?;
    ws.name = new_name;
    Ok(())
}

#[tauri::command]
pub fn remove_workspace(workspace_id: String) -> Result<(), String> {
    let mut workspaces = WORKSPACES.lock().map_err(|e| e.to_string())?;
    workspaces
        .remove(&workspace_id)
        .ok_or_else(|| format!("Workspace {} not found", workspace_id))?;
    Ok(())
}

#[tauri::command]
pub fn report_agent_status(
    workspace_id: String,
    agent_name: String,
    status: String,
    pane_id: Option<String>,
) -> Result<(), String> {
    let mut workspaces = WORKSPACES.lock().map_err(|e| e.to_string())?;
    let ws = workspaces
        .get_mut(&workspace_id)
        .ok_or_else(|| format!("Workspace {} not found", workspace_id))?;

    let agent = AgentInfo {
        name: agent_name.clone(),
        status: status.clone(),
        pane_id,
        elapsed_secs: None,
    };

    // Update or add agent
    if let Some(existing) = ws.agents.iter_mut().find(|a| a.name == agent_name) {
        existing.status = status.clone();
    } else {
        ws.agents.push(agent);
    }

    // If waiting + not active workspace → increment unread
    if status == "waiting" && !ws.is_active {
        ws.unread_count += 1;
        ws.last_notification = Some(format!("{} is waiting for input", agent_name));
    }

    log::info!(
        "[workspace] Agent {} status → {} in {}",
        agent_name,
        status,
        ws.name
    );
    Ok(())
}

pub fn upsert_agent_from_output(workspace_id: &str, output: &str) {
    if let Some(agent) = detect_agent_from_output(output) {
        let Ok(mut workspaces) = WORKSPACES.lock() else { return };
        if let Some(ws) = workspaces.get_mut(workspace_id) {
            if !ws.agents.iter().any(|a| a.name == agent.name) {
                ws.agents.push(agent);
                log::info!("[workspace] Auto-detected agent in {}: {}", ws.name, ws.agents.last().unwrap().name);
            }
        }
    }
}

#[tauri::command]
pub fn update_git_info(
    workspace_id: String,
    branch: Option<String>,
    pr_number: Option<u32>,
    pr_status: Option<String>,
    pr_checks: Option<String>,
) -> Result<(), String> {
    let mut workspaces = WORKSPACES.lock().map_err(|e| e.to_string())?;
    let ws = workspaces
        .get_mut(&workspace_id)
        .ok_or_else(|| format!("Workspace {} not found", workspace_id))?;

    ws.git_branch = branch;

    if let Some(number) = pr_number {
        ws.git_pr = Some(PrInfo {
            number,
            status: pr_status.unwrap_or_else(|| "open".to_string()),
            checks: pr_checks.unwrap_or_else(|| "unknown".to_string()),
        });
    }

    Ok(())
}

#[tauri::command]
pub fn detect_listening_ports(workspace_id: String) -> Result<Vec<u16>, String> {
    let ports = detect_listening_ports_internal(&workspace_id);
    let mut workspaces = WORKSPACES.lock().map_err(|e| e.to_string())?;
    if let Some(ws) = workspaces.get_mut(&workspace_id) {
        ws.listening_ports = ports.clone();
    }
    Ok(ports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_workspace() {
        let ws = create_workspace("test".to_string(), "/tmp/test".to_string()).unwrap();
        assert_eq!(ws.name, "test");
        assert_eq!(ws.directory, "/tmp/test");
        assert!(!ws.id.is_empty());
    }

    #[test]
    fn test_report_agent_waiting_increments_unread() {
        let ws = create_workspace("agent-test".to_string(), "/tmp".to_string()).unwrap();
        // Deactivate it first so unread increments
        switch_workspace("nonexistent".to_string()).ok();
        {
            let mut workspaces = WORKSPACES.lock().unwrap();
            if let Some(w) = workspaces.get_mut(&ws.id) {
                w.is_active = false;
            }
        }
        report_agent_status(ws.id.clone(), "GLM-5.1".to_string(), "waiting".to_string(), None)
            .unwrap();
        let updated = get_workspace(ws.id).unwrap();
        assert!(updated.unread_count > 0);
    }
}
