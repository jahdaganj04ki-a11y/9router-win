// SQLite-Runtime Self-Heal: repliziert cli/hooks/sqliteRuntime.js.
// better-sqlite3@13.0.3 (+ sql.js@1.14.1) in %APPDATA%\9router\runtime\node_modules.
// --ignore-scripts weil 13.x N-API-Prebuilds mitbringt (Node >= 22).

use std::fs;
use std::path::Path;
use std::process::Command;

const SQL_JS_VERSION: &str = "1.14.1";
const BETTER_SQLITE3_VERSION: &str = "13.0.3";

fn runtime_dir(data_dir: &str) -> std::path::PathBuf {
    Path::new(data_dir).join("runtime")
}

fn runtime_node_modules(data_dir: &str) -> std::path::PathBuf {
    runtime_dir(data_dir).join("node_modules")
}

fn write_runtime_package_json(dir: &Path) -> std::io::Result<()> {
    let pkg_path = dir.join("package.json");
    if pkg_path.exists() {
        return Ok(());
    }
    let json = serde_json::json!({
        "name": "9router-runtime",
        "version": "1.0.0",
        "private": true,
        "description": "User-writable runtime deps for 9router (better-sqlite3 native binary)"
    });
    fs::write(pkg_path, serde_json::to_string_pretty(&json).unwrap())
}

fn module_installed(data_dir: &str, name: &str) -> bool {
    runtime_node_modules(data_dir).join(name).join("package.json").exists()
}

pub fn ensure_sqlite_runtime(data_dir: &str) -> bool {
    let runtime = runtime_dir(data_dir);
    if fs::create_dir_all(&runtime).is_err() {
        return false;
    }
    if write_runtime_package_json(&runtime).is_err() {
        return false;
    }

    let node_exe = super::resources_dir().join("node").join("node.exe");
    let npm_cmd = super::resources_dir().join("node").join("npm.cmd");
    if !node_exe.exists() {
        eprintln!("[sqlite-runtime] Node.js binary fehlt: {}", node_exe.display());
        return false;
    }

    // sql.js zuerst (wasm-Fallback), dann better-sqlite3 mit --ignore-scripts (N-API Prebuilds).
    let pkgs = [format!("sql.js@{}", SQL_JS_VERSION), format!("better-sqlite3@{}", BETTER_SQLITE3_VERSION)];
    let mut cmd = Command::new(&npm_cmd);
    cmd.args(["install"])
        .args(&pkgs)
        .args(["--no-audit", "--no-fund", "--prefer-online", "--ignore-scripts"])
        .current_dir(&runtime);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x00000008 | 0x00000200); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    }

    match cmd.output() {
        Ok(output) => {
            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                eprintln!("[sqlite-runtime] npm install fehlgeschlagen: {}", err);
                return false;
            }
        }
        Err(e) => {
            eprintln!("[sqlite-runtime] npm nicht ausführbar: {}", e);
            return false;
        }
    }

    let ok = module_installed(data_dir, "sql.js") || module_installed(data_dir, "better-sqlite3");
    if ok {
        eprintln!("[sqlite-runtime] SQLite-Runtime bereit unter {}", runtime.display());
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_are_pinned() {
        assert_eq!(BETTER_SQLITE3_VERSION, "13.0.3");
        assert_eq!(SQL_JS_VERSION, "1.14.1");
    }
}
