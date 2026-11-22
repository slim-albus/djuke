# djuke 🎵 ☕

> Fast, seamless JDK manager and build-tool orchestrator for Java ecosystems, written in Rust.

`djuke` gives you an **npm-like** developer experience for Java projects. Clone any Java repo, run `djuke setup` or `djuke build`, and get the exact right JDK and build environment immediately—without manual SDK installers, path juggling, or sudo access.

---

## ✨ Features

- ⚡ **Zero-Config Detection**: Automatically inspects `pom.xml`, `build.gradle`, `build.gradle.kts`, `.java-version`, and `.sdkmanrc`.
- 📦 **Multi-Vendor JDK Management**: Instant resolution and installation of Eclipse Temurin, Amazon Corretto, Azul Zulu, Microsoft OpenJDK, and more via Foojay Disco API v3.
- 🛡️ **Wrapper-First Build Orchestration**: Directly delegates tasks (`build`, `test`, `run`, `clean`) to existing `./gradlew` or `./mvnw` wrappers without downloading redundant toolchains.
- 🔒 **Process Isolation**: Injects `JAVA_HOME` and prepends `$JAVA_HOME/bin` to `PATH` ephemerally for child processes—never corrupting your global system environment.
- 🩺 **Doctor Diagnostics**: Built-in `djuke doctor` to inspect machine status, JDK paths, wrapper permissions, and network connectivity.
- 🚀 **Blazing Fast & Lightweight**: Pure Rust implementation with zero JVM bootstrapping latency.

---

## 🚀 Quick Start

```bash
# Check system health & environment
djuke doctor

# Install Java 21 (Eclipse Temurin LTS by default)
djuke java install 21

# Install Amazon Corretto 17
djuke java install corretto@17

# List remote LTS JDK releases
djuke java list --remote --lts

# Prepare current project (installs configured JDK and validates wrappers)
djuke setup

# Execute build with automatic JDK resolution
djuke build
djuke test
djuke run

# Run any arbitrary command inside the project Java environment
djuke exec java -version
```

---

## 📁 Repository Documentation

Comprehensive architectural design and references are available in [`docs/`](docs/index.md):

- [Architecture & Design Blueprint](docs/architecture.md)
- [CLI Command & Flag Reference](docs/cli-reference.md)
- [Configuration & Zero-Config Detection](docs/configuration.md)
- [JDK Catalog & Package Management](docs/jdk-management.md)
- [Build Tool Orchestration](docs/build-orchestration.md)
- [Developer & Contributing Guide](docs/development.md)
- [Architecture Decision Records (ADRs)](docs/adr/0001-workspace-crate-structure.md)

---

## 📜 License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
