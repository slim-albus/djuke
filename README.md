# djuke

> **The modern developer toolchain for Java.**  
> Fast JDK manager, build orchestrator, single-file script runner, and package manager in a single native binary.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-windows%20%7C%20macos%20%7C%20linux-lightgrey)](https://github.com/slim-albus/djuke)

---

## Why djuke?

`djuke` gives Java the streamlined developer experience of `bun`, `uv`, and `npm`:

- **Zero-Config Detection**: Automatically detects JDK versions and build tools from `pom.xml`, `build.gradle.kts`, `build.gradle`, `.java-version`, or `.sdkmanrc`.
- **Zero-Download Wrappers**: Generates Gradle and Maven wrappers with embedded offline assets in `< 5ms` with smart version compatibility (e.g. Gradle 9.9.2 for Java 24–27+).
- **10 OpenJDK Distributions**: Install and switch seamlessly between `openjdk` (default), `temurin`, `corretto`, `zulu`, `microsoft`, `semeru`, `graalvm`, `liberica`, `oracle`, and `sapmachine` via the Foojay Disco API.
- **Single-File Script Runner**: Run standalone `.java` scripts directly (`djuke run Script.java`) with inline dependencies (`//DEPS com.google.code.gson:gson:2.11.0` or `//DEPS gson`), auto-downloading and caching JARs.
- **Package Management**: Add and remove dependencies (`djuke add`, `djuke remove`) across Kotlin DSL, Groovy, and Maven XML with version resolution from Maven Central.
- **Security & Insights**: Inspect dependency trees (`djuke tree`), check for outdated libraries (`djuke outdated`), and scan vulnerabilities (`djuke audit`) via OSV.dev.
- **Live Development**: Launch hot-reloading dev modes (`djuke dev`) for Spring Boot, Quarkus, Micronaut, and Javalin.
- **Self-Updating**: Keep djuke up to date with a single command (`djuke upgrade`).

---

## Quick Start

### 1. Create a New Project

```bash
# Interactive project generator with curated dependencies & test libraries
djuke new my-app

# Or create non-interactively
djuke new my-service --template spring-boot --build-tool gradle --java 21
```

### 2. Work in Any Java Repo

Clone any Java repository and run:

```bash
# Automatically install required JDK and prime build wrappers
djuke setup

# Start live reload development mode
djuke dev

# Run application or test suite
djuke run
djuke test

# Execute any Gradle task or Maven goal directly
djuke build
djuke bootJar
djuke spotlessApply
```

### 3. Run Single-File Java Scripts (with Inline Dependencies)

Create `hello.java`:

```java
//DEPS com.google.code.gson:gson:2.11.0
//JAVA 21

import com.google.gson.Gson;
import java.util.Map;

public class hello {
    public static void main(String[] args) {
        System.out.println(new Gson().toJson(Map.of("message", "Hello from djuke!")));
    }
}
```

Run it immediately:

```bash
djuke run hello.java
# Or simply:
djuke hello.java
```

`djuke` automatically resolves and caches all dependencies from Maven Central, sets the classpath, and launches the script.

### 4. Manage Java Runtimes

```bash
# Switch active JDK for the project (or use --global for system-wide default)
djuke use 21
djuke use temurin@17
djuke use latest --global

# List installed runtimes or install new ones
djuke java list
djuke java install 21 --distro openjdk
djuke java install latest --distro graalvm
```

### 5. Manage Dependencies

```bash
# Add dependencies (supports coordinates or popular aliases like lombok, guava, postgres)
djuke add lombok
djuke add org.postgresql:postgresql:42.7.4

# Remove dependencies
djuke remove lombok

# Inspect dependencies, check for updates, and audit security
djuke tree
djuke outdated
djuke audit
```

### 6. Upgrade djuke

```bash
# Upgrade djuke directly to the latest GitHub release
djuke upgrade

# Or check for updates without installing
djuke upgrade --check
```

---

## CLI Overview

| Command | Description |
|---|---|
| `djuke new [name]` | Create a new Java project from industry templates (Spring Boot, Quarkus, Micronaut, Javalin, CLI, Lib, etc.) |
| `djuke init` | Initialize a Java project template in the current directory |
| `djuke setup` | Install project-required JDK and prime build tool wrappers |
| `djuke run [target]` | Run project application or execute single-file script (`Script.java`) |
| `djuke dev` | Start application in development/live-reload mode |
| `djuke build` | Execute project build task (`gradle build` / `mvn package`) |
| `djuke test` | Execute test suite |
| `djuke use [spec]` | Set active Java runtime for project or globally (`--global`) |
| `djuke java <subcommand>` | Install, list, uninstall, and search JDK distributions |
| `djuke add <dep>` | Add dependency to `pom.xml`, `build.gradle.kts`, or `build.gradle` |
| `djuke remove <dep>` | Remove dependency from project build files |
| `djuke tree` | Render colorized tree of project dependencies |
| `djuke outdated` | Check dependencies for available updates on Maven Central |
| `djuke audit` | Scan dependencies for known security vulnerabilities via OSV.dev |
| `djuke search <query>` | Search Maven Central for packages and libraries |
| `djuke doctor` | Run diagnostic health check on machine and toolchain |
| `djuke upgrade` | Upgrade djuke binary to the latest release |
| `djuke exec <cmd...>` | Execute any command with project `JAVA_HOME` and `PATH` injected |
| `djuke env` | Output shell export statements for `JAVA_HOME` and `PATH` |

---

## Documentation

Comprehensive guides and architectural specifications are available in the [`docs/`](docs/index.md) folder:

- [Architecture & Design Blueprint](docs/architecture.md)
- [CLI Reference](docs/cli-reference.md)
- [Commands Specification](docs/commands-specification.md)
- [Configuration & Project Markers](docs/configuration.md)
- [JDK Management & Disco API](docs/jdk-management.md)
- [Build Tool Orchestration](docs/build-orchestration.md)
- [Project Templates](docs/templates.md)
- [Package Management & Script Runner](docs/package-management.md)

---

## License

This project is licensed under the [MIT License](LICENSE).
