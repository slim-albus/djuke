# djuke Configuration & Discovery Specification

## 1. Zero-Config Auto-Discovery

`djuke` operates in a **zero-config by default** mode. If no `djuke.toml` is present in the workspace, `djuke` automatically scans the directory tree upwards for standard Java markers:

1. **Build Tool Marker**:
   - `gradlew` / `gradlew.bat` -> Gradle with wrapper
   - `mvnw` / `mvnw.cmd` -> Maven with wrapper
   - `build.gradle` / `build.gradle.kts` -> Gradle
   - `pom.xml` -> Maven
2. **Java Version Marker**:
   - `.java-version` (asdf, jenv, mise)
   - `.sdkmanrc` (`java=21.0.4-tem`)
   - `pom.xml` (`<java.version>`, `<maven.compiler.target>`, `<maven.compiler.release>`)
   - `build.gradle(.kts)` (`jvmToolchain(21)`, `sourceCompatibility = JavaVersion.VERSION_21`)
   - Fallback: Defaults to LTS JDK 21 (Temurin).

---

## 2. Configuration Precedence

When resolving settings (Java version, build tool, task commands), values are resolved with the following strict hierarchy:

1. **CLI Explicit Arguments** (e.g., `djuke --java 17 build`)
2. **Environment Variables** (e.g., `DJUKE_JAVA=17`)
3. **Project Manifest** (`djuke.toml` or `juke.toml`)
4. **Project Markers** (`pom.xml`, `build.gradle`, `.java-version`, `.sdkmanrc`)
5. **Global Configuration** (`~/.djuke/config.toml`)
6. **Built-in System Fallbacks** (Default LTS Temurin 21)

---

## 3. `djuke.toml` Schema

```toml
[project]
name = "my-service"

[java]
version = "21"
distribution = "temurin" # temurin | corretto | zulu | microsoft | semeru

[build]
tool = "gradle"          # gradle | maven
version = "8.10"         # used if wrapper needs generation
wrapper = true           # prefer gradlew/mvnw

[tasks]
build = "build"
test = "test"
run = "bootRun"
clean = "clean"
lint = "spotlessCheck"
format = "spotlessApply"

[env]
GRADLE_OPTS = "-Xmx2g"
```

---

## 4. Global Configuration (`~/.djuke/config.toml`)

```toml
[java]
default_version = "21"
default_distribution = "temurin"

[cache]
retention_days = 30
max_cache_size_gb = 10

[network]
timeout_seconds = 30
retries = 3
```
