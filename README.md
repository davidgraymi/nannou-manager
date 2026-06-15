# nannou-manager

[![CI](https://github.com/davidgraymi/nannou-manager/actions/workflows/ci.yml/badge.svg)](https://github.com/davidgraymi/nannou-manager/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/davidgraymi/nannou-manager/branch/main/graph/badge.svg)](https://codecov.io/gh/davidgraymi/nannou-manager)

Manage [nannou](https://nannou.cc/) creative-coding projects from a CLI or a Tauri desktop app.

## Workspace

- `crates/core` — shared library: project scanning, copy/delete, git ops, config
- `crates/cli` — `nannou-manager` binary
- `crates/desktop` — Tauri desktop app

## Development

```bash
cargo test --workspace --exclude nannou-manager-desktop
```

Coverage (requires `cargo-llvm-cov`):

```bash
cargo llvm-cov --package nannou-manager-core --package nannou-manager-cli --summary-only
```
