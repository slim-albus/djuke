# djuke Architecture & System Blueprint

## 1. High-Level Vision

`djuke` is designed to be the single entrypoint for Java application development. Rather than burdening developers with manual JDK installation, environment variable configuration (`JAVA_HOME`, `PATH`), and build tool synchronization, `djuke` orchestrates these dependencies ephemerally and reliably.

## 2. Core Architectural Principles

1. **Zero Global Pollution**: `djuke` never mutates system-wide registry entries (Windows) or global shell profiles without explicit opt-in. All environment injections (`JAVA_HOME`, `PATH`) are strictly scoped to child process executions or directory-local shell exports.
2. **Wrapper-First Execution**: If a project provides `./gradlew` or `./mvnw`, `djuke` invokes that wrapper directly. It does not re-download Gradle or Maven runtime distributions when wrappers exist.
3. **Decoupled Multi-Crate Boundaries**: To prevent monolithic entanglement and allow parallel subagent engineering, the workspace is partitioned into specialized domain crates.
4. **Resilient Network & I/O**: Checksums (SHA-256) are always verified before archive extraction. Extraction operations are atomic and protected against zip-slip path traversal attacks.

## 3. Data Flow Diagram

```mermaid
sequenceDiagram
    autonumber
    actor Dev as Developer / CI
    participant CLI as djuke (CLI Entrypoint)
    participant Cfg as djuke-config
    participant JDK as djuke-jdk
    participant Build as djuke-build
    participant Proc as djuke-process
    participant Target as Subprocess (gradlew / java)

    Dev->>CLI: djuke build
    CLI->>Cfg: Detect project root & config
    Cfg-->>CLI: ProjectConfig (Java 21, Gradle wrapper present)
    CLI->>JDK: Ensure JDK 21 installed
    alt JDK Missing
        JDK->>JDK: Query Foojay Disco API
        JDK->>JDK: Download & verify SHA-256
        JDK->>JDK: Extract to ~/.djuke/jdks/temurin-21.x
    end
    JDK-->>CLI: Resolved JDK Home path
    CLI->>Build: Resolve build command for "build"
    Build-->>CLI: ResolvedCommand (./gradlew build)
    CLI->>Proc: Spawn with JAVA_HOME=... and PATH=$JAVA_HOME/bin:$PATH
    Proc->>Target: Execute ./gradlew build
    Target-->>Dev: Streaming output & exit code
```

## 4. Crate Dependency Graph

```mermaid
graph TD
    djuke["crates/djuke (Binary)"] --> djuke-core
    djuke --> djuke-config
    djuke --> djuke-platform
    djuke --> djuke-jdk
    djuke --> djuke-build
    djuke --> djuke-process
    djuke --> djuke-doctor

    djuke-jdk --> djuke-core
    djuke-jdk --> djuke-platform
    djuke-jdk --> djuke-download
    djuke-jdk --> djuke-archive

    djuke-build --> djuke-core
    djuke-build --> djuke-platform
    djuke-build --> djuke-process
    djuke-build --> djuke-download
    djuke-build --> djuke-archive

    djuke-config --> djuke-core
    djuke-config --> djuke-platform

    djuke-doctor --> djuke-core
    djuke-doctor --> djuke-platform
    djuke-doctor --> djuke-jdk
    djuke-doctor --> djuke-build

    djuke-download --> djuke-core
    djuke-archive --> djuke-core
    djuke-process --> djuke-core
    djuke-platform --> djuke-core
```
