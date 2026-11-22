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
