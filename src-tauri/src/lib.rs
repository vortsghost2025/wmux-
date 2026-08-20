mod intelligence;
mod ipc;
mod notifications;
mod pty_manager;
mod recorder;
mod session;
mod sync_groups;
mod wave_bridge;
mod workspace;

use intelligence::{get_workspace_activity, predict_next_focus};
use ipc::{ipc_status, set_app_handle, start_ipc_server};
use notifications::{get_all_notifications, mark_read, send_notification};
use pty_manager::{close_pty, create_pty, list_ptys, resize_pty, write_pty};
use recorder::{
    delete_recording, export_recording, get_recording, import_recording, list_recordings,
    replay_recording, start_recording, stop_recording,
};
use session::{load_session, save_session};
use sync_groups::{
    add_pane_to_sync, broadcast_to_sync_group, create_sync_group, delete_sync_group,
    execute_staged_command, list_sync_groups, remove_pane_from_sync, toggle_sync_group,
};
use wave_bridge::{wave_ask_all, wave_bridge_status, wave_send_command};
use workspace::{
    create_workspace, detect_listening_ports, get_workspace, list_workspaces, remove_workspace,
    rename_workspace, report_agent_status, switch_workspace, update_git_info,
};

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_title("wmux").ok();
            eprintln!("[wmux] Starting up...");
            eprintln!("[wmux] Agent-aware terminal multiplexer for Windows");
            set_app_handle(app.handle().clone());
            start_ipc_server();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            // Workspace commands
            create_workspace,
            list_workspaces,
            get_workspace,
            switch_workspace,
            rename_workspace,
            remove_workspace,
            report_agent_status,
            update_git_info,
            detect_listening_ports,
            // PTY commands
            create_pty,
            write_pty,
            resize_pty,
            close_pty,
            list_ptys,
            // Notification commands
            send_notification,
            mark_read,
            get_all_notifications,
            // Session commands
            save_session,
            load_session,
            // Wave bridge commands
            wave_bridge_status,
            wave_send_command,
            wave_ask_all,
            // IPC commands
            ipc_status,
            // Intelligence commands
            predict_next_focus,
            get_workspace_activity,
            // Sync group commands
            create_sync_group,
            add_pane_to_sync,
            remove_pane_from_sync,
            toggle_sync_group,
            delete_sync_group,
            list_sync_groups,
            broadcast_to_sync_group,
            execute_staged_command,
            // Recorder commands
            start_recording,
            stop_recording,
            list_recordings,
            get_recording,
            export_recording,
            import_recording,
            delete_recording,
            replay_recording,
        ])
        .build(tauri::generate_context!())
        .expect("error while building wmux")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                pty_manager::kill_all();
            }
        });
}

#[tauri::command]
fn ping() -> String {
    "pong".to_string()
}

#[cfg(test)]
mod lib_tests {
    use super::*;

    #[test]
    fn test_ping_returns_pong() {
        assert_eq!(ping(), "pong");
    }
}
