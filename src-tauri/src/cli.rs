// wmux CLI — named pipe client for IPC with the wmux app

use serde_json::json;

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, HANDLE};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, WriteFile, ReadFile};
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;

#[cfg(not(target_os = "windows"))]
use std::os::unix::net::UnixStream;
use std::io::{BufRead, BufReader, Write};

fn pipe_name() -> String {
    if cfg!(target_os = "windows") {
        r"\\.\pipe\wmux".to_string()
    } else {
        "/tmp/wmux.sock".to_string()
    }
}

fn send_request(request: serde_json::Value) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        send_request_windows(request)
    }
    #[cfg(not(target_os = "windows"))]
    {
        send_request_unix(request)
    }
}

#[cfg(target_os = "windows")]
fn send_request_windows(request: serde_json::Value) -> Result<serde_json::Value, String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let wide_name: Vec<u16> = OsStr::new(&pipe_name()).encode_wide().chain(Some(0)).collect();

    let handle: HANDLE = unsafe {
        CreateFileW(
            PCWSTR(wide_name.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }.map_err(|e| format!("Failed to connect to wmux IPC pipe. Is wmux running? Error: {}", e))?;

    if handle.0.is_null() {
        return Err("Failed to connect to wmux IPC pipe: invalid handle".to_string());
    }

    let request_str = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    let request_bytes = request_str.as_bytes();

    let mut bytes_written = 0u32;
    unsafe {
        WriteFile(
            handle,
            Some(request_bytes),
            Some(&mut bytes_written),
            None,
        )
        .map_err(|e| e.to_string())?;
    }

    let mut buffer = [0u8; 4096];
    let mut bytes_read = 0u32;
    unsafe {
        ReadFile(
            handle,
            Some(&mut buffer),
            Some(&mut bytes_read),
            None,
        )
        .map_err(|e| e.to_string())?;
    }

    let response_data = &buffer[..bytes_read as usize];
    let response_str = std::str::from_utf8(response_data).map_err(|e| e.to_string())?;
    let response: serde_json::Value = serde_json::from_str(response_str).map_err(|e| e.to_string())?;

    unsafe { windows::Win32::Foundation::CloseHandle(handle).ok(); }

    Ok(response)
}

#[cfg(not(target_os = "windows"))]
fn send_request_unix(request: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut stream = UnixStream::connect(&pipe_name()).map_err(|e| format!("Failed to connect to wmux IPC socket: {}", e))?;
    let request_str = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    stream.write_all(request_str.as_bytes()).map_err(|e| e.to_string())?;
    stream.write_all(b"\n").map_err(|e| e.to_string())?;

    let mut reader = std::io::BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let response: serde_json::Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;

    Ok(response)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "notify" => {
            let mut workspace_id = "default";
            let mut title = "wmux";
            let mut body = "";
            let mut urgency = "normal";

            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "-w" | "--workspace" => {
                        if i + 1 < args.len() {
                            workspace_id = &args[i + 1];
                            i += 2;
                        } else {
                            eprintln!("Error: --workspace requires a value");
                            return;
                        }
                    }
                    "-t" | "--title" => {
                        if i + 1 < args.len() {
                            title = &args[i + 1];
                            i += 2;
                        } else {
                            eprintln!("Error: --title requires a value");
                            return;
                        }
                    }
                    "-b" | "--body" => {
                        if i + 1 < args.len() {
                            body = &args[i + 1];
                            i += 2;
                        } else {
                            eprintln!("Error: --body requires a value");
                            return;
                        }
                    }
                    "-u" | "--urgency" => {
                        if i + 1 < args.len() {
                            urgency = &args[i + 1];
                            i += 2;
                        } else {
                            eprintln!("Error: --urgency requires a value");
                            return;
                        }
                    }
                    "-h" | "--help" => {
                        print_notify_usage();
                        return;
                    }
                    _ => {
                        eprintln!("Unknown argument: {}", args[i]);
                        print_notify_usage();
                        return;
                    }
                }
            }

            let request = json!({
                "command": "notify",
                "payload": {
                    "workspace_id": workspace_id,
                    "title": title,
                    "body": body,
                    "urgency": urgency
                }
            });

            match send_request(request) {
                Ok(response) => {
                    if response.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
                        println!("{}", response.get("message").and_then(|v| v.as_str()).unwrap_or("Notification sent"));
                    } else {
                        eprintln!("Error: {}", response.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error"));
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "ping" => {
            let request = json!({"command": "ping"});
            match send_request(request) {
                Ok(response) => {
                    println!("{}", response.get("message").and_then(|v| v.as_str()).unwrap_or("pong"));
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "status" => {
            let request = json!({"command": "status"});
            match send_request(request) {
                Ok(response) => {
                    println!("{}", response.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown status"));
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        "-h" | "--help" => print_usage(),
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
        }
    }
}

fn print_usage() {
    println!("wmux-cli - CLI for wmux terminal multiplexer");
    println!();
    println!("Usage: wmux-cli <COMMAND> [OPTIONS]");
    println!();
    println!("Commands:");
    println!("  notify    Send a notification to wmux");
    println!("  ping      Ping the wmux IPC server");
    println!("  status    Get IPC server status");
    println!("  help      Show this help");
    println!();
    println!("Run 'wmux-cli <COMMAND> --help' for more information on a command.");
}

fn print_notify_usage() {
    println!("wmux-cli notify - Send a notification to wmux");
    println!();
    println!("Usage: wmux-cli notify [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -w, --workspace <ID>    Workspace ID (default: default)");
    println!("  -t, --title <TITLE>     Notification title (default: wmux)");
    println!("  -b, --body <BODY>       Notification body (required)");
    println!("  -u, --urgency <LEVEL>   Urgency: low, normal, critical (default: normal)");
    println!("  -h, --help              Show this help");
}