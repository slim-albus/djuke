# djuke CLI Reference

## Global Options

- `-v, --verbose`: Enable debug/trace logging.
- `-q, --quiet`: Suppress non-essential output.
- `--json`: Output machine-readable JSON for integration with IDEs and scripts.
- `--no-color`: Disable ANSI color escape sequences.
- `-h, --help`: Display help information.
- `-V, --version`: Print version information.

---

## Subcommands

### 1. `djuke doctor`
Diagnose the development environment, verifying OS/arch, directory permissions, disk space, network reachability to Foojay Disco API, installed JDKs, and wrapper readiness.

```bash
djuke doctor
djuke doctor --json
```

### 2. `djuke java`
JDK management subcommands.

```bash
# List locally installed JDKs
djuke java list

# List remote available JDKs from catalog
djuke java list --remote
djuke java list --remote --lts

# Install a specific JDK
djuke java install 21
djuke java install temurin@21
djuke java install corretto@17
djuke java install zulu@21.0.4

# Set project-local Java version (modifies djuke.toml)
djuke java use 21
djuke java use temurin@21

# Set global default Java version (modifies ~/.djuke/config.toml)
djuke java use 21 --global

# Show currently resolved Java runtime
djuke java current

# Uninstall a managed JDK
djuke java uninstall temurin-21.0.4
```

### 3. `djuke setup`
Prepares a repository for immediate development:
1. Inspects project files (`pom.xml`, `build.gradle`, `djuke.toml`).
2. Installs the required JDK if missing.
3. Detects or bootstraps the appropriate build tool wrapper (`gradlew` / `mvnw`).
4. Verifies execution sanity (`java -version`, wrapper execution).

```bash
djuke setup
```

### 4. `djuke build`, `test`, `run`, `clean`
Unified task runners that delegate directly to the detected build system wrapper:

| djuke command | Gradle Delegation | Maven Delegation |
|:---|:---|:---|
| `djuke build` | `./gradlew build` | `./mvnw clean package` |
| `djuke test` | `./gradlew test` | `./mvnw test` |
| `djuke run` | `./gradlew run` | `./mvnw exec:java` / `spring-boot:run` |
| `djuke clean` | `./gradlew clean` | `./mvnw clean` |

Pass-through arguments:
```bash
djuke build -- --info
djuke test -- --tests "*UserServiceTest"
djuke run -- --port=8080
```

### 5. `djuke exec`
Execute any arbitrary command with `JAVA_HOME` and `PATH` prepended to point to the resolved JDK:

```bash
djuke exec java -version
djuke exec javac -version
djuke exec env
```

### 6. `djuke env`
Generate shell exports for shell integration:

```bash
# Bash / Zsh
eval "$(djuke env bash)"

# PowerShell
djuke env powershell | Invoke-Expression
```

### 7. `djuke new`
Scaffold a new Java project from built-in templates with pre-configured build scripts, wrapper, sample code, and `djuke.toml`.

```bash
# Interactive or default CLI application
djuke new my-app

# Spring Boot application with Maven and Java 21
djuke new my-service --template spring-boot --build-tool maven --java 21 --package com.example.service

# Quarkus application with Gradle Kotlin DSL
djuke new my-quarkus --template quarkus --build-tool gradle-kts

# Available templates: cli, lib, spring-boot, quarkus, micronaut
# Available build tools: gradle-kts, gradle-groovy, maven
```

### 8. `djuke add`
Add a dependency to `pom.xml` or `build.gradle(.kts)` with automated scope resolution and latest-version lookup from Maven Central if unspecified.

```bash
# Add latest postgresql driver
djuke add org.postgresql:postgresql

# Add specific version with scope
djuke add org.projectlombok:lombok:1.18.30 --scope compile-only
djuke add org.junit.jupiter:junit-jupiter:5.10.2 --scope test
```

### 9. `djuke remove`
Remove a dependency from `pom.xml` or `build.gradle(.kts)`.

```bash
djuke remove org.postgresql:postgresql
djuke remove lombok
```

### 10. `djuke search`
Fast search across Maven Central Solr catalog with artifact coordinates, latest version, and packaging.

```bash
djuke search postgresql
djuke search spring-boot-starter-web --limit 5
```

### 11. `djuke cache`
Inspect, analyze, and purge cached JDK downloads and transient artifacts.

```bash
# Show cache size and file count
djuke cache stats

# List cached archive files
djuke cache list

# Clean all cached downloads
djuke cache clean
```

### 12. `djuke completions`
Generate shell completion scripts for your current shell.

```bash
# Bash
djuke completions bash > ~/.local/share/bash-completion/completions/djuke

# Zsh
djuke completions zsh > "${fpath[1]}/_djuke"

# Fish
djuke completions fish > ~/.config/fish/completions/djuke.fish

# PowerShell
djuke completions powershell >> $PROFILE
```

### 13. Single-File Java Execution (`djuke run <file.java>`)
Run single-file Java source files directly without a build file (JEP 330) using the resolved JDK:

```bash
djuke run HelloWorld.java
djuke run Script.java -- --arg1 --arg2
```
