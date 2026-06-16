# nannou-manager

[![CI](https://github.com/davidgraymi/nannou-manager/actions/workflows/ci.yml/badge.svg)](https://github.com/davidgraymi/nannou-manager/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/davidgraymi/nannou-manager/branch/main/graph/badge.svg)](https://codecov.io/gh/davidgraymi/nannou-manager)

Manage [nannou](https://nannou.cc/) creative-coding projects from a CLI or a Tauri desktop app.

## Workspace

- `crates/core` — shared library: project scanning, copy/delete, git ops, config
- `crates/cli` — `nou` binary (the CLI)
- `crates/desktop` — Tauri desktop app

## Install

CLI only:

```bash
brew install davidgraymi/tap/nou
```

Desktop app (bundles the `nou` CLI and symlinks it onto `PATH`, à la VS Code's `code`):

```bash
brew install --cask davidgraymi/tap/nannou-manager
```

`nou <subcommand>` runs the CLI; bare `nou` launches the desktop app (when installed).

Linux: `.deb`, `.rpm`, and AppImage bundles are attached to each [GitHub release](https://github.com/davidgraymi/nannou-manager/releases). Windows: MSI installer.

## Development

```bash
cargo test --workspace --exclude nannou-manager-desktop
```

Coverage (requires `cargo-llvm-cov`):

```bash
cargo llvm-cov --package nannou-manager-core --package nannou-manager-cli --summary-only
```

## Releases

Releases are driven by [release-plz](https://release-plz.dev):

1. Merges to `main` open/update a release PR that bumps the workspace version and updates changelogs based on conventional commits.
2. Merging the release PR tags the workspace (`vX.Y.Z`) and creates a GitHub release.
3. `release-artifacts.yml` builds the CLI archives and Tauri bundles for macOS (arm64/x86_64), Linux (x86_64), and Windows (x86_64), uploads them to the release, then regenerates `Formula/nou.rb` and `Casks/nannou-manager.rb` in [`davidgraymi/homebrew-tap`](https://github.com/davidgraymi/homebrew-tap).

Required repo secrets:

- `HOMEBREW_TAP_TOKEN` — fine-grained PAT with `contents: write` on `davidgraymi/homebrew-tap`.
- `RELEASE_PLZ_TOKEN` (optional) — PAT used by release-plz so its PRs/tags trigger downstream workflows. Falls back to `GITHUB_TOKEN`.
