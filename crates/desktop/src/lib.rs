use nannou_manager_core::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{async_runtime::spawn_blocking, AppHandle, Emitter, State};

struct AppState {
    // name → spawned app Child
    running: Arc<Mutex<HashMap<String, Child>>>,
    // name → cargo build Child (stdout already taken before insertion)
    compiling: Arc<Mutex<HashMap<String, Child>>>,
}

#[derive(serde::Serialize, Clone)]
struct CompileProgress {
    name: String,
    artifacts: usize,
}

#[derive(serde::Serialize, Clone)]
struct CompileResult {
    name: String,
    success: bool,
    cancelled: bool,
    error: Option<String>,
}

async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    spawn_blocking(f)
        .await
        .map_err(|e| format!("Background task failed: {e}"))
}

fn find_project(name: &str) -> Result<ProjectInfo, String> {
    let config = load_config();
    scan_projects(&config.projects_dir)
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Project '{name}' not found"))
}

#[tauri::command]
async fn list_projects() -> Vec<ProjectInfo> {
    blocking(|| scan_projects(&load_config().projects_dir))
        .await
        .unwrap_or_default()
}

#[tauri::command]
fn get_running(state: State<AppState>) -> Vec<String> {
    let mut running = state.running.lock().unwrap();
    running.retain(|_, child| matches!(child.try_wait(), Ok(None)));
    running.keys().cloned().collect()
}

#[tauri::command]
fn get_compiling(state: State<AppState>) -> Vec<String> {
    state.compiling.lock().unwrap().keys().cloned().collect()
}

#[tauri::command]
async fn create_project_cmd(name: String) -> Result<Vec<ProjectInfo>, String> {
    blocking(move || {
        let config = load_config();
        create_project(&config, &name)?;
        Ok(scan_projects(&config.projects_dir))
    })
    .await?
}

#[tauri::command]
async fn clone_project_cmd(url: String, name: String) -> Result<Vec<ProjectInfo>, String> {
    blocking(move || {
        let config = load_config();
        git_clone(&url, &config.projects_dir, &name)?;
        Ok(scan_projects(&config.projects_dir))
    })
    .await?
}

#[tauri::command]
async fn delete_project_cmd(
    name: String,
    state: State<'_, AppState>,
) -> Result<Vec<ProjectInfo>, String> {
    // Stop any running instance first.
    if let Some(mut child) = state.running.lock().unwrap().remove(&name) {
        let _ = child.kill();
    }
    if let Some(mut child) = state.compiling.lock().unwrap().remove(&name) {
        let _ = child.kill();
    }
    blocking(move || {
        let project = find_project(&name)?;
        delete_project(&project.path)?;
        Ok(scan_projects(&load_config().projects_dir))
    })
    .await?
}

#[tauri::command]
async fn copy_project_cmd(name: String, new_name: String) -> Result<Vec<ProjectInfo>, String> {
    blocking(move || {
        let project = find_project(&name)?;
        let config = load_config();
        copy_project(&project.path, &config.projects_dir, &new_name)?;
        Ok(scan_projects(&config.projects_dir))
    })
    .await?
}

#[tauri::command]
async fn git_status_cmd(name: String) -> Result<GitStatus, String> {
    blocking(move || {
        let project = find_project(&name)?;
        Ok(git_status(&project.path))
    })
    .await?
}

#[tauri::command]
async fn git_init_cmd(name: String) -> Result<GitStatus, String> {
    blocking(move || {
        let project = find_project(&name)?;
        git_init(&project.path)?;
        Ok(git_status(&project.path))
    })
    .await?
}

#[tauri::command]
async fn git_set_remote_cmd(name: String, url: String) -> Result<GitStatus, String> {
    blocking(move || {
        let project = find_project(&name)?;
        git_set_remote(&project.path, &url)?;
        Ok(git_status(&project.path))
    })
    .await?
}

#[tauri::command]
async fn git_sync_cmd(name: String, message: String) -> Result<GitStatus, String> {
    blocking(move || {
        let project = find_project(&name)?;
        let msg = if message.trim().is_empty() {
            "Update".to_string()
        } else {
            message
        };
        git_sync(&project.path, &msg)?;
        Ok(git_status(&project.path))
    })
    .await?
}

#[tauri::command]
async fn git_pull_cmd(name: String) -> Result<GitStatus, String> {
    blocking(move || {
        let project = find_project(&name)?;
        git_pull(&project.path)?;
        Ok(git_status(&project.path))
    })
    .await?
}

#[tauri::command]
fn compile_and_run_cmd(name: String, state: State<AppState>, app: AppHandle) -> Result<(), String> {
    if state.compiling.lock().unwrap().contains_key(&name) {
        return Ok(());
    }

    let config = load_config();
    let projects = scan_projects(&config.projects_dir);
    let project = projects
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Project '{name}' not found"))?
        .clone();

    let running = Arc::clone(&state.running);
    let compiling = Arc::clone(&state.compiling);

    std::thread::spawn(move || {
        let result = do_compile_and_run(&name, &project.path, &app, &running, &compiling);
        // If child was removed by stop_compile_cmd the map won't have it; clean up otherwise.
        compiling.lock().unwrap().remove(&name);

        let cancelled = matches!(&result, Err(e) if e == "Compilation cancelled");
        app.emit(
            "compile-result",
            CompileResult {
                name: name.clone(),
                success: result.is_ok(),
                cancelled,
                error: result.err().filter(|_| !cancelled),
            },
        )
        .ok();
    });

    Ok(())
}

fn do_compile_and_run(
    name: &str,
    path: &str,
    app: &AppHandle,
    running: &Arc<Mutex<HashMap<String, Child>>>,
    compiling: &Arc<Mutex<HashMap<String, Child>>>,
) -> Result<(), String> {
    let mut child = Command::new("cargo")
        .args(["build", "--release", "--message-format=json"])
        .current_dir(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start cargo build: {e}"))?;

    let stdout = child.stdout.take().unwrap();
    compiling.lock().unwrap().insert(name.to_string(), child);

    let reader = BufReader::new(stdout);
    let mut artifacts = 0usize;
    let mut exe_path: Option<String> = None;

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if json["reason"].as_str() == Some("compiler-artifact") {
            artifacts += 1;
            app.emit(
                "compile-progress",
                CompileProgress {
                    name: name.to_string(),
                    artifacts,
                },
            )
            .ok();
            if let Some(exe) = json["executable"].as_str() {
                if !exe.is_empty() {
                    exe_path = Some(exe.to_string());
                }
            }
        }
    }

    // stdout EOF: child either finished or was killed. Remove from map to get it back.
    let Some(mut build_child) = compiling.lock().unwrap().remove(name) else {
        // stop_compile_cmd already removed and killed it
        return Err("Compilation cancelled".into());
    };

    let status = build_child
        .wait()
        .map_err(|e| format!("cargo build error: {e}"))?;
    if !status.success() {
        return Err("Compilation failed".into());
    }

    let exe = exe_path.ok_or_else(|| "Could not locate compiled binary".to_string())?;
    let app_child = Command::new(&exe)
        .current_dir(path)
        .spawn()
        .map_err(|e| format!("Failed to launch project: {e}"))?;

    running.lock().unwrap().insert(name.to_string(), app_child);
    Ok(())
}

#[tauri::command]
fn stop_compile_cmd(name: String, state: State<AppState>) -> Result<(), String> {
    if let Some(mut child) = state.compiling.lock().unwrap().remove(&name) {
        child.kill().ok();
    }
    Ok(())
}

#[tauri::command]
fn stop_project_cmd(name: String, state: State<AppState>) -> Result<(), String> {
    if let Some(mut child) = state.running.lock().unwrap().remove(&name) {
        child.kill().map_err(|e| format!("Failed to stop: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
async fn open_project_cmd(name: String) -> Result<(), String> {
    blocking(move || {
        let config = load_config();
        let project = find_project(&name)?;
        open_in_editor(&config.editor_cmd, &project.path)
    })
    .await?
}

#[tauri::command]
async fn get_config_cmd() -> Config {
    blocking(load_config).await.unwrap_or_default()
}

#[tauri::command]
async fn save_config_cmd(config: Config) -> Result<Vec<ProjectInfo>, String> {
    blocking(move || {
        save_config(&config)?;
        Ok(scan_projects(&config.projects_dir))
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_progress_serializes_to_expected_shape() {
        let p = CompileProgress {
            name: "demo".into(),
            artifacts: 7,
        };
        let v: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert_eq!(v["name"], "demo");
        assert_eq!(v["artifacts"], 7);
    }

    #[test]
    fn compile_result_success_has_null_error() {
        let r = CompileResult {
            name: "demo".into(),
            success: true,
            cancelled: false,
            error: None,
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["name"], "demo");
        assert_eq!(v["success"], true);
        assert_eq!(v["cancelled"], false);
        assert!(v["error"].is_null());
    }

    #[test]
    fn compile_result_cancelled_carries_flag() {
        let r = CompileResult {
            name: "demo".into(),
            success: false,
            cancelled: true,
            error: None,
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["cancelled"], true);
        assert_eq!(v["success"], false);
        assert!(v["error"].is_null());
    }

    #[test]
    fn compile_result_failure_carries_error_message() {
        let r = CompileResult {
            name: "demo".into(),
            success: false,
            cancelled: false,
            error: Some("Compilation failed".into()),
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["error"], "Compilation failed");
    }

    #[test]
    fn app_state_starts_empty() {
        let state = AppState {
            running: Arc::new(Mutex::new(HashMap::new())),
            compiling: Arc::new(Mutex::new(HashMap::new())),
        };
        assert!(state.running.lock().unwrap().is_empty());
        assert!(state.compiling.lock().unwrap().is_empty());
    }
}

/// Build and run the Tauri desktop application. Called from this crate's own
/// `main.rs` binary and, when the CLI is built with `--features desktop`, from
/// the `nou` binary for an in-process GUI launch.
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            running: Arc::new(Mutex::new(HashMap::new())),
            compiling: Arc::new(Mutex::new(HashMap::new())),
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            get_running,
            get_compiling,
            create_project_cmd,
            clone_project_cmd,
            delete_project_cmd,
            copy_project_cmd,
            git_status_cmd,
            git_init_cmd,
            git_set_remote_cmd,
            git_sync_cmd,
            git_pull_cmd,
            compile_and_run_cmd,
            stop_compile_cmd,
            stop_project_cmd,
            open_project_cmd,
            get_config_cmd,
            save_config_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
