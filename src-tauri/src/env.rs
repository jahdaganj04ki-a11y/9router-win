use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const SECRET_KEYS: &[&str] = &["JWT_SECRET", "API_KEY_SECRET", "MACHINE_ID_SALT"];

pub fn data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("DATA_DIR") {
        if !d.is_empty() && !(cfg!(target_os = "windows") && d.starts_with('/')) {
            return PathBuf::from(d);
        }
    }
    if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .ok()
            .map(|a| PathBuf::from(a).join("9router"))
            .unwrap_or_else(|| PathBuf::from(env_home()).join(".9router"))
    } else {
        PathBuf::from(env_home()).join(".9router")
    }
}

fn env_home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| std::env::var("USERPROFILE").unwrap_or_default())
}

pub fn env_path() -> PathBuf {
    data_dir().join(".env")
}

pub fn read_env_file() -> HashMap<String, String> {
    let path = env_path();
    let mut map = HashMap::new();
    let Ok(content) = fs::read_to_string(&path) else { return map };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

pub fn persist_secrets(missing: &[(&str, String)]) -> std::io::Result<()> {
    let path = env_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = read_env_file();
    let mut new_lines: Vec<String> = Vec::new();
    for (k, v) in missing {
        if existing.contains_key(*k) {
            continue;
        }
        new_lines.push(format!("{}={}", k, v));
    }
    if new_lines.is_empty() {
        return Ok(());
    }
    let mut file = fs::OpenOptions::new().create(true).append(true).open(path)?;
    let _ = file.write_all(new_lines.join("\n").as_bytes());
    let _ = file.write_all(b"\n");
    Ok(())
}

#[allow(dead_code)]
pub fn load_env_file_into_process() {
    let path = env_path();
    let Ok(content) = fs::read_to_string(&path) else { return };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            let v = v.trim();
            if std::env::var(k).is_err() {
                let _ = std::env::set_var(k, v);
            }
        }
    }
}

pub fn generate_secret() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}

pub fn ensure_secrets() -> HashMap<String, String> {
    let mut existing = read_env_file();
    let mut missing: Vec<(&str, String)> = Vec::new();
    for key in SECRET_KEYS {
        if !existing.contains_key(*key) {
            let val = generate_secret();
            existing.insert(key.to_string(), val.clone());
            missing.push((key, val));
        }
    }
    let _ = persist_secrets(&missing);
    existing
}

#[allow(dead_code)]
pub fn secret_env_vars() -> Vec<(String, String)> {
    let map = ensure_secrets();
    SECRET_KEYS
        .iter()
        .filter_map(|k| map.get(*k).map(|v| (k.to_string(), v.clone())))
        .collect()
}
