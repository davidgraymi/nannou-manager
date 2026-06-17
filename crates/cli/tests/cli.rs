use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

struct Sandbox {
    _tmp: TempDir,
    projects: PathBuf,
    config_file: PathBuf,
}

fn sandbox() -> Sandbox {
    let tmp = tempdir().unwrap();
    let projects = tmp.path().join("projects");
    fs::create_dir_all(&projects).unwrap();
    let config_file = tmp.path().join("config.json");
    let body = serde_json::json!({
        "projects_dir": projects.to_string_lossy(),
        "editor_cmd": "echo",
    });
    fs::write(&config_file, serde_json::to_string_pretty(&body).unwrap()).unwrap();
    Sandbox {
        _tmp: tmp,
        projects,
        config_file,
    }
}

fn make_project(dir: &Path, name: &str) {
    let p = dir.join(name);
    fs::create_dir_all(p.join("src")).unwrap();
    fs::write(
        p.join("Cargo.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
    )
    .unwrap();
    fs::write(p.join("src/main.rs"), "fn main() {}\n").unwrap();
}

fn run(sb: &Sandbox) -> Command {
    let mut cmd = Command::cargo_bin("nou").unwrap();
    cmd.env("NANNOU_MANAGER_CONFIG", &sb.config_file);
    cmd
}

#[test]
fn list_empty_projects_dir() {
    let sb = sandbox();
    run(&sb)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No projects found"));
}

#[test]
fn list_finds_projects_alphabetical() {
    let sb = sandbox();
    make_project(&sb.projects, "beta");
    make_project(&sb.projects, "alpha");
    let assert = run(&sb).arg("list").assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let alpha = stdout.find("alpha").expect("alpha not listed");
    let beta = stdout.find("beta").expect("beta not listed");
    assert!(alpha < beta, "expected alpha before beta:\n{stdout}");
}

#[test]
fn config_show_reflects_sandbox() {
    let sb = sandbox();
    run(&sb)
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            sb.projects.to_string_lossy().to_string(),
        ))
        .stdout(predicate::str::contains("echo"));
}

#[test]
fn config_set_editor_persists_to_file() {
    let sb = sandbox();
    run(&sb)
        .args(["config", "set-editor", "vim"])
        .assert()
        .success();
    let raw = fs::read_to_string(&sb.config_file).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed["editor_cmd"], "vim");
}

#[test]
fn config_set_dir_persists_to_file() {
    let sb = sandbox();
    run(&sb)
        .args(["config", "set-dir", "/some/where"])
        .assert()
        .success();
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sb.config_file).unwrap()).unwrap();
    assert_eq!(parsed["projects_dir"], "/some/where");
}

#[test]
fn delete_missing_project_exits_nonzero() {
    let sb = sandbox();
    run(&sb)
        .args(["delete", "ghost", "-y"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn delete_existing_project_removes_directory() {
    let sb = sandbox();
    make_project(&sb.projects, "doomed");
    run(&sb).args(["delete", "doomed", "-y"]).assert().success();
    assert!(!sb.projects.join("doomed").exists());
}

#[test]
fn copy_creates_new_project_with_renamed_package() {
    let sb = sandbox();
    make_project(&sb.projects, "src");
    run(&sb).args(["copy", "src", "clone"]).assert().success();
    let toml = fs::read_to_string(sb.projects.join("clone/Cargo.toml")).unwrap();
    assert!(toml.contains("name = \"clone\""));
    assert!(sb.projects.join("src/Cargo.toml").exists());
}

#[test]
fn git_status_on_non_repo_reports_no_repository() {
    let sb = sandbox();
    make_project(&sb.projects, "p");
    run(&sb)
        .args(["git", "status", "p"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No git repository"));
}

#[test]
fn unknown_subcommand_exits_with_clap_error() {
    let sb = sandbox();
    run(&sb)
        .arg("not-a-real-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized"));
}
