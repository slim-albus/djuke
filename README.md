# djuke

A fast JDK manager, build orchestrator, and package manager for Java.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## Quick Start

### Create a Project

```bash
djuke new my-app
```

### Work in an Existing Project

```bash
# Set up required JDK and wrappers
djuke setup

# Run the app or tests
djuke run
djuke test

# Start live reload dev mode
djuke dev

# Run any build task directly
djuke build
```

### Manage Java Versions

```bash
# Set the active JDK for this project (or --global for system default)
djuke use 21
djuke use temurin@17

# List or install runtimes
djuke java list
djuke java install 21
```

### Manage Dependencies

```bash
# Add or remove dependencies
djuke add lombok
djuke add org.postgresql:postgresql:42.7.4
djuke remove lombok

# View tree, outdated packages, or security advisories
djuke tree
djuke outdated
djuke audit
```

### Run Single-File Scripts

```bash
# Run a Java source file directly with inline dependencies (//DEPS)
djuke run Script.java
```

---

## Commands

| Command | Description |
|---|---|
| `djuke new [name]` | Create a new Java project from a template |
| `djuke init` | Initialize a Java project in the current directory |
| `djuke setup` | Install required JDK and configure build wrappers |
| `djuke run [target]` | Run project application or a single `.java` script |
| `djuke dev` | Start application in development/live-reload mode |
| `djuke build` | Execute build task |
| `djuke test` | Execute test suite |
| `djuke clean` | Execute clean task |
| `djuke use [spec]` | Set active Java runtime for project or globally (`--global`) |
| `djuke java <subcommand>` | Install, list, and manage JDK distributions |
| `djuke add <dep>` | Add a dependency to the project |
| `djuke remove <dep>` | Remove a dependency from the project |
| `djuke tree` | Display dependency tree |
| `djuke outdated` | Check dependencies for updates |
| `djuke audit` | Scan dependencies for vulnerabilities |
| `djuke search <query>` | Search Maven Central for libraries |
| `djuke doctor` | Check environment and toolchain health |
| `djuke upgrade` | Upgrade djuke to the latest version |
| `djuke exec <cmd...>` | Run a command with project JDK environment injected |
| `djuke env` | Print shell export statements for `JAVA_HOME` and `PATH` |

---

## License

[MIT](LICENSE)
