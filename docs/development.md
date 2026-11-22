# Developer & Contributing Guide

## 1. Workspace Layout

The project is structured as a multi-crate Cargo workspace under `crates/`:

| Crate | Purpose | Key Dependencies |
|:---|:---|:---|
| `djuke` | CLI application binary, clap commands, terminal UX | `clap`, `tokio`, `tracing`, `colored` |
| `djuke-core` | Shared domain models, errors, event traits | `thiserror`, `serde`, `miette` |
| `djuke-platform` | OS/arch detection, platform directory resolution | `dirs` |
| `djuke-config` | `djuke.toml`, global config, project marker discovery | `serde`, `toml`, `regex` |
| `djuke-download` | Streaming HTTP, SHA-256 validation, progress bar | `reqwest`, `sha2`, `indicatif` |
| `djuke-archive` | Safe tarball/zip extraction, permissions, atomic moves | `tar`, `flate2`, `zip`, `tempfile` |
| `djuke-jdk` | Foojay Disco API v3 client, JDK package resolver | `reqwest`, `serde_json`, `chrono` |
| `djuke-build` | Wrapper-first Gradle/Maven orchestrator | `which` |
| `djuke-process` | Isolated process runner with `JAVA_HOME`/`PATH` | `tokio` (process) |
| `djuke-doctor` | Diagnostic health checks | `sysinfo`, `reqwest` |

---

## 2. Testing Guidelines

- **Unit Tests**: Every crate must have unit tests colocated in `src/` covering edge cases.
- **Integration Tests**: End-to-end tests live under `tests/` and test public CLI behavior using simulated local project fixtures.
- **Run all tests**:
  ```bash
  cargo test --workspace
  ```
- **Linting & Formatting**:
  ```bash
  cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --all -- --check
  ```
