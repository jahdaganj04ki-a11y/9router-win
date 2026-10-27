// Tauri-Commands: ersetzen die cli/cli.js-Serverrolle (Spawn, Port, PID, Autostart).

use std::process::{Command, Stdio};
use std::time::Duration;

use tauri::{AppHandle, Manager, State};

use crate::port::{find_free_port, port_in_use, wait_for_port, DEFAULT_PORT};
use crate::sqlite::ensure_sqlite_runtime;
use crate::ServerState;

use super::env::data_dir;
use super::{app_dir, build_node_env, log_path, node_exe, pid_path};

#[tauri::command]
pub fn get_data_dir() -> String {
    data_dir().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn get_port(state: State<'_, ServerState>) -> u16 {
    state.port.lock().unwrap().unwrap_or(0)
}

#[tauri::command]
pub fn is_server_running(state: State<'_, ServerState>) -> bool {
    let port = state.port.lock().unwrap().unwrap_or(0);
    port != 0 && port_in_use(port)
}

#[tauri::command]
pub fn start_server(app: AppHandle, state: State<'_, ServerState>) -> Result<u16, String> {
    // Bereits laufend? Port zurückgeben.
    if let Some(port) = *state.port.lock().unwrap() {
        if port_in_use(port) {
            return Ok(port);
        }
    }

    let data_dir = data_dir();
    std::fs::create_dir_all(data_dir.join("logs")).map_err(|e| e.to_string())?;

    // SQLite-Runtime selbstheilend sicherstellen (best effort).
    let _ = ensure_sqlite_runtime(data_dir.to_str().unwrap_or(""));

    let secrets = crate::env::ensure_secrets();

    let port = find_free_port(DEFAULT_PORT, 10)
        .ok_or_else(|| "Kein freier Port im Bereich 20128-20137".to_string())?;

    let node = node_exe();
    if !node.exists() {
        return Err(format!("Node.js Binary nicht gefunden: {}", node.display()));
    }

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .write(true)
        .open(log_path())
        .map_err(|e| e.to_string())?;
    let log_clone = log_file.try_clone().map_err(|e| e.to_string())?;

    let env = build_node_env(&data_dir, port, &secrets);

    let mut cmd = Command::new(&node);
    cmd.args(["--dns-result-order=ipv4first", "--max-old-space-size=6144", "custom-server.js"])
        .current_dir(app_dir())
        .envs(env)
        .stdout(Stdio::from(log_clone))
        .stderr(Stdio::from(log_file))
        .stdin(Stdio::null());
    super::detached(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| format!("Node.js Start fehlgeschlagen: {}", e))?;
    let pid = child.id();
    let _ = std::fs::write(pid_path(), pid.to_string());

    if !wait_for_port(port, Duration::from_secs(15), Duration::from_millis(150)) {
        let _ = child.kill();
        return Err(format!("Server erreichte Port {} nicht innerhalb 15s", port));
    }

    let url = format!("http://127.0.0.1:{}/dashboard", port);
    tauri::WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::External(
        url::Url::parse(&url).map_err(|e| e.to_string())?,
    ))
    .title("9router")
    .inner_size(1200.0, 800.0)
    .resizable(true)
    .build()
    .map_err(|e| format!("Fenster konnte nicht erstellt werden: {}", e))?;

    *state.child.lock().unwrap() = Some(child);
    *state.port.lock().unwrap() = Some(port);

    Ok(port)
}

#[tauri::command]
pub fn stop_server(state: State<'_, ServerState>) -> bool {
    let pid_opt = {
        let guard = state.child.lock().unwrap();
        guard.as_ref().map(|c| c.id())
    };
    if let Some(pid) = pid_opt {
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("taskkill").args(["/F", "/T", "/PID", &pid.to_string()]).status();
        }
        if let Ok(mut guard) = state.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    let _ = std::fs::remove_file(pid_path());
    *state.port.lock().unwrap() = None;
    true
}

#[tauri::command]
pub fn open_dashboard(app: AppHandle) -> bool {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        true
    } else {
        false
    }
}

#[tauri::command]
pub fn ensure_sqlite_runtime_cmd() -> bool {
    let data_dir = data_dir();
    ensure_sqlite_runtime(data_dir.to_str().unwrap_or(""))
}

// --- Autostart via Registry HKCU\...\Run ---

const RUN_KEY: &str = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "9router";

#[tauri::command]
pub fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        Command::new("reg")
            .args(["query", RUN_KEY, "/v", RUN_VALUE])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    false
}

#[tauri::command]
pub fn toggle_autostart(enable: bool) -> bool {
    #[cfg(target_os = "windows")]
    {
        let exe = std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().replace('/', "\\"))
            .unwrap_or_default();
        if exe.is_empty() {
            return false;
        }
        if enable {
            Command::new("reg")
                .args(["add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &exe, "/f"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else {
            Command::new("reg")
                .args(["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enable;
        false
    }
}
