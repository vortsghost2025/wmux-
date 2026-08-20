// Neural Pane Intelligence — AI-powered focus prediction and activity analytics
// Tracks user behavior patterns to predict which pane needs attention next

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActivityEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String, // "input", "output", "focus", "agent_response"
    pub pane_id: String,
    pub workspace_id: String,
    pub metadata: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PaneActivityScore {
    pub pane_id: String,
    pub workspace_id: String,
    pub activity_score: f64,      // 0.0 - 1.0, higher = more active
    pub urgency_score: f64,        // 0.0 - 1.0, higher = needs attention
    pub relevance_score: f64,      // 0.0 - 1.0, predicted user interest
    pub last_activity: DateTime<Utc>,
    pub input_count: u32,
    pub output_lines: u32,
    pub agent_interactions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FocusPrediction {
    pub recommended_pane_id: String,
    pub confidence: f64,
    pub reason: String,
    pub alternative_panes: Vec<String>,
}

#[derive(Clone)]
struct PaneActivityLog {
    events: Vec<ActivityEvent>,
    total_input: u32,
    total_output_lines: u32,
    agent_interactions: u32,
    last_focus_time: Option<DateTime<Utc>>,
}

static ACTIVITY_LOGS: Lazy<Mutex<HashMap<String, PaneActivityLog>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

const DECAY_HALF_LIFE_SECS: f64 = 300.0; // 5 minutes

/// Record an activity event for a pane
pub fn record_activity(
    pane_id: String,
    workspace_id: String,
    event_type: String,
    metadata: serde_json::Value,
) {
    let mut logs = match ACTIVITY_LOGS.lock() {
        Ok(l) => l,
        Err(_) => return,
    };

    let entry = logs.entry(pane_id.clone()).or_insert_with(|| PaneActivityLog {
        events: Vec::with_capacity(100),
        total_input: 0,
        total_output_lines: 0,
        agent_interactions: 0,
        last_focus_time: None,
    });

    let event = ActivityEvent {
        timestamp: Utc::now(),
        event_type: event_type.clone(),
        pane_id: pane_id.clone(),
        workspace_id,
        metadata,
    };

    // Keep last 100 events per pane
    if entry.events.len() >= 100 {
        entry.events.remove(0);
    }
    entry.events.push(event);

    match event_type.as_str() {
        "input" => entry.total_input += 1,
        "output" => entry.total_output_lines += 1,
        "agent_response" => entry.agent_interactions += 1,
        "focus" => entry.last_focus_time = Some(Utc::now()),
        _ => {}
    }

    log::trace!(
        "[intelligence] recorded {} for pane {}",
        event_type,
        pane_id
    );
}

/// Calculate decayed activity score for a pane
fn calculate_activity_score(log: &PaneActivityLog) -> f64 {
    let now = Utc::now();
    let mut score = 0.0;

    for event in &log.events {
        let age_secs = (now - event.timestamp).num_seconds() as f64;
        let decay = (-age_secs * std::f64::consts::LN_2 / DECAY_HALF_LIFE_SECS).exp();

        let weight = match event.event_type.as_str() {
            "input" => 1.0,
            "output" => 0.3,
            "agent_response" => 1.5,
            "focus" => 0.5,
            _ => 0.1,
        };

        score += decay * weight;
    }

    // Normalize to 0-1 range (cap at 10 effective events)
    (score / 10.0).min(1.0)
}

/// Calculate urgency score based on agent state and output patterns
fn calculate_urgency_score(log: &PaneActivityLog) -> f64 {
    let mut urgency: f32 = 0.0;

    // Check recent events for waiting patterns
    let now = Utc::now();
    for event in log.events.iter().rev().take(20) {
        let age_secs = (now - event.timestamp).num_seconds() as f64;
        if age_secs > 120.0 {
            break;
        }

        if let Some(msg) = event.metadata.get("message").and_then(|v| v.as_str()) {
            let lower = msg.to_lowercase();
            if lower.contains("waiting") || lower.contains("input") || lower.contains("? (y/n)") {
                urgency += 2.0;
            }
            if lower.contains("error") || lower.contains("failed") {
                urgency += 1.5;
            }
        }
    }

    // Agent interactions without recent input = high urgency
    if log.agent_interactions > 0 && log.total_input == 0 {
        urgency += 1.0;
    }

    (urgency.min(1.0)) as f64
}

/// Predict which pane the user should focus on next
#[tauri::command]
pub fn predict_next_focus(workspace_id: Option<String>) -> Result<FocusPrediction, String> {
    let logs = ACTIVITY_LOGS.lock().map_err(|e| e.to_string())?;

    let mut candidates: Vec<(String, String, f64)> = Vec::new();

    for (pane_id, log) in logs.iter() {
        if let Some(ref ws_id) = workspace_id {
            if log.events
                .last()
                .map(|e| &e.workspace_id != ws_id)
                .unwrap_or(true)
            {
                continue;
            }
        }

        let activity = calculate_activity_score(log);
        let urgency = calculate_urgency_score(log);
        let recency = log
            .last_focus_time
            .map(|t| {
                let age = (Utc::now() - t).num_seconds() as f64;
                (1.0 / (1.0 + age / 60.0)).min(1.0)
            })
            .unwrap_or(0.5);

        // Combined relevance score
        let relevance = activity * 0.4 + urgency * 0.4 + recency * 0.2;

        let workspace = log
            .events
            .last()
            .map(|e| e.workspace_id.clone())
            .unwrap_or_default();

        candidates.push((pane_id.clone(), workspace, relevance));
    }

    drop(logs);

    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    if candidates.is_empty() {
        return Ok(FocusPrediction {
            recommended_pane_id: String::new(),
            confidence: 0.0,
            reason: "No activity data available".to_string(),
            alternative_panes: vec![],
        });
    }

    let top = &candidates[0];
    let reason = if top.2 > 0.8 {
        "High activity and urgency detected"
    } else if top.2 > 0.5 {
        "Moderate activity with recent engagement"
    } else {
        "Low activity, suggesting based on recency"
    }
    .to_string();

    let alternatives: Vec<String> = candidates.iter().skip(1).take(3).map(|c| c.0.clone()).collect();

    Ok(FocusPrediction {
        recommended_pane_id: top.0.clone(),
        confidence: top.2,
        reason,
        alternative_panes: alternatives,
    })
}

/// Get activity scores for all panes in a workspace
#[tauri::command]
pub fn get_workspace_activity(workspace_id: String) -> Result<Vec<PaneActivityScore>, String> {
    let logs = ACTIVITY_LOGS.lock().map_err(|e| e.to_string())?;
    let now = Utc::now();

    let mut scores = Vec::new();

    for (pane_id, log) in logs.iter() {
        // Filter by workspace
        if !log.events.iter().any(|e| e.workspace_id == workspace_id) {
            continue;
        }

        let last_activity = log.events.last().map(|e| e.timestamp).unwrap_or(now);

        scores.push(PaneActivityScore {
            pane_id: pane_id.clone(),
            workspace_id: workspace_id.clone(),
            activity_score: calculate_activity_score(log),
            urgency_score: calculate_urgency_score(log),
            relevance_score: 0.0, // Will be calculated externally if needed
            last_activity,
            input_count: log.total_input,
            output_lines: log.total_output_lines,
            agent_interactions: log.agent_interactions,
        });
    }

    scores.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));

    Ok(scores)
}

/// Clear old activity data to prevent memory bloat
pub fn prune_old_events(max_age_hours: u64) {
    let mut logs = match ACTIVITY_LOGS.lock() {
        Ok(l) => l,
        Err(_) => return,
    };

    let cutoff = Utc::now() - chrono::Duration::hours(max_age_hours as i64);

    for log in logs.values_mut() {
        log.events.retain(|e| e.timestamp > cutoff);
    }

    // Remove completely empty logs
    logs.retain(|_, log| !log.events.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_record_activity() {
        record_activity(
            "pane-1".to_string(),
            "ws-1".to_string(),
            "input".to_string(),
            json!({"command": "ls"}),
        );

        let logs = ACTIVITY_LOGS.lock().unwrap();
        assert!(logs.contains_key("pane-1"));
        let log = logs.get("pane-1").unwrap();
        assert_eq!(log.total_input, 1);
        assert_eq!(log.events.len(), 1);
    }

    #[test]
    fn test_activity_decay() {
        // Fresh events should have high score
        record_activity(
            "pane-fresh".to_string(),
            "ws-1".to_string(),
            "input".to_string(),
            json!({}),
        );

        let logs = ACTIVITY_LOGS.lock().unwrap();
        let log = logs.get("pane-fresh").unwrap();
        let score = calculate_activity_score(log);
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_urgency_detection() {
        record_activity(
            "pane-waiting".to_string(),
            "ws-1".to_string(),
            "output".to_string(),
            json!({"message": "Waiting for your input..."}),
        );

        let logs = ACTIVITY_LOGS.lock().unwrap();
        let log = logs.get("pane-waiting").unwrap();
        let urgency = calculate_urgency_score(log);
        assert!(urgency > 0.5);
    }
}
