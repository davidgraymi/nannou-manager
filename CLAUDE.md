# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

Workspace tests (the desktop crate is excluded because Tauri needs system libs not always present locally):

```bash
cargo test --workspace --exclude nannou-manager-desktop
```

Single test:

```bash
cargo test -p nannou-manager-core scan_projects
cargo test -p nannou-manager-cli --test cli -- list_outputs_projects
```

CLI binary:

```bash
cargo run -p nannou-manager-cli -- <subcommand>
```

Desktop app (requires Tauri prerequisites; not part of the workspace test run). The UI bundle (`crates/desktop/ui/main.js`) is gitignored and built by esbuild — build it first:

```bash
(cd crates/desktop/ui && npm ci && npm run build)
cargo run -p nannou-manager-desktop
```

Coverage (matches CI; CI enforces `--fail-under-lines 70`):

```bash
cargo llvm-cov --package nannou-manager-core --package nannou-manager-cli --summary-only
```

CI runs with `RUSTFLAGS=-Dwarnings`, so warnings break the build.

## Releases

Driven by [release-plz](https://release-plz.dev): merges to `main` open a release PR; merging it cuts a single workspace tag `vX.Y.Z` and a GitHub release. `release-artifacts.yml` then builds the `nou` CLI archives and Tauri bundles (macOS .dmg, Linux .deb/.rpm/AppImage, Windows .msi) for the matrix, uploads them to the release, and pushes updated `Formula/nou.rb` + `Casks/nannou-manager.rb` to `davidgraymi/homebrew-tap`. The cask symlinks the bundled CLI to `nou`, so installing the cask supersedes the formula. All three crates share `[workspace.package].version`; do not bump individual `version = …` fields.

## Architecture

Cargo workspace with three crates:

- `crates/core` (`nannou-manager-core`) — pure library. Owns all project-management logic: scanning `projects_dir` for directories containing `Cargo.toml`, `cargo new` + `cargo add nannou` for `create_project`, copy/delete/clone, git operations, and JSON `Config` persistence under the user's config dir. No CLI/UI dependencies — only `serde`, `serde_json`, `dirs`, and `std::process::Command`. Both frontends consume this crate.
- `crates/cli` (`nannou-manager-cli`, binary `nou`) — thin `clap` wrapper. Each subcommand (`list`, `new`, `run`, `open`, `delete`, `copy`, `clone`, `git …`, `config …`) maps to one `core` function. Bare `nou` (no subcommand) launches the desktop app via `open -a "Nannou Manager"` on macOS, or by execing `nannou-manager-desktop` on Linux/Windows. Integration tests in `tests/cli.rs` use `assert_cmd` against a `tempfile` projects directory.
- `crates/desktop` (`nannou-manager-desktop`) — Tauri 2 app. `src/main.rs` exposes `core` functions as Tauri commands and adds desktop-only concerns: tracking spawned project `Child` processes in `AppState` (`running` and `compiling` maps behind `Arc<Mutex<…>>`), streaming `cargo build` artifact progress back to the UI via `Emitter`, and cancelling in-flight compiles. UI assets live in `crates/desktop/ui/`; Tauri config in `tauri.conf.json`.

Key invariants when editing:

- A directory is considered a "project" iff it contains `Cargo.toml`. All core functions enforce this and return `Result<_, String>` with user-facing messages.
- The desktop crate is the only place that holds long-lived process state. The core library spawns processes but does not retain them — keep it that way so the CLI stays simple and core stays testable without a runtime.
- Core git tests shell out to real `git`, so they require a configured `user.email` / `user.name` (CI sets these globally; do the same locally if running `cargo test -p nannou-manager-core` on a fresh machine).
