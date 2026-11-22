# djuke CLI Commands Specification & Situational Requirements

This document is the authoritative specification for all commands in the `djuke` toolchain. It defines the exact behaviors, preconditions, inputs, edge cases, exit codes, and guarantees across every operational situation.

---

## 1. Core Architectural Invariants

Every command implementation in `djuke` MUST strictly obey the following four invariants:

1. **Zero-Distribution-Download Invariant**:
   - `djuke` MUST NEVER download a 150MB+ standalone Gradle or Maven distribution zip archive when running project tasks (`run`, `build`, `test`, `dev`, `clean`, `setup`, `gradle`, `maven`).
   - If a project lacks a build tool wrapper (`gradlew` or `mvnw`), `djuke` MUST automatically scaffold the official, lightweight wrapper instantly (<5ms) in-place from embedded pre-compiled assets.
   - Full toolchain downloads (`~/.djuke/tools/`) are strictly deprecated in favor of embedded wrapper bootstrapping.

2. **Deterministic Isolation Invariant**:
   - `djuke` NEVER mutates the host machine's system-wide `PATH`, Windows Registry, or `/etc/environment` unless the user explicitly requests global changes with `--global`.
   - Tool execution is isolated via child process environment injection (`JAVA_HOME` pointing to `~/.djuke/jdks/<spec>`, with `bin/` prepended to `PATH`).

3. **JDK Resolution Hierarchy Invariant**:
   When resolving which Java runtime to activate, `djuke` strictly evaluates the following order of precedence:
   1. Explicit CLI Flags (`--java <v>`, `--distro <d>`)
   2. Project Manifest (`djuke.toml` `[java]` section)
   3. File-based version markers (`.java-version`)
   4. Legacy tool markers (`.sdkmanrc`)
   5. Build script compiler targets (`pom.xml` `<maven.compiler.target>`, `build.gradle.kts` `JavaLanguageVersion.of(...)`)
   6. Global user default (`~/.djuke/config.toml` `[java] default_version`)
   7. System `PATH` / system `JAVA_HOME` fallback

4. **High-Speed & Offline-Resilient Transfer Invariant**:
   - Downloads use 12 parallel chunk streams with 4MB sequential memory buffers writing into isolated `.part_{i}.tmp` files, completely avoiding lock contention and random disk seeks.
   - When running offline (`--offline`), commands must succeed if local assets exist and fail gracefully with actionable error messages if network is missing.

---

## 2. Command Requirements Matrix

### 2.1 `djuke doctor`
*Check development environment health and diagnostic status.*

- **Signature**: `djuke doctor [--json] [--quiet] [--verbose]`
- **Preconditions**: None. Can run anywhere.
- **Situational Behavior**:
  - **Situation A (No JDK installed)**:
    - Reports `Platform` (OS, Arch) as OK.
    - Reports `djuke Home` directory status.
    - Pings Foojay Disco API (`https://api.foojay.io/disco/v3.0/major_versions`).
    - Reports `Installed JDKs` as **FAIL** with hint `"Run 'djuke java install 21' or 'djuke java install latest' to install a JDK"`.
    - `overall_status` MUST be `false`. Exits with notice that environment is NOT ready for Java development.
  - **Situation B (One or more managed JDKs installed)**:
    - Reports installed JDK count, paths, and distribution details.
    - If in non-project directory: reports `"Running in non-project directory (global mode)"`.
    - `overall_status` is `true`.
  - **Situation C (Inside a valid project)**:
    - Inspects project build tool (`Gradle` or `Maven`), wrapper presence (`gradlew` or `mvnw`), and configured target Java version.
    - Checks whether the project's required JDK is already installed in `~/.djuke/jdks/`. If missing, reports **FAIL** for `Project Java Version` with hint `"Run 'djuke setup' to install <spec>"`.
  - **Situation D (Machine Output `--json`)**:
    - Emits structured `DoctorReport` JSON schema with `checks: [{ name, status, message, hint }]` and `overall_status: boolean`.
- **Exit Codes**:
  - `0`: All checks passed.
  - `0` (or `1` in strict CI mode): Report rendered with diagnostic failure warnings.

---

### 2.2 `djuke java list`
*List locally installed or remote available JDKs.*

- **Signature**: `djuke java list [--remote] [--lts] [-d|--distro <distro>] [--json]`
- **Situational Behavior**:
  - **Situation A (Local Listing - default)**:
    - Reads `~/.djuke/jdks/` directory.
    - If directory is empty, outputs helpful message: `"No JDKs installed in ~/.djuke/jdks. Run 'djuke java install 21' to install a recommended JDK."`
    - If JDKs exist, outputs list of all installed versions: `Distribution@Version (Major) -> /path/to/java_home`.
    - If `--json`, outputs array of `InstalledJdk` objects.
  - **Situation B (Remote Query `--remote`)**:
    - Queries Foojay Disco API for GA releases matching the selected distribution (defaults to `temurin` if not specified).
    - If `--lts` is passed, filters strictly to LTS releases (8, 11, 17, 21, 25).
    - If `-d <distro>` is passed, queries the specified distribution: `temurin`, `openjdk`, `corretto`, `zulu`, `microsoft`, `semeru`, `graalvm`, `liberica`, `oracle`, `sapmachine`.
    - If network is unreachable, returns descriptive error suggesting to check proxy or internet connection.
- **Exit Codes**: `0` on success, `1` on network/API failure.

---

### 2.3 `djuke java install`
*Install a specific JDK distribution and version.*

- **Signature**: `djuke java install [spec] [-d|--distro <distro>] [--quiet] [--json]`
- **Default Argument**: `spec` defaults to `"latest"` (or prompt/LTS if omitted).
- **Situational Behavior**:
  - **Situation A (Already Installed)**:
    - Checks local store `~/.djuke/jdks/`. If a matching distribution and version already exists, skips download and reports: `"JDK <distro>@<version> is already installed at <path>"`.
  - **Situation B (Version Specification Formats)**:
    - `djuke java install 21`: Installs Temurin (default) Java 21 LTS.
    - `djuke java install openjdk@25`: Installs OpenJDK Java 25.
    - `djuke java install corretto@latest`: Queries Disco for latest Corretto and installs.
    - `djuke java install 17 -d zulu`: Installs Azul Zulu Java 17.
  - **Situation C (Download & Verification)**:
    - Resolves exact package via Foojay Disco API for current OS (`windows`, `linux`, `macos`) and architecture (`x64`, `aarch64`).
    - Downloads package archive using 12 parallel range streams into `~/.djuke/cache/`.
    - Verifies checksum (SHA-256) if provided by provider.
    - Extracts archive to `~/.djuke/jdks/<id>/`.
    - Verifies binary existence (`bin/java` or `bin/java.exe`).
- **Exit Codes**: `0` on success, `1` on invalid spec or network error.

---

### 2.4 `djuke java use`
*Switch the active Java version locally or globally.*

- **Signature**: `djuke java use <spec> [-d|--distro <distro>] [--global]`
- **Situational Behavior**:
  - **Situation A (Local Project Directory - default)**:
    - If `djuke.toml` exists: updates `[java] version = "..."` and `distribution = "..."`.
    - If `djuke.toml` does not exist: creates or updates `djuke.toml` (and `.java-version`) in the current working directory.
    - If target JDK is not installed, automatically invokes `install_jdk` to download and set it up seamlessly.
  - **Situation B (Global Default `--global`)**:
    - Updates user global configuration `~/.djuke/config.toml` (`[java] default_version`, `default_distribution`).
    - Reports: `"✔ Set global default Java to <distro>@<version>"`.
- **Exit Codes**: `0` on success, `1` on error.

---

### 2.5 `djuke java current`
*Display the currently resolved Java runtime.*

- **Signature**: `djuke java current [--json]`
- **Situational Behavior**:
  - **Situation A (Configured & Installed)**:
    - Resolves configuration via project hierarchy.
    - Outputs: `Java runtime: <distro>@<version> -> /path/to/java_home`.
  - **Situation B (Configured but Not Installed)**:
    - Outputs: `Configured Java: <distro>@<version> (not installed locally). Run 'djuke setup' to install.`
  - **Situation C (`--json`)**:
    - Outputs JSON: `{ "spec": "...", "distribution": "...", "version": "...", "installed": true, "java_home": "..." }`.
- **Exit Codes**: `0`.

---

### 2.6 `djuke java uninstall`
*Uninstall a managed JDK.*

- **Signature**: `djuke java uninstall <target>`
- **Situational Behavior**:
  - Matches `<target>` against installed directory names or ID prefixes.
  - Deletes the JDK directory from `~/.djuke/jdks/`.
  - Reports: `"✔ Removed JDK at <path>"`.
  - If target not found: returns error listing available installed JDKs.
- **Exit Codes**: `0` on success, `1` if target not found.

---

### 2.7 `djuke new`
*Create a new Java project from a template in a new directory.*

- **Signature**: `djuke new [name] [-t|--template <type>] [-b|--build-tool <tool>] [--group <group>] [--java <version>] [-d|--distro <distro>] [-y|--yes]`
- **Situational Behavior**:
  - **Situation A (Interactive Terminal - no `-y`)**:
    - Prompts for Project Name (default: `my-app` or current directory name).
    - Prompts for Template Selection:
      1. CLI Application (minimal console app with Main class - Java 8 to 27+)
      2. Library (reusable Java library with JUnit 5 test suite - Java 8 to 27+)
      3. Spring Boot Web API (enterprise web application with Spring Boot 3.4 & Actuator - Java 17 to 25)
      4. Quarkus REST API (high-performance reactive microservice - Java 17 to 23)
      5. Micronaut Microservice (cloud-native modular app - Java 17 to 23)
    - Prompts for Build Tool:
      1. Gradle Kotlin DSL (`build.gradle.kts`) [Latest Gradle 9.9 - Recommended]
      2. Gradle Groovy DSL (`build.gradle`) [Latest Gradle 9.9 - Classic syntax]
      3. Maven (`pom.xml`) [Latest Maven 3.9 - Standard XML]
    - Prompts for Target Java Version (Highly Accessible Selection):
      1. 21 (LTS) - Recommended: Current production standard, maximum library compatibility
      2. 25 (LTS) - Modern standard: Next-generation long-term support
      3. 23 (GA)  - Recent feature release: Virtual threads & modern language features
      4. 17 (LTS) - Enterprise LTS: Broad compatibility for established projects
      5. 27 (EA)  - Next-Gen / Bleeding edge: Early access language features
      6. 11 (LTS) - Legacy enterprise: Older enterprise maintenance
      7. 8  (LTS) - Legacy baseline: Java 1.8 compatibility
      8. Custom version... (type any version, e.g. 24, 22, 26)
    - Prompts for Root Package / Group ID (default: `com.example.<project_name>`).
    - Prompts for JDK Distribution Flavour (Temurin [Default], OpenJDK, Corretto, Zulu, Microsoft, GraalVM, Liberica, Oracle, SapMachine, Semeru).
  - **Situation B (Non-interactive / CI `-y`)**:
    - Uses CLI arguments or defaults: name `my-app`, template `cli`, build tool `gradle-kts`, Java 21, Temurin.
  - **Situation C (Destination Directory Exists & Non-Empty)**:
    - Aborts immediately with error: `"Target directory '<dest>' already exists and is not empty"`. Never overwrites.
  - **Situation D (Dynamic Wrapper Resolution, Toolchain Priming & Zero-Wait Execution)**:
    - Dynamically queries official remote APIs (`services.gradle.org/versions/current` for Gradle, Maven Central metadata for Maven) at execution time to discover the true latest release.
    - Caches resolved tool versions in `~/.djuke/cache/tool_versions.json` (TTL: 24 hours); seamlessly falls back to embedded baseline constants (`9.9.2` / `3.9.16`) when offline or behind firewalls.
    - Dynamically resolves framework versions (Spring Boot, Quarkus, Micronaut) from Maven Central when online.
    - Dynamically resolves Java `latest` and `lts` aliases via Foojay Disco API.
    - Ensures target JDK is available and **immediately primes and pre-downloads the wrapper distribution** (`gradlew --version` / `mvnw -v`) during project creation.
    - When the developer runs `cd <project> && djuke run`, all toolchains and distributions are already verified and cached on disk—zero unexpected 150MB runtime downloads.
    - Automatically writes `gradle.properties` configuring parallel builds, caching, and `--enable-native-access=ALL-UNNAMED` for modern Java runtimes.
    - MUST write `djuke.toml` with project configuration.
- **Exit Codes**: `0` on success, `1` on validation or IO failure.

---

### 2.8 `djuke init`
*Initialize a new Java project in the current working directory.*

- **Signature**: `djuke init [name] [-t|--template <type>] [-b|--build-tool <tool>] [--group <group>] [--java <version>] [-d|--distro <distro>] [-y|--yes]`
- **Situational Behavior**:
  - Same prompts/options as `djuke new`, but defaults project name to the current directory's folder name.
  - Directly creates files in the current directory.
  - Does NOT fail if folder exists, but generates files safely.
  - Auto-bootstraps wrapper (`gradlew` or `mvnw`) in the current directory.
- **Exit Codes**: `0` on success, `1` on error.

---

### 2.9 `djuke add`
*Add a dependency to the current project (Maven or Gradle).*

- **Signature**: `djuke add <dependency> [--scope <scope>] [--test]`
- **Situational Behavior**:
  - **Situation A (Maven `pom.xml`)**:
    - Parses XML structure.
    - If version is omitted (e.g. `djuke add com.google.guava:guava` or `djuke add guava`), queries Maven Central Solr API, discovers latest release version, and adds `<dependency>` to `<dependencies>` block with proper indentation.
    - Sets `<scope>test</scope>` if `--test` or `--scope test` is provided.
  - **Situation B (Gradle Kotlin DSL `build.gradle.kts`)**:
    - Locates `dependencies { ... }` block.
    - Appends `implementation("group:artifact:version")` (or `testImplementation(...)`).
  - **Situation C (Gradle Groovy DSL `build.gradle`)**:
    - Appends `implementation 'group:artifact:version'` (or `testImplementation '...'`).
  - **Situation D (Outside a project)**:
    - Returns error: `"No build.gradle or pom.xml found in project root"`.
- **Exit Codes**: `0` on success, `1` on error.

---

### 2.10 `djuke remove`
*Remove a dependency from the current project.*

- **Signature**: `djuke remove <dependency>`
- **Situational Behavior**:
  - Accepts `groupId:artifactId` or shorthand `artifactId`.
  - In `pom.xml`: removes the matching `<dependency>...</dependency>` block.
  - In Gradle build scripts: removes the line declaring the matching dependency coordinate.
  - Reports: `"✔ Removed <group>:<artifact> from <file>"`.
- **Exit Codes**: `0` on success, `1` if not found or invalid format.

---

### 2.11 `djuke search`
*Search Maven Central for packages and libraries.*

- **Signature**: `djuke search <query> [--limit <N>]`
- **Situational Behavior**:
  - Sends query to `https://search.maven.org/solrsearch/select?q=...&rows=N&wt=json`.
  - Formats results as a tabular terminal view with columns: `ARTIFACT`, `LATEST`, `PACKAGING`.
  - If no packages match, prints `"No matching artifacts found."`.
- **Exit Codes**: `0` on success, `1` on network error.

---

### 2.12 `djuke cache`
*Manage downloaded archives and temporary cache files.*

- **Signature**: `djuke cache <stats|list|clean> [--json]`
- **Subcommands**:
  - `stats`: Calculates total file count and disk usage of `~/.djuke/cache/`.
  - `list`: Lists all cached `.zip`, `.tar.gz` archive files.
  - `clean`: Purges all cached download archives to reclaim disk space. Preserves installed JDKs in `~/.djuke/jdks/`.
- **Exit Codes**: `0`.

---

### 2.13 `djuke setup`
*Prepare project dependencies, JDK, and build tool wrappers.*

- **Signature**: `djuke setup [-t|--tool <tool>] [-d|--distro <distro>] [--quiet]`
- **Situational Behavior**:
  - **Step 1 (JDK Resolution & Installation)**:
    - Resolves required JDK from project configuration (`djuke.toml`, `.java-version`, or default).
    - If JDK is not installed, automatically downloads and installs it.
  - **Step 2 (Wrapper Bootstrapping)**:
    - Detects whether project is Gradle or Maven.
    - If wrapper (`gradlew` or `mvnw`) is missing, generates it immediately from embedded assets without downloading any 150MB distribution.
  - **Step 3 (Status Summary)**:
    - Prints confirmation of Java version, `JAVA_HOME`, build tool, and wrapper status.
- **Exit Codes**: `0` on success, `1` on failure.

---

### 2.14 `djuke build`
*Execute the project build task.*

- **Signature**: `djuke build [-t|--tool <tool>] [--offline] [-U|--refresh] [-P <prop>] [-D <def>] [--parallel] [-- <extra-args>]`
- **Situational Behavior**:
  - Resolves project configuration and ensures target JDK is installed.
  - Maps command to tool task:
    - Gradle: `gradlew build` (or `gradlew.bat build` on Windows).
    - Maven: `mvnw package` (or `mvnw.cmd package`).
  - **Missing Wrapper Guarantee**: If `gradlew` / `mvnw` does not exist in project root, auto-generates wrapper in <5ms from embedded assets. NEVER downloads distribution archive!
  - Passes translated flags:
    - `--offline`: `--offline` (Gradle) / `-o` (Maven).
    - `--refresh`: `--refresh-dependencies` (Gradle) / `-U` (Maven).
    - `--parallel`: `--parallel` (Gradle) / `-T 1C` (Maven).
    - `-P<prop>`: forwarded to tool.
    - `-D<def>`: forwarded to tool.
  - Injects `JAVA_HOME` and prepends `bin/` to `PATH`.
- **Exit Codes**: Propagates the build tool process exit status (0 on success, non-zero on compiler/build error).

---

### 2.15 `djuke test`
*Execute the project test task.*

- **Signature**: `djuke test [-t|--tool <tool>] [--offline] [-U|--refresh] [-P <prop>] [-D <def>] [--parallel] [-- <extra-args>]`
- **Situational Behavior**:
  - Maps to `gradlew test` or `mvnw test`.
  - Auto-scaffolds wrapper if missing.
  - Propagates test process exit status verbatim.
- **Exit Codes**: 0 if all tests pass; non-zero if any test fails.

---

### 2.16 `djuke run`
*Execute project application or single Java source file.*

- **Signature**: `djuke run [-t|--tool <tool>] [--offline] [-U|--refresh] [-P <prop>] [-D <def>] [-- <args>]`
- **Situational Behavior**:
  - **Situation A (Single-file JEP 330 source)**:
    - If the first argument ends with `.java` (e.g. `djuke run Hello.java`), directly executes via `java <file>` using the project's resolved JDK. No build tool or wrapper is required.
  - **Situation B (Project Application Execution)**:
    - In Gradle: executes `gradlew run` (or configured custom task in `djuke.toml`).
    - In Maven: executes `mvnw exec:java`, `mvnw spring-boot:run`, or configured task.
    - **Missing Wrapper Guarantee**: Auto-scaffolds wrapper immediately if missing. NEVER downloads Gradle or Maven distribution!
- **Exit Codes**: Propagates application process exit status.

---

### 2.17 `djuke dev`
*Start the project application in development/live-reload mode.*

- **Signature**: `djuke dev [-t|--tool <tool>] [-- <args>]`
- **Situational Behavior**:
  - In Gradle: executes `gradlew run --continuous` (or `quarkusDev`, `bootRun`).
  - In Maven: executes `mvnw quarkus:dev`, `mvnw spring-boot:run`, etc.
  - Auto-scaffolds wrapper if missing.
- **Exit Codes**: Keeps running until interrupted (Ctrl+C).

---

### 2.18 `djuke clean`
*Execute the project clean task.*

- **Signature**: `djuke clean [-t|--tool <tool>] [-- <args>]`
- **Situational Behavior**:
  - Executes `gradlew clean` or `mvnw clean`.
  - Auto-scaffolds wrapper if missing.
- **Exit Codes**: Propagates tool process exit status.

---

### 2.19 `djuke tree`
*Display a visual tree of project dependencies.*

- **Signature**: `djuke tree`
- **Situational Behavior**:
  - Parses project dependencies from `pom.xml`, `build.gradle.kts`, or `build.gradle`.
  - Renders a clean visual ASCII dependency tree in the terminal.
  - If no dependencies exist, informs user cleanly.
- **Exit Codes**: `0`.

---

### 2.20 `djuke outdated`
*Check for outdated dependencies against Maven Central.*

- **Signature**: `djuke outdated`
- **Situational Behavior**:
  - Extracts all declared dependencies and their current versions.
  - Queries Maven Central Solr API for the latest release versions.
  - Prints a formatted comparison table:
    - `DEPENDENCY`: `groupId:artifactId`
    - `CURRENT`: Current version in project
    - `LATEST`: Latest version on Maven Central
    - `STATUS`: `✔ Up-to-date` or `⚠ Outdated`
- **Exit Codes**: `0`.

---

### 2.21 `djuke audit`
*Audit dependencies for known security vulnerabilities via OSV.dev.*

- **Signature**: `djuke audit`
- **Situational Behavior**:
  - Extracts all dependencies.
  - Sends batch queries to OSV.dev (`https://api.osv.dev/v1/querybatch`) for Maven ecosystem vulnerabilities.
  - If vulnerabilities are discovered, prints detailed vulnerability report with CVE ID, affected version ranges, summary, and database URL.
- **Exit Codes**:
  - `0`: No vulnerabilities detected.
  - `1`: One or more security vulnerabilities detected.

---

### 2.22 `djuke exec`
*Execute an arbitrary command with project Java environment injected.*

- **Signature**: `djuke exec <command> [args...]`
- **Situational Behavior**:
  - Resolves project's JDK and injects `JAVA_HOME` and `PATH` into the child process environment.
  - Executes `<command>` (e.g. `djuke exec javac -version`, `djuke exec jshell`, `djuke exec node script.js`).
  - Passes stdio directly through to the terminal.
- **Exit Codes**: Propagates command exit code.

---

### 2.23 `djuke env`
*Generate shell export commands for PATH and JAVA_HOME.*

- **Signature**: `djuke env [shell]`
- **Supported Shells**: `bash`, `zsh`, `fish`, `powershell`.
- **Situational Behavior**:
  - Outputs export commands (e.g. `export JAVA_HOME="..."`, `$env:JAVA_HOME = "..."`).
  - Can be evaluated in shell profile: `eval "$(djuke env)"`.
- **Exit Codes**: `0`.

---

### 2.24 `djuke completions`
*Generate shell autocomplete scripts.*

- **Signature**: `djuke completions <shell>`
- **Supported Shells**: `bash`, `zsh`, `fish`, `powershell`, `elvish`.
- **Exit Codes**: `0`.

---

### 2.25 `djuke gradle`
*Gradle build tool commands and direct execution.*

- **Signature**: `djuke gradle [subcommand|args]`
- **Situational Behavior**:
  - `djuke gradle wrapper [--version <v>]`: Creates Gradle wrapper directly in project.
  - `djuke gradle stop`: Stops background Gradle daemons via `--stop`.
  - `djuke gradle status`: Shows running Gradle daemons via `--status`.
  - `djuke gradle <any-tasks>`: Forwards tasks directly to project wrapper. If wrapper is missing, auto-creates it first.
- **Exit Codes**: Propagates Gradle exit code.

---

### 2.26 `djuke maven`
*Maven build tool commands and direct execution.*

- **Signature**: `djuke maven [subcommand|args]`
- **Situational Behavior**:
  - `djuke maven wrapper [--version <v>]`: Creates Maven wrapper directly in project.
  - `djuke maven <any-goals>`: Forwards goals directly to project wrapper. If wrapper is missing, auto-creates it first.
- **Exit Codes**: Propagates Maven exit code.

---

### 2.27 `djuke upgrade`
*Self-upgrade the djuke binary directly from GitHub releases.*

- **Signature**: `djuke upgrade [--check] [-f|--force]`
- **Situational Behavior**:
  - **Situation A (Check only `--check`)**:
    - Queries GitHub Releases API for `slim-albus/djuke`.
    - Compares current semver against latest release tag.
    - Reports whether an update is available without modifying local files.
  - **Situation B (Upgrade available)**:
    - Finds the prebuilt binary or archive matching current OS and architecture.
    - Downloads archive, extracts binary, and atomically replaces the currently executing `djuke` executable.
    - Reports: `"✔ Successfully upgraded djuke to vX.Y.Z!"`.
  - **Situation C (Already up to date)**:
    - Reports `"✔ djuke is already up to date!"` unless `--force` is specified.
- **Exit Codes**: `0` on success, `1` on network or IO error.

---

### 2.28 External Subcommand Passthrough
*Fallback for arbitrary tool tasks.*

- **Signature**: `djuke <task-name> [args...]`
- **Situational Behavior**:
  - When an unrecognized subcommand is provided (e.g. `djuke spotbugs` or `djuke flywayMigrate`), `djuke` looks up custom tasks in `djuke.toml`.
  - If defined in `[tasks]`, runs the mapped command.
  - Otherwise, forwards `<task-name>` directly to the active build tool wrapper.
- **Exit Codes**: Propagates process exit code.

---

## 3. Enforcement and Verification Plan

| Requirement Area | Enforcement Mechanism | Verification Test |
| :--- | :--- | :--- |
| **No Distribution Download** | `crates/djuke-build/src/executor.rs`: removed `ensure_build_tool_installed` fallback; auto-calls `create_gradle_wrapper` / `create_maven_wrapper` | `test_gradle_and_maven_wrapper_auto_generation_in_project_without_wrapper` |
| **Doctor Failure on Missing JDK** | `crates/djuke-doctor/src/checks.rs`: `overall_status = checks.iter().all(\|c\| c.status)` | `test_doctor_reports_unhealthy_when_no_jdks_installed` |
| **All 10 JDK Distributions** | `crates/djuke-core/src/model.rs`: `JdkDistribution` enum support for Temurin, OpenJdk, Corretto, Zulu, Microsoft, Semeru, Graalvm, Liberica, Oracle, Sapmachine | `test_jdk_distribution_support_all_10_distros` |
| **Template Custom Flavours** | `crates/djuke-templates/src/lib.rs`: supports distro & version parameters in `djuke.toml` and build scripts | `test_template_generation_with_custom_distro_and_latest_java` |
| **Offline Wrapper Bootstrapping** | `crates/djuke-build/src/bootstrap.rs`: embedded `gradlew`, `gradlew.bat`, `gradle-wrapper.jar`, `mvnw`, `mvnw.cmd`, `maven-wrapper.jar` | `test_create_gradle_wrapper_creates_all_files_offline`, `test_create_maven_wrapper_creates_all_files_offline` |
| **High-Speed Chunk Downloader** | `crates/djuke-download/src/parallel.rs`: 12 range streams, 4MB in-memory chunk buffers, zero lock contention | `crates/djuke-download` tests |
| **Dependency Management** | `crates/djuke-pm`: Solr search, XML / Kotlin AST manipulation, OSV.dev vulnerability auditing | `crates/djuke-pm` tests |
