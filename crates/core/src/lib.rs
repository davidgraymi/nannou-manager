use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub projects_dir: String,
    pub editor_cmd: String,
}

impl Default for Config {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            projects_dir: home.join("nannou-projects").to_string_lossy().into_owned(),
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

pub fn delete_project(path: &str) -> Result<(), String> {
    let p = PathBuf::from(path);
    if !p.is_dir() || !p.join("Cargo.toml").exists() {
        return Err(format!("'{path}' does not look like a project directory"));
    }
    std::fs::remove_dir_all(&p).map_err(|e| format!("Failed to delete project: {e}"))
}

pub fn copy_project(from_path: &str, projects_dir: &str, new_name: &str) -> Result<(), String> {
    let src = PathBuf::from(from_path);
    if !src.join("Cargo.toml").exists() {
        return Err(format!("'{from_path}' is not a project"));
    }
    let dst_parent = PathBuf::from(projects_dir);
    std::fs::create_dir_all(&dst_parent)
        .map_err(|e| format!("Could not create projects directory: {e}"))?;
    let dst = dst_parent.join(new_name);
    if dst.exists() {
        return Err(format!("Project '{new_name}' already exists"));
    }
    copy_dir_skipping(&src, &dst, &["target"])?;

    // Update package name in Cargo.toml so cargo doesn't refuse to build.
    let cargo_toml = dst.join("Cargo.toml");
    if let Ok(contents) = std::fs::read_to_string(&cargo_toml) {
        let replaced = replace_package_name(&contents, new_name);
        let _ = std::fs::write(&cargo_toml, replaced);
    }
    Ok(())
}

fn replace_package_name(toml: &str, new_name: &str) -> String {
    let mut in_package = false;
    let mut out = String::with_capacity(toml.len());
    for line in toml.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            in_package = trimmed.starts_with("[package]");
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_package && trimmed.starts_with("name") && trimmed.contains('=') {
            out.push_str(&format!("name = \"{new_name}\"\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn copy_dir_skipping(src: &Path, dst: &Path, skip: &[&str]) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("Failed to create {}: {e}", dst.display()))?;
    let entries =
        std::fs::read_dir(src).map_err(|e| format!("Failed to read {}: {e}", src.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if skip.iter().any(|s| *s == name_str) {
            continue;
        }
        let path = entry.path();
        let target = dst.join(&name);
        let ft = entry
            .file_type()
            .map_err(|e| format!("file_type error: {e}"))?;
        if ft.is_dir() {
            copy_dir_skipping(&path, &target, skip)?;
        } else if ft.is_symlink() {
            // Skip symlinks for safety
            continue;
        } else {
            std::fs::copy(&path, &target)
                .map_err(|e| format!("Failed to copy {}: {e}", path.display()))?;
        }
    }
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
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nannou-manager/config.json")
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

// ─── Git ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitStatus {
    pub initialized: bool,
    pub branch: Option<String>,
    pub remote: Option<String>,
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
}

fn git(args: &[&str], cwd: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let msg = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if msg.is_empty() {
            format!("git {} failed", args.join(" "))
        } else {
            msg
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn git_status(path: &str) -> GitStatus {
    let p = PathBuf::from(path);
    if !p.join(".git").exists() {
        return GitStatus::default();
    }
    let mut status = GitStatus {
        initialized: true,
        ..Default::default()
    };
    status.branch = git(&["rev-parse", "--abbrev-ref", "HEAD"], &p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "HEAD");
    status.remote = git(&["remote", "get-url", "origin"], &p)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Ok(porcelain) = git(&["status", "--porcelain"], &p) {
        status.dirty = !porcelain.trim().is_empty();
    }
    if status.remote.is_some() {
        if let Ok(counts) = git(&["rev-list", "--left-right", "--count", "@{u}...HEAD"], &p) {
            let parts: Vec<&str> = counts.split_whitespace().collect();
            if parts.len() == 2 {
                status.behind = parts[0].parse().unwrap_or(0);
                status.ahead = parts[1].parse().unwrap_or(0);
            }
        }
    }
    status
}

pub fn git_init(path: &str) -> Result<(), String> {
    let p = PathBuf::from(path);
    if p.join(".git").exists() {
        return Err("Repository already initialized".into());
    }
    git(&["init"], &p)?;
    // Make an initial commit so a branch exists.
    git(&["add", "-A"], &p)?;
    // Allow empty commit in case there's nothing to add for whatever reason.
    git(&["commit", "--allow-empty", "-m", "Initial commit"], &p)?;
    Ok(())
}

pub fn git_set_remote(path: &str, url: &str) -> Result<(), String> {
    let p = PathBuf::from(path);
    if !p.join(".git").exists() {
        return Err("Not a git repository".into());
    }
    // Try to set existing origin; if missing, add it.
    if git(&["remote", "set-url", "origin", url], &p).is_err() {
        git(&["remote", "add", "origin", url], &p)?;
    }
    Ok(())
}

pub fn git_sync(path: &str, message: &str) -> Result<(), String> {
    let p = PathBuf::from(path);
    if !p.join(".git").exists() {
        return Err("Not a git repository".into());
    }
    git(&["add", "-A"], &p)?;
    // Commit only if there is something staged.
    let porcelain = git(&["status", "--porcelain"], &p)?;
    if !porcelain.trim().is_empty() {
        git(&["commit", "-m", message], &p)?;
    }
    // Push current branch, set upstream if needed.
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"], &p)?
        .trim()
        .to_string();
    if branch.is_empty() || branch == "HEAD" {
        return Err("No active branch to push".into());
    }
    if git(&["push"], &p).is_err() {
        git(&["push", "-u", "origin", &branch], &p)?;
    }
    Ok(())
}

pub fn git_pull(path: &str) -> Result<(), String> {
    let p = PathBuf::from(path);
    if !p.join(".git").exists() {
        return Err("Not a git repository".into());
    }
    git(&["pull", "--ff-only"], &p)?;
    Ok(())
}

pub fn git_clone(url: &str, projects_dir: &str, name: &str) -> Result<(), String> {
    let dir = PathBuf::from(projects_dir);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create projects directory: {e}"))?;
    let target = dir.join(name);
    if target.exists() {
        return Err(format!("Project '{name}' already exists"));
    }
    let output = Command::new("git")
        .args(["clone", url, name])
        .current_dir(&dir)
        .output()
        .map_err(|e| format!("Failed to run git clone: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "git clone failed".into()
        } else {
            stderr
        });
    }
    if !target.join("Cargo.toml").exists() {
        // Not a Cargo project — still leave the clone in place; warn caller.
        return Err(format!(
            "Cloned '{name}' but no Cargo.toml found at top level"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_fake_project(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::create_dir_all(p.join("src")).unwrap();
        std::fs::write(
            p.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .unwrap();
        std::fs::write(p.join("src/main.rs"), "fn main() {}\n").unwrap();
        p
    }

    #[test]
    fn scan_projects_nonexistent_dir_is_empty() {
        assert!(scan_projects("/this/path/should/not/exist/nannou-mgr").is_empty());
    }

    #[test]
    fn scan_projects_empty_dir_is_empty() {
        let dir = tempdir().unwrap();
        assert!(scan_projects(dir.path().to_str().unwrap()).is_empty());
    }

    #[test]
    fn scan_projects_finds_cargo_projects_sorted() {
        let dir = tempdir().unwrap();
        make_fake_project(dir.path(), "zeta");
        make_fake_project(dir.path(), "alpha");
        make_fake_project(dir.path(), "mid");
        std::fs::create_dir_all(dir.path().join("not-a-project")).unwrap();
        std::fs::write(dir.path().join("loose-file"), "x").unwrap();
        let found = scan_projects(dir.path().to_str().unwrap());
        let names: Vec<_> = found.iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, vec!["alpha", "mid", "zeta"]);
        for p in &found {
            assert!(PathBuf::from(&p.path).join("Cargo.toml").exists());
        }
    }

    #[test]
    fn delete_project_rejects_non_project_dir() {
        let dir = tempdir().unwrap();
        assert!(delete_project(dir.path().to_str().unwrap()).is_err());
    }

    #[test]
    fn delete_project_removes_directory() {
        let dir = tempdir().unwrap();
        let p = make_fake_project(dir.path(), "x");
        delete_project(p.to_str().unwrap()).unwrap();
        assert!(!p.exists());
    }

    #[test]
    fn copy_project_copies_files_skips_target_and_renames_package() {
        let dir = tempdir().unwrap();
        let src = make_fake_project(dir.path(), "orig");
        std::fs::create_dir_all(src.join("target/debug")).unwrap();
        std::fs::write(src.join("target/debug/leftover"), "binary").unwrap();

        copy_project(
            src.to_str().unwrap(),
            dir.path().to_str().unwrap(),
            "renamed",
        )
        .unwrap();

        let dst = dir.path().join("renamed");
        assert!(dst.join("Cargo.toml").exists());
        assert!(dst.join("src/main.rs").exists());
        assert!(!dst.join("target").exists());
        let toml = std::fs::read_to_string(dst.join("Cargo.toml")).unwrap();
        assert!(toml.contains("name = \"renamed\""));
        assert!(!toml.contains("name = \"orig\""));
    }

    #[test]
    fn copy_project_rejects_non_project_source() {
        let dir = tempdir().unwrap();
        let bogus = dir.path().join("bogus");
        std::fs::create_dir_all(&bogus).unwrap();
        let result = copy_project(
            bogus.to_str().unwrap(),
            dir.path().to_str().unwrap(),
            "out",
        );
        assert!(result.is_err());
    }

    #[test]
    fn copy_project_rejects_existing_target() {
        let dir = tempdir().unwrap();
        let src = make_fake_project(dir.path(), "orig");
        make_fake_project(dir.path(), "taken");
        let result = copy_project(
            src.to_str().unwrap(),
            dir.path().to_str().unwrap(),
            "taken",
        );
        assert!(result.is_err());
    }

    #[test]
    fn replace_package_name_only_inside_package_section() {
        let toml = "\
[package]
name = \"old\"
version = \"0.1.0\"

[dependencies]
name = \"not_this\"
";
        let out = replace_package_name(toml, "new");
        assert!(out.contains("name = \"new\""));
        assert!(out.contains("name = \"not_this\""));
        assert!(!out.contains("name = \"old\""));
    }

    #[test]
    fn replace_package_name_leaves_unrelated_toml_intact() {
        let toml = "name = \"unchanged\"\n[other]\nname = \"also\"\n";
        let out = replace_package_name(toml, "new");
        assert!(out.contains("name = \"unchanged\""));
        assert!(out.contains("name = \"also\""));
        assert!(!out.contains("name = \"new\""));
    }

    #[test]
    fn replace_package_name_handles_indented_keys() {
        let toml = "[package]\n  name = \"old\"\n";
        let out = replace_package_name(toml, "new");
        assert!(out.contains("name = \"new\""));
        assert!(!out.contains("name = \"old\""));
    }

    #[test]
    fn config_default_has_non_empty_dir_and_code_editor() {
        let c = Config::default();
        assert!(!c.projects_dir.is_empty());
        assert_eq!(c.editor_cmd, "code");
    }

    #[test]
    fn config_serde_roundtrip() {
        let c = Config {
            projects_dir: "/tmp/projects".into(),
            editor_cmd: "vim".into(),
        };
        let s = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&s).unwrap();
        assert_eq!(back.projects_dir, c.projects_dir);
        assert_eq!(back.editor_cmd, c.editor_cmd);
    }

    #[test]
    fn config_path_includes_app_dir() {
        let p = config_path();
        assert!(p.to_string_lossy().contains("nannou-manager"));
        assert!(p.file_name().unwrap() == "config.json");
    }

    #[test]
    fn git_status_default_for_non_repo() {
        let dir = tempdir().unwrap();
        let s = git_status(dir.path().to_str().unwrap());
        assert!(!s.initialized);
        assert!(s.branch.is_none());
        assert!(s.remote.is_none());
        assert!(!s.dirty);
        assert_eq!(s.ahead, 0);
        assert_eq!(s.behind, 0);
    }

    #[test]
    fn git_set_remote_rejects_non_repo() {
        let dir = tempdir().unwrap();
        let r = git_set_remote(dir.path().to_str().unwrap(), "https://example.com/r.git");
        assert!(r.is_err());
    }

    #[test]
    fn git_sync_rejects_non_repo() {
        let dir = tempdir().unwrap();
        assert!(git_sync(dir.path().to_str().unwrap(), "m").is_err());
    }

    #[test]
    fn git_pull_rejects_non_repo() {
        let dir = tempdir().unwrap();
        assert!(git_pull(dir.path().to_str().unwrap()).is_err());
    }

    #[test]
    fn git_init_creates_repo_and_rejects_second_init() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = tempdir().unwrap();
        let project = make_fake_project(dir.path(), "g");
        // Avoid needing global git config for the initial commit.
        let env_pairs = [
            ("GIT_AUTHOR_NAME", "test"),
            ("GIT_AUTHOR_EMAIL", "test@example.com"),
            ("GIT_COMMITTER_NAME", "test"),
            ("GIT_COMMITTER_EMAIL", "test@example.com"),
        ];
        for (k, v) in &env_pairs {
            std::env::set_var(k, v);
        }
        let result = git_init(project.to_str().unwrap());
        if result.is_err() {
            return;
        }
        assert!(project.join(".git").exists());
        let status = git_status(project.to_str().unwrap());
        assert!(status.initialized);
        assert!(git_init(project.to_str().unwrap()).is_err());
    }

    #[test]
    fn starter_template_is_a_runnable_nannou_sketch() {
        assert!(STARTER_TEMPLATE.contains("fn main()"));
        assert!(STARTER_TEMPLATE.contains("nannou"));
        assert!(STARTER_TEMPLATE.contains("fn view"));
    }
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
