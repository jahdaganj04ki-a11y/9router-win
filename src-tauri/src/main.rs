#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod env;
mod port;
mod single_instance;
mod sqlite;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{
    generate_handler, Manager, RunEvent, State, WebviewUrl, WindowEvent,
};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Builder, WebviewWindowBuilder};

use crate::port::{find_free_port, DEFAULT_PORT};
use crate::single_instance::SingleInstance;
use crate::sqlite::ensure_sqlite_runtime;

const APP_NAME: &str = "9router";
const CLOUD_URL: &str = "https://9router.com";

pub struct ServerState {
    pub port: Mutex<Option<u16>>,
    pub child: Mutex<Option<std::process::Child>>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            port: Mutex::new(None),
            child: Mutex::new(None),
        }
    }
}

// --- Pfade (mirror src/lib/dataDir.js & cli/cli.js conventions) ---

pub fn data_dir() -> PathBuf {
    std::env::var("APPDATA")
        .ok()
        .map(|appdata| PathBuf::from(appdata).join(APP_NAME))
        .unwrap_or_else(|| std::env::temp_dir().join(APP_NAME))
}

pub fn resources_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default()
}

pub fn node_exe() -> PathBuf {
    resources_dir().join("resources").join("node").join("node.exe")
}

pub fn npm_cmd() -> PathBuf {
    resources_dir().join("resources").join("node").join("npm.cmd")
}

pub fn app_dir() -> PathBuf {
    resources_dir().join("resources").join("app")
}

pub fn pid_path() -> PathBuf {
    data_dir().join("9router.pid")
}

pub fn log_path() -> PathBuf {
    data_dir().join("logs").join("9router.log")
}

pub fn secrets_path() -> PathBuf {
    data_dir().join(".env")
}

// --- Secrets: JWT_SECRET, API_KEY_SECRET, MACHINE_ID_SALT als KEY=VALUE ---

pub fn load_or_create_secrets(path: &std::path::Path) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    if let Ok(text) = fs::read_to_string(path) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }

    let mut changed = false;
    for key in ["JWT_SECRET", "API_KEY_SECRET", "MACHINE_ID_SALT"] {
        if map.get(key).map_or(true, |v| v.is_empty()) {
            map.insert(key.to_string(), uuid::Uuid::new_v4().to_string());
            changed = true;
        }
    }

    if changed {
        let mut lines: Vec<String> = map.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        lines.sort();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, lines.join("\n") + "\n");
    }

    map
}

// --- Node-Prozess Umgebung ---

pub fn build_node_env(data_dir: &PathBuf, port: u16, secrets: &HashMap<String, String>) -> HashMap<String, String> {
    let mut env_map: HashMap<String, String> = std::env::vars().collect();
    let base_url = format!("http://127.0.0.1:{}", port);

    let runtime_nm = data_dir.join("runtime").join("node_modules");
    let app_nm = app_dir().join("node_modules");
    let existing = env_map.get("NODE_PATH").cloned().unwrap_or_default();
    let parts: Vec<String> = [
        runtime_nm.to_string_lossy().into_owned(),
        app_nm.to_string_lossy().into_owned(),
        existing,
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect();
    env_map.insert("NODE_PATH".into(), parts.join(";"));

    env_map.insert("DATA_DIR".into(), data_dir.to_string_lossy().into_owned());
    env_map.insert("PORT".into(), port.to_string());
    env_map.insert("HOSTNAME".into(), "127.0.0.1".into());
    env_map.insert("NODE_ENV".into(), "production".into());
    env_map.insert("BASE_URL".into(), base_url.clone());
    env_map.insert("NEXT_PUBLIC_BASE_URL".into(), base_url.clone());
    env_map.insert("CLOUD_URL".into(), CLOUD_URL.into());
    env_map.insert("NEXT_PUBLIC_CLOUD_URL".into(), CLOUD_URL.into());
    env_map.insert("AUTH_COOKIE_SECURE".into(), "false".into());
    env_map.insert("REQUIRE_API_KEY".into(), "false".into());
    env_map.insert("ENABLE_REQUEST_LOGS".into(), "false".into());
    env_map.insert("OBSERVABILITY_ENABLED".into(), "true".into());

    for key in ["JWT_SECRET", "API_KEY_SECRET", "MACHINE_ID_SALT"] {
        if let Some(value) = secrets.get(key) {
            env_map.insert(key.into(), value.clone());
        }
    }

    env_map
}

/// DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP auf Windows (Node `detached: true` Äquivalent).
pub fn detached(cmd: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x00000008 | 0x00000200);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = cmd;
}

// --- Bootstrap: Server starten + Fenster öffnen ---

fn bootstrap(app: &AppHandle, state: &State<'_, ServerState>) -> Result<u16, String> {
    // Bereits laufend?
    if let Some(port) = state.port.lock().unwrap().as_ref().copied() {
        if crate::port::port_in_use(port) {
            return Ok(port);
        }
    }

    let data_dir = data_dir();
    fs::create_dir_all(&data_dir).ok();
    fs::create_dir_all(data_dir.join("logs")).ok();

    let _ = ensure_sqlite_runtime(data_dir.to_str().unwrap_or(""));

    let secrets = load_or_create_secrets(&secrets_path());

    let port = find_free_port(DEFAULT_PORT, 10)
        .ok_or_else(|| "Kein freier Port im Bereich 20128-20137".to_string())?;

    let node = node_exe();
    if !node.exists() {
        return Err(format!("Node.js Binary nicht gefunden: {}", node.display()));
    }

    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .write(true)
        .open(log_path())
        .map_err(|e| e.to_string())?;
    let log_clone = log_file.try_clone().map_err(|e| e.to_string())?;

    let env_map = build_node_env(&data_dir, port, &secrets);

    let mut cmd = Command::new(&node);
    cmd.args(["--dns-result-order=ipv4first", "--max-old-space-size=6144", "custom-server.js"])
        .current_dir(app_dir())
        .envs(env_map)
        .stdout(std::process::Stdio::from(log_clone))
        .stderr(std::process::Stdio::from(log_file))
        .stdin(std::process::Stdio::null());
    detached(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| format!("Node.js Start fehlgeschlagen: {}", e))?;
    let pid = child.id();
    let _ = fs::write(pid_path(), pid.to_string());

    if !crate::port::wait_for_port(port, Duration::from_secs(15), Duration::from_millis(150)) {
        let _ = child.kill();
        return Err(format!("Server erreichte Port {} nicht innerhalb 15s", port));
    }

    let url = format!("http://127.0.0.1:{}/dashboard", port);
    WebviewWindowBuilder::new(app, "main", WebviewUrl::External(
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

fn stop_server(state: &State<'_, ServerState>) {
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
    let _ = fs::remove_file(pid_path());
    *state.port.lock().unwrap() = None;
}

fn main() {
    // Single-Instance: zweite Instanz bringt erstes Fenster nach vorne und beendet sich.
    let _single = match SingleInstance::acquire() {
        Some(s) => s,
        None => return,
    };

    Builder::default()
        .manage(ServerState::default())
        .setup(|app| {
            let app_handle = app.handle();
            let state: State<'_, ServerState> = app.state();
            if let Err(e) = bootstrap(&app_handle, &state) {
                eprintln!("[9router] Start fehlgeschlagen: {}", e);
            }

            // --- System Tray ---
            let dashboard = MenuItemBuilder::with_id("dashboard", "Dashboard öffnen")
                .build(app_handle)?;
            let autostart = MenuItemBuilder::with_id("autostart", "Autostart")
                .build(app_handle)?;
            let exit = MenuItemBuilder::with_id("exit", "Beenden")
                .build(app_handle)?;

            let menu = MenuBuilder::new(app_handle)
                .items(&[&dashboard, &autostart, &exit])
                .build()?;

            let tray = TrayIconBuilder::new()
                .menu(&menu)
                .build(app_handle)?;

            tray.on_menu_event(move |app, event| {
                if event.id() == "dashboard" {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                } else if event.id() == "autostart" {
                    let _ = commands::toggle_autostart(true);
                } else if event.id() == "exit" {
                    if let Some(state) = app.try_state::<crate::ServerState>() {
                        stop_server(&state);
                    }
                    app.exit(0);
                }            });

            tray.on_tray_icon_event(move |_tray, event| {
                if matches!(
                    event,
                    TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. }
                ) {
                    if let Some(window) = _tray.app_handle().get_webview_window("main") {
                        let is_visible = window.is_visible().unwrap_or(false);
                        if is_visible {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::is_server_running,
            commands::get_port,
            commands::get_data_dir,
            commands::open_dashboard,
            commands::ensure_sqlite_runtime_cmd,
            commands::is_autostart_enabled,
            commands::toggle_autostart,
        ])
        .on_window_event(|app, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                // Fenster verstecken statt zerstören: Server läuft im Hintergrund weiter.
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Fehler beim Erstellen der 9router-Anwendung")
        .run(|app: &tauri::AppHandle, event| {
            if let RunEvent::ExitRequested { .. } = event {
                if let Some(state) = app.try_state::<ServerState>() {
                    stop_server(&state);
                }
            }
        });
}
