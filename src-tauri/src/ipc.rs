// IPC Server — named pipe for CLI↔app communication

use once_cell::sync::Lazy;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HANDLE};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::{
    PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Pipes::{
    CreateNamedPipeW, ConnectNamedPipe, DisconnectNamedPipe,
    PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT,
};
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;

#[cfg(not(target_os = "windows"))]
use std::os::unix::net::UnixListener;

static IPC_APP_HANDLE: Lazy<Mutex<Option<AppHandle>>> = Lazy::new(|| Mutex::new(None));

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct IpcRequest {
    pub command: String,
    pub payload: serde_json::Value,
}

#[derive(serde::Serialize, Debug)]
pub struct IpcResponse {
    pub success: bool,
    pub message: String,
}

fn pipe_name() -> String {
    if cfg!(target_os = "windows") {
        r"\\.\pipe\wmux".to_string()
    } else {
        "/tmp/wmux.sock".to_string()
    }
}

pub fn set_app_handle(app: AppHandle) {
    let mut guard = IPC_APP_HANDLE.lock().unwrap();
    *guard = Some(app);
}

pub fn start_ipc_server() {
    let pipe = pipe_name();
    log::info!("[ipc] Starting server on {}", pipe);
    eprintln!("[ipc] Starting server on {}", pipe);

    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(move || {
            eprintln!("[ipc] Windows server thread started");
            ipc_server_windows(&pipe);
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::thread::spawn(move || {
            eprintln!("[ipc] Unix server thread started");
            ipc_server_unix(&pipe);
        });
    }
}

#[cfg(target_os = "windows")]
fn ipc_server_windows(pipe_name: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::ERROR_PIPE_CONNECTED;

    let wide_name: Vec<u16> = OsStr::new(pipe_name).encode_wide().chain(Some(0)).collect();

    loop {
        eprintln!("[ipc] Creating named pipe...");
        let handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(wide_name.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                10,
                4096,
                4096,
                0,
                None,
            )
        };

        if handle.is_invalid() {
            let err = std::io::Error::last_os_error();
            log::error!("[ipc] Failed to create named pipe: {}", err);
            eprintln!("[ipc] Failed to create named pipe: {}", err);
            std::thread::sleep(std::time::Duration::from_secs(1));
            continue;
        }

        eprintln!("[ipc] Pipe created, waiting for connection...");
        let connected = unsafe { ConnectNamedPipe(handle, None) };
        if connected.is_err() {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(ERROR_PIPE_CONNECTED.0 as i32) {
                log::debug!("[ipc] ConnectNamedPipe: {}", err);
                eprintln!("[ipc] ConnectNamedPipe error: {}", err);
                unsafe { windows::Win32::Foundation::CloseHandle(handle).ok(); }
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
        }

        log::info!("[ipc] Client connected");
        eprintln!("[ipc] Client connected");
        handle_client_windows(handle);
        unsafe { DisconnectNamedPipe(handle).ok(); }
        unsafe { windows::Win32::Foundation::CloseHandle(handle).ok(); }
    }
}

#[cfg(target_os = "windows")]
fn handle_client_windows(handle: HANDLE) {
    let mut buffer = [0u8; 4096];
    loop {
        let mut bytes_read = 0u32;
        let result = unsafe {
            ReadFile(
                handle,
                Some(&mut buffer),
                Some(&mut bytes_read),
                None,
            )
        };

        if result.is_err() || bytes_read == 0 {
            break;
        }

        let request_data = &buffer[..bytes_read as usize];
        if let Ok(request_str) = std::str::from_utf8(request_data) {
            if let Ok(req) = serde_json::from_str::<IpcRequest>(request_str) {
                let response = handle_ipc_request(req);
                let response_json = serde_json::to_string(&response).unwrap_or_default();
                let response_bytes = response_json.as_bytes();

                let mut bytes_written = 0u32;
                unsafe {
                    WriteFile(
                        handle,
                        Some(response_bytes),
                        Some(&mut bytes_written),
                        None,
                    )
                    .ok();
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn ipc_server_unix(socket_path: &str) {
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path).expect("Failed to bind Unix socket");
    log::info!("[ipc] Unix socket listening on {}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                use std::io::{BufRead, BufReader, Write};
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() {
                    if let Ok(req) = serde_json::from_str::<IpcRequest>(&line) {
                        let response = handle_ipc_request(req);
                        let response_json = serde_json::to_string(&response).unwrap_or_default();
                        let _ = stream.write_all(response_json.as_bytes());
                        let _ = stream.write_all(b"\n");
                    }
                }
            }
            Err(e) => log::error!("[ipc] Connection error: {}", e),
        }
    }
}

fn handle_ipc_request(req: IpcRequest) -> IpcResponse {
    match req.command.as_str() {
        "notify" => {
            let workspace_id = req.payload.get("workspace_id").and_then(|v| v.as_str()).unwrap_or("default");
            let title = req.payload.get("title").and_then(|v| v.as_str()).unwrap_or("wmux");
            let body = req.payload.get("body").and_then(|v| v.as_str()).unwrap_or("");
            let urgency = req.payload.get("urgency").and_then(|v| v.as_str()).unwrap_or("normal");

            if let Some(app) = IPC_APP_HANDLE.lock().unwrap().as_ref() {
                let _ = crate::notifications::push_notification(
                    workspace_id.to_string(),
                    title.to_string(),
                    body.to_string(),
                    urgency.to_string(),
                );
                let _ = app.emit("agent_waiting", serde_json::json!({
                    "pty_id": "cli",
                    "workspace_id": workspace_id,
                    "title": title,
                    "body": body
                }));
            }

            IpcResponse {
                success: true,
                message: "Notification sent".to_string(),
            }
        }
        "ping" => IpcResponse {
            success: true,
            message: "pong".to_string(),
        },
        _ => IpcResponse {
            success: false,
            message: format!("Unknown command: {}", req.command),
        },
    }
}

#[tauri::command]
pub fn ipc_status() -> Result<String, String> {
    Ok(format!("IPC server running on {}", pipe_name()))
}