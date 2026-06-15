#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use nannou_manager_core::*;
use std::collections::HashMap;
use std::process::Child;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    running: Mutex<HashMap<String, Child>>,
}

#[tauri::command]
fn list_projects() -> Vec<ProjectInfo> {
    scan_projects(&load_config().projects_dir)
}

#[tauri::command]
fn get_running(state: State<AppState>) -> Vec<String> {
    let mut running = state.running.lock().unwrap();
    running.retain(|_, child| matches!(child.try_wait(), Ok(None)));
    running.keys().cloned().collect()
}

#[tauri::command]
fn create_project_cmd(name: String) -> Result<Vec<ProjectInfo>, String> {
    let config = load_config();
    create_project(&config, &name)?;
    Ok(scan_projects(&config.projects_dir))
}

#[tauri::command]
fn run_project_cmd(name: String, state: State<AppState>) -> Result<(), String> {
    let config = load_config();
    let projects = scan_projects(&config.projects_dir);
    let project = projects
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Project '{name}' not found"))?;

    let mut running = state.running.lock().unwrap();
    if running.contains_key(&name) {
        return Ok(());
    }
    let child = spawn_project(&project.path)?;
    running.insert(name, child);
    Ok(())
}

#[tauri::command]
fn stop_project_cmd(name: String, state: State<AppState>) -> Result<(), String> {
    let mut running = state.running.lock().unwrap();
    if let Some(mut child) = running.remove(&name) {
        child.kill().map_err(|e| format!("Failed to stop: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
fn open_project_cmd(name: String) -> Result<(), String> {
    let config = load_config();
    let projects = scan_projects(&config.projects_dir);
    let project = projects
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Project '{name}' not found"))?;
    open_in_editor(&config.editor_cmd, &project.path)
}

#[tauri::command]
fn get_config_cmd() -> Config {
    load_config()
}

#[tauri::command]
fn save_config_cmd(config: Config) -> Result<Vec<ProjectInfo>, String> {
    save_config(&config)?;
    Ok(scan_projects(&config.projects_dir))
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            running: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            get_running,
            create_project_cmd,
            run_project_cmd,
            stop_project_cmd,
            open_project_cmd,
            get_config_cmd,
            save_config_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
