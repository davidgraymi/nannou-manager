use nannou_manager_core::*;
use std::fs;
use std::path::{Path, PathBuf};
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

#[test]
fn lifecycle_scan_copy_delete() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_str().unwrap();

    project(dir.path(), "first");
    let projects = scan_projects(root);
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "first");
    let first_path = projects[0].path.clone();

    copy_project(&first_path, root, "second").unwrap();
    let after_copy = scan_projects(root);
    let names: Vec<_> = after_copy.iter().map(|p| p.name.clone()).collect();
    assert_eq!(names, vec!["first", "second"]);

    delete_project(&first_path).unwrap();
    let after_delete = scan_projects(root);
    let names: Vec<_> = after_delete.iter().map(|p| p.name.clone()).collect();
    assert_eq!(names, vec!["second"]);
}

#[test]
fn copy_preserves_nested_files_and_skips_target() {
    let dir = tempdir().unwrap();
    let src = project(dir.path(), "orig");
    fs::create_dir_all(src.join("src/utils")).unwrap();
    fs::write(src.join("src/utils/helper.rs"), "// helper\n").unwrap();
    fs::write(src.join("README.md"), "# orig\n").unwrap();
    fs::create_dir_all(src.join("target/debug")).unwrap();
    fs::write(src.join("target/debug/heavy.bin"), vec![0u8; 1024]).unwrap();

    copy_project(
        src.to_str().unwrap(),
        dir.path().to_str().unwrap(),
        "renamed",
    )
    .unwrap();

    let dst = dir.path().join("renamed");
    assert!(dst.join("src/utils/helper.rs").exists());
    assert_eq!(
        fs::read_to_string(dst.join("src/utils/helper.rs")).unwrap(),
        "// helper\n"
    );
    assert!(dst.join("README.md").exists());
    assert!(!dst.join("target").exists());
    let toml = fs::read_to_string(dst.join("Cargo.toml")).unwrap();
    assert!(toml.contains("name = \"renamed\""));
    assert!(!toml.contains("name = \"orig\""));
}

#[test]
fn scan_ignores_directories_without_cargo_toml() {
    let dir = tempdir().unwrap();
    project(dir.path(), "real");
    fs::create_dir_all(dir.path().join("imposter/src")).unwrap();
    fs::write(dir.path().join("imposter/src/main.rs"), "fn main(){}").unwrap();

    let found = scan_projects(dir.path().to_str().unwrap());
    let names: Vec<_> = found.iter().map(|p| p.name.clone()).collect();
    assert_eq!(names, vec!["real"]);
}

#[test]
fn save_and_load_config_via_env_override() {
    let dir = tempdir().unwrap();
    let config_file = dir.path().join("cfg.json");
    std::env::set_var("NANNOU_MANAGER_CONFIG", &config_file);

    let cfg = Config {
        projects_dir: "/sandbox/projects".into(),
        editor_cmd: "neovim".into(),
    };
    save_config(&cfg).unwrap();
    assert!(config_file.exists());

    let loaded = load_config();
    assert_eq!(loaded.projects_dir, "/sandbox/projects");
    assert_eq!(loaded.editor_cmd, "neovim");
    assert_eq!(config_path(), config_file);

    std::env::remove_var("NANNOU_MANAGER_CONFIG");
}
