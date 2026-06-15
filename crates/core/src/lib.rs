use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Child, Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub projects_dir: String,
    pub editor_cmd: String,
}

impl Default for Config {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        Self {
            projects_dir: format!("{home}/nannou-projects"),
            editor_cmd: "code".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub path: String,
}

pub fn scan_projects(dir: &str) -> Vec<ProjectInfo> {
    let path = PathBuf::from(dir);
    let Ok(entries) = std::fs::read_dir(&path) else {
        return vec![];
    };
    let mut projects: Vec<ProjectInfo> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.is_dir() && p.join("Cargo.toml").exists() {
                Some(ProjectInfo {
                    name: p.file_name()?.to_string_lossy().into_owned(),
                    path: p.to_string_lossy().into_owned(),
                })
            } else {
                None
            }
        })
        .collect();
    projects.sort_by(|a, b| a.name.cmp(&b.name));
    projects
}

pub fn create_project(config: &Config, name: &str) -> Result<(), String> {
    let dir = PathBuf::from(&config.projects_dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create projects directory: {e}"))?;

    let project_path = dir.join(name);
    if project_path.exists() {
        return Err(format!("Project '{name}' already exists"));
    }

    let status = Command::new("cargo")
        .args(["new", name])
        .current_dir(&dir)
        .status()
        .map_err(|e| format!("Failed to run cargo new: {e}"))?;
    if !status.success() {
        return Err("cargo new failed".into());
    }

    let status = Command::new("cargo")
        .args(["add", "nannou"])
        .current_dir(&project_path)
        .status()
        .map_err(|e| format!("Failed to run cargo add: {e}"))?;
    if !status.success() {
        return Err("cargo add nannou failed".into());
    }

    std::fs::write(project_path.join("src/main.rs"), STARTER_TEMPLATE)
        .map_err(|e| format!("Failed to write main.rs: {e}"))?;

    Ok(())
}

pub fn spawn_project(path: &str) -> Result<Child, String> {
    Command::new("cargo")
        .args(["run", "--release"])
        .current_dir(path)
        .spawn()
        .map_err(|e| format!("Failed to spawn project: {e}"))
}

pub fn open_in_editor(editor: &str, path: &str) -> Result<(), String> {
    Command::new(editor)
        .arg(path)
        .spawn()
        .map_err(|e| format!("Failed to open editor '{editor}': {e}"))?;
    Ok(())
}

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/nannou-manager/config.json")
}

pub fn load_config() -> Config {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_config(config: &Config) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {e}"))?;
    }
    let data = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    std::fs::write(&path, data).map_err(|e| format!("Failed to write config: {e}"))?;
    Ok(())
}

pub const STARTER_TEMPLATE: &str = r#"use nannou::prelude::*;

fn main() {
    nannou::app(model).update(update).run();
}

struct Model {}

fn model(app: &App) -> Model {
    app.new_window().view(view).build().unwrap();
    Model {}
}

fn update(_app: &App, _model: &mut Model, _update: Update) {}

fn view(app: &App, _model: &Model, frame: Frame) {
    let draw = app.draw();
    draw.background().color(PLUM);
    draw.ellipse().color(STEELBLUE);
    draw.to_frame(app, &frame).unwrap();
}
"#;
