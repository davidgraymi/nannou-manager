use nannou_manager_core::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn project(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    fs::create_dir_all(p.join("src")).unwrap();
    fs::write(
        p.join("Cargo.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
    )
    .unwrap();
    fs::write(p.join("src/main.rs"), "fn main() {}\n").unwrap();
    p
}

fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

fn set_git_identity_env() {
    for (k, v) in [
        ("GIT_AUTHOR_NAME", "test"),
        ("GIT_AUTHOR_EMAIL", "test@example.com"),
        ("GIT_COMMITTER_NAME", "test"),
        ("GIT_COMMITTER_EMAIL", "test@example.com"),
    ] {
        std::env::set_var(k, v);
    }
}

#[test]
fn init_then_status_then_remote_then_dirty() {
    if !git_available() {
        return;
    }
    set_git_identity_env();

    let dir = tempdir().unwrap();
    let p = project(dir.path(), "proj");
    let path = p.to_str().unwrap();

    let before = git_status(path);
    assert!(!before.initialized);

    git_init(path).unwrap();
    let after_init = git_status(path);
    assert!(after_init.initialized);
    assert!(after_init.branch.is_some());
    assert!(after_init.remote.is_none());
    assert!(!after_init.dirty);

    git_set_remote(path, "https://example.com/proj.git").unwrap();
    let after_remote = git_status(path);
    assert_eq!(
        after_remote.remote.as_deref(),
        Some("https://example.com/proj.git")
    );

    git_set_remote(path, "https://example.com/proj-v2.git").unwrap();
    let after_change = git_status(path);
    assert_eq!(
        after_change.remote.as_deref(),
        Some("https://example.com/proj-v2.git")
    );

    fs::write(p.join("new.txt"), "hello").unwrap();
    let dirty = git_status(path);
    assert!(dirty.dirty);
}

#[test]
fn init_rejects_double_init() {
    if !git_available() {
        return;
    }
    set_git_identity_env();

    let dir = tempdir().unwrap();
    let p = project(dir.path(), "p");
    let path = p.to_str().unwrap();

    git_init(path).unwrap();
    assert!(git_init(path).is_err());
}

#[test]
fn sync_commits_locally_even_when_push_fails() {
    if !git_available() {
        return;
    }
    set_git_identity_env();

    let dir = tempdir().unwrap();
    let p = project(dir.path(), "p");
    let path = p.to_str().unwrap();
    git_init(path).unwrap();
    fs::write(p.join("change.txt"), "hello").unwrap();

    // No remote configured; sync's push step should fail but the commit must land.
    let _ = git_sync(path, "test commit");

    let log = Command::new("git")
        .args(["log", "--oneline"])
        .current_dir(&p)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&log.stdout);
    assert!(
        stdout.contains("test commit"),
        "expected 'test commit' in log, got:\n{stdout}"
    );
}

#[test]
fn pull_rejects_non_repo() {
    let dir = tempdir().unwrap();
    let result = git_pull(dir.path().to_str().unwrap());
    assert!(result.is_err());
}

#[test]
fn clone_rejects_existing_target() {
    let dir = tempdir().unwrap();
    let p = project(dir.path(), "taken");
    assert!(p.exists());
    let result = git_clone(
        "https://example.com/whatever.git",
        dir.path().to_str().unwrap(),
        "taken",
    );
    assert!(result.is_err());
}
