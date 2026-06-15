use std::path::Path;
use std::process::Command;

fn main() {
    build_ui();
    tauri_build::build();
}

fn build_ui() {
    let ui_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");

    println!("cargo:rerun-if-changed=ui/src");
    println!("cargo:rerun-if-changed=ui/index.html");
    println!("cargo:rerun-if-changed=ui/style.css");
    println!("cargo:rerun-if-changed=ui/package.json");
    println!("cargo:rerun-if-changed=ui/package-lock.json");

    let node_modules = ui_dir.join("node_modules");
    if !node_modules.exists() {
        run(&ui_dir, "npm", &["install"], "npm install");
    }
    run(&ui_dir, "npm", &["run", "build"], "npm run build");
}

fn run(cwd: &Path, program: &str, args: &[&str], label: &str) {
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap_or_else(|e| panic!("failed to execute `{label}`: {e}"));
    if !status.success() {
        panic!("`{label}` exited with status {status}");
    }
}
