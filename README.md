# djuke

> Prototype JDK manager and build-tool orchestrator for Java ecosystems, written in Rust.

`djuke` is an experimental prototype providing a streamlined, npm-like developer experience for Java projects. Clone any Java repository, run `djuke setup` or `djuke build`, and get the exact right JDK and build environment immediately—without manual SDK installers, path juggling, or administrator privileges.

---

## Features

- **Zero-Config Detection**: Automatically inspects `pom.xml`, `build.gradle`, `build.gradle.kts`, `.java-version`, and `.sdkmanrc`.
- **Multi-Vendor JDK Management**: Automatic resolution and installation of Temurin, Corretto, Zulu, Microsoft, Semeru, GraalVM, Liberica, Oracle, and SapMachine via Foojay Disco API v3, with dynamic `latest` and `lts` alias support.
- **Build Tool Selection**: Seamlessly choose between Gradle and Maven via `--tool`, `djuke.toml`, or project auto-detection.
- **Arbitrary Task Passthrough**: Any task or goal (e.g. `djuke bootJar`, `djuke compileJava`, `djuke spotlessApply`, `djuke dependency:tree`) is forwarded directly to the build tool with `JAVA_HOME` injected.
- **Dedicated Tool Execution**: Direct access to `djuke gradle` (daemon stop/status, wrapper generation, custom tasks) and `djuke maven` (wrapper generation, phases/goals).
- **Process Isolation**: Ephemerally injects `JAVA_HOME` and prepends `$JAVA_HOME/bin` to `PATH` for child processes without modifying global system environment variables.
- **Dependency Insights**: Interactive dependency tree (`djuke tree`), version upgrade checks (`djuke outdated`), and vulnerability security auditing via OSV.dev (`djuke audit`).
- **Project Bootstrapping**: Scaffolding for `cli`, `lib`, `spring-boot`, `quarkus`, and `micronaut` templates via `djuke new` and in-place `djuke init`.
- **Diagnostics**: Built-in `djuke doctor` to inspect machine status, JDK store health, wrapper permissions, and network connectivity.

---

## Quick Start

```bash
# Check system health and environment
djuke doctor

# Install the latest GA or LTS JDK for any distribution
djuke java install latest
djuke java install lts --distro zulu

# Initialize a new project in the current directory
djuke init --template spring-boot --build-tool maven --distro temurin

# Prepare current project (installs configured JDK and validates wrappers)
djuke setup

# Start development / live-reload mode
djuke dev

# Run project build or test suite (supports --tool, --offline, -U, -P, -D, --parallel)
djuke build --offline
djuke test -DskipTests=false

# Run arbitrary tasks or plugin goals directly
djuke bootJar
djuke spotlessApply
djuke dependency:tree

# Manage Gradle daemons directly
djuke gradle status
djuke gradle stop

# Inspect project dependencies and security advisories
djuke tree
djuke outdated
djuke audit

# Run a single Java source file directly (JEP 330)
djuke run HelloWorld.java

# Run any arbitrary command inside the project Java environment
djuke exec java -version
```

---

## Documentation

Comprehensive design specifications and guides are available in [`docs/`](docs/index.md):

- [Architecture & Design Blueprint](docs/architecture.md)
- [CLI Command & Flag Reference](docs/cli-reference.md)
- [Configuration & Zero-Config Detection](docs/configuration.md)
- [JDK Catalog & Package Management](docs/jdk-management.md)
- [Build Tool Orchestration](docs/build-orchestration.md)
- [Project Templates](docs/templates.md)
- [Package Management & Insights](docs/package-management.md)
- [Developer & Contributing Guide](docs/development.md)
- [Architecture Decision Records (ADRs)](docs/adr/0001-workspace-crate-structure.md)

---

## License

Dual-licensed under MIT OR Apache-2.0.
