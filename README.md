# djuke

A prototype JDK manager, build orchestrator, and package manager for Java.

---

## Features

- **Build Tool Orchestration**: Uses Gradle or Maven underneath, automatically managing wrappers and delegating build tasks.
- **JDK Management**: Installs and switches between OpenJDK distributions (OpenJDK, Temurin, Corretto, Zulu, GraalVM, and others).
- **Project Detection**: Detects Java versions and build settings from `pom.xml`, `build.gradle`, `build.gradle.kts`, `.java-version`, and `.sdkmanrc`.
- **Single-File Scripts**: Runs `.java` files directly with inline dependencies (`//DEPS`).
- **Package Operations**: Adds, removes, inspects, and audits dependencies across Maven and Gradle project files.

---

## Quick Start

### Create a Project

```bash
djuke new my-app
```

### Work in an Existing Project

```bash
# Set up required JDK and wrappers (alias: djuke install)
djuke setup

# Run the app or tests
djuke run
djuke test

# Run any build task directly
djuke build
```

### Manage Java Versions

```bash
# Set the active JDK for this project (or --global for system default)
djuke use 21
djuke use temurin@17

# List, install, or uninstall runtimes
djuke jdk
djuke jdk install 21
djuke jdk uninstall 17
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
| `djuke setup` | Install required JDK and configure build wrappers (alias: `install`) |
| `djuke run [target]` | Run project application or a single `.java` script |
| `djuke build` | Execute build task |
| `djuke test` | Execute test suite |
| `djuke clean` | Execute clean task |
| `djuke use [spec]` | Set active Java runtime for project or globally (`--global`) |
| `djuke jdk [subcommand]` | Manage JDK distributions (alias: `java`) |
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
