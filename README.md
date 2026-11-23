# djuke

The fast, unified CLI for Java development. Manages JDKs, orchestrates build tools (Gradle & Maven), resolves dependencies, and runs single-file scripts without configuration hassles.

---

## Key Highlights

- **Seamless JDK Management**: Install, switch, list, and uninstall OpenJDK distributions (Temurin, Corretto, Zulu, GraalVM, OpenJDK, Semeru, Liberica, Microsoft, Oracle, SAPMachine) with automatic caching and zero manual `JAVA_HOME` configuration.
- **Smart Build Tool Orchestration**: Transparently wraps Gradle and Maven. Automatically detects project configurations, bootstraps required wrappers (`gradlew` / `mvnw`), primes distributions, and forwards build tasks.
- **Modern Package Management**: Add, remove, inspect (`tree`), check for updates (`outdated`), and security audit (`audit` via OSV) project dependencies across both `pom.xml` and `build.gradle(.kts)`.
- **Single-File Java Scripts**: Execute `.java` scripts instantly with automatic inline dependency resolution via `//DEPS` comments.
- **Zero Friction Onboarding**: Run `djuke install` (or `djuke setup`) to configure project JDKs and build tools in seconds.
- **Interactive or Scriptable**: Rich terminal prompts with clean output, plus full `--json` support for CI/CD pipelines.

---

## Quick Start

### 1. Create a New Project

Generate a modern, scaffolded Java project in one command:

```bash
djuke new my-app
```

Follow the interactive prompts to choose your build tool (Gradle or Maven), language (Java or Kotlin), and template (Spring Boot, Quarkus, Micronaut, Javalin, gRPC, or CLI).

Or initialize in an existing directory:

```bash
cd existing-codebase
djuke init
```

### 2. Machine & Project Onboarding

Clone any existing repository and get it running immediately:

```bash
# Detects JDK version and build tool, downloads missing JDK, and bootstraps wrappers
djuke install
```

> **Tip:** You can also pass a specific JDK version to install and prime:
> ```bash
> djuke install 21
> ```

### 3. Build, Run, and Test

Delegate standard tasks directly without worrying about which tool is used:

```bash
djuke run              # Run project application or dev task
djuke test             # Execute unit and integration tests
djuke build            # Compile and package application
djuke clean            # Clean build artifacts
```

Pass arbitrary tool flags directly:

```bash
djuke build -- --info --stacktrace
```

---

## JDK Lifecycle Management

`djuke` provides first-class JDK management via `djuke jdk` (aliased as `djuke java`).

### View Installed JDKs

Running `djuke jdk` with no arguments directly lists all locally installed JDK runtimes and marks the currently active version:

```bash
djuke jdk
```

Output:
```text
Installed JDKs:
  ✓ temurin-21.0.2         (21.0.2)           ~/.djuke/jdks/temurin-21.0.2 [active]
    corretto-17.0.10       (17.0.10)          ~/.djuke/jdks/corretto-17.0.10
    graalvm-21.0.2         (21.0.2)           ~/.djuke/jdks/graalvm-21.0.2
```

### Install a JDK

Install any major version or specific distribution:

```bash
# Install by major LTS / GA version (defaults to Eclipse Temurin)
djuke jdk install 21
djuke jdk install 17

# Install specific vendor distributions
djuke jdk install corretto@17
djuke jdk install zulu@21
djuke jdk install graalvm@21
djuke jdk install openjdk@23

# Interactive installation (picker opens if target is omitted)
djuke jdk install
```

Supported distributions: `temurin`, `corretto`, `zulu`, `graalvm`, `openjdk`, `microsoft`, `semeru`, `liberica`, `oracle`, `sapmachine`.

### Uninstall a JDK

Remove local JDKs to reclaim disk space:

```bash
# Uninstall by version or folder name
djuke jdk uninstall 17
djuke jdk uninstall temurin-21.0.2

# Interactive uninstall (displays interactive selector of installed JDKs if target is omitted)
djuke jdk uninstall
```

### Switch Active JDK Version

Switch the active Java version for your current project or across your entire machine:

```bash
# Set project-level JDK (writes to .djuke/config.toml)
djuke use 21
djuke use temurin@17

# Set global default JDK (writes to ~/.djuke/config.toml)
djuke use 21 --global

# Show currently resolved JDK runtime and detection source
djuke jdk current
```

### Search Remote Distributions

Explore remote releases from the Foojay Disco API:

```bash
# List available remote releases
djuke jdk list --remote
djuke jdk list --remote --distro corretto
```

---

## Dependency Management

`djuke` gives Java developers an `npm`/`cargo`-like dependency experience across Maven (`pom.xml`) and Gradle (`build.gradle` / `build.gradle.kts`).

### Add & Remove Dependencies

```bash
# Add by shorthand or Maven coordinates (auto-fetches latest stable version)
djuke add lombok
djuke add org.postgresql:postgresql:42.7.4

# Remove dependencies
djuke remove lombok
djuke remove org.postgresql:postgresql
```

### Search Maven Central

Search for libraries and generate instant Gradle / Maven configuration snippets:

```bash
djuke search jackson
```

### Inspect, Update & Audit

```bash
# Visualize project dependency hierarchy
djuke tree

# Check for newer package versions on Maven Central
djuke outdated

# Scan dependencies for known security vulnerabilities via OSV database
djuke audit
```

---

## Single-File Scripts (`djuke run`)

Run standalone `.java` files with automated dependency fetching:

```java
/// usr/bin/env djuke run "$0" "$@" ; exit $?
//DEPS com.google.guava:guava:33.0.0-jre
//DEPS com.google.code.gson:gson:2.10.1

import com.google.common.base.Joiner;
import com.google.gson.Gson;

public class Script {
    public static void main(String[] args) {
        var items = new String[] { "Java", "with", "djuke", "scripts!" };
        System.out.println(Joiner.on(" ").join(items));
    }
}
```

Execute directly:

```bash
djuke run Script.java
```

`djuke` resolves and downloads the specified `//DEPS` into the local cache, builds the classpath, and launches the script using the active JDK.

---

## Environment & Shell Integration

### Run Commands in Project Context

Run arbitrary commands (such as jshell, jconsole, or third-party CLIs) with `JAVA_HOME` and `PATH` automatically pointing to the project's configured JDK:

```bash
djuke exec java -version
djuke exec jshell
```

### Export Shell Environment

Generate shell commands to configure your current terminal session:

```bash
# Detects shell automatically
eval "$(djuke env)"

# Or specify shell explicitly: bash, zsh, fish, powershell, cmd, nu
djuke env --shell powershell | Invoke-Expression
```

---

## Diagnostics & Maintenance

### System Diagnostics (`djuke doctor`)

Verify toolchains, environment variables, wrappers, and network connectivity:

```bash
djuke doctor
```

### Cache Management

Manage cached JDK downloads and dependencies in `~/.djuke/cache/`:

```bash
djuke cache stats      # View total cache size and file count
djuke cache list       # List cached archive files
djuke cache clean      # Clear cached files to free up disk space
```

### Self-Update

Keep `djuke` up to date with official GitHub releases:

```bash
djuke upgrade          # Download and install the latest release
djuke upgrade --check  # Check if a new version is available
```

---

## Command Reference

Help is built into every command level via `--help` or `-h`:

```bash
djuke --help
djuke jdk --help
djuke install --help
```

| Command | Aliases | Description |
|---|---|---|
| `djuke install [spec]` | `setup` | Install required JDK and bootstrap build wrappers (e.g. `djuke install 21`) |
| `djuke new [name]` | | Create a new Java project from a template |
| `djuke init` | | Initialize a Java project in the current directory |
| `djuke run [target]` | | Run project application or a single `.java` script |
| `djuke build [args...]` | | Compile and package project |
| `djuke test [args...]` | | Run test suite |
| `djuke clean [args...]` | | Clean build outputs |
| `djuke use [spec]` | | Set active Java runtime for project or globally (`--global`) |
| `djuke jdk [subcommand]` | `java` | Manage JDKs (bare command lists installed; `install`, `uninstall`, `current`, `list`) |
| `djuke add <dep>` | | Add a dependency to the project |
| `djuke remove <dep>` | | Remove a dependency from the project |
| `djuke tree` | | Display dependency tree |
| `djuke outdated` | | Check dependencies for updates |
| `djuke audit` | | Scan dependencies for security vulnerabilities |
| `djuke search <query>` | | Search Maven Central for libraries |
| `djuke exec <cmd...>` | | Run a command with project JDK environment injected |
| `djuke env` | | Print shell export statements for `JAVA_HOME` and `PATH` |
| `djuke doctor` | | Check environment and toolchain health |
| `djuke cache <subcommand>` | | Manage downloaded archives and cache |
| `djuke upgrade` | | Upgrade djuke to the latest version |

### Global Flags

| Flag | Description |
|---|---|
| `-h, --help` | Print help information for any command or subcommand |
| `-V, --version` | Print djuke version |
| `-q, --quiet` | Suppress non-essential informational output |
| `-v, --verbose` | Enable debug and diagnostic logging |
| `--json` | Output structured JSON data for scripting and automation |

---

## License

[MIT](LICENSE)
