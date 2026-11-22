# djuke Documentation Index

Welcome to the internal engineering documentation for **djuke**.

## Table of Contents

1. [Architecture & System Blueprint](architecture.md)
   - Component boundaries, dataflow diagrams, process execution model, error handling strategy.
2. [CLI Reference](cli-reference.md)
   - Detailed specification of commands, flags, arguments, exit codes, and JSON outputs.
3. [Configuration & Manifest Discovery](configuration.md)
   - Precedence order (CLI > Env > Manifest > Global > Fallback), marker detection, and `djuke.toml` schema.
4. [JDK Management](jdk-management.md)
   - Foojay Disco API v3 client, version resolution algorithms, checksum validation, archive extraction, and local storage layout.
5. [Build Tool Orchestration](build-orchestration.md)
   - Wrapper-first delegation for Gradle (`gradlew`, `gradlew.bat`) and Maven (`mvnw`, `mvnw.cmd`), task mapping, and wrapper bootstrapping.
6. [Developer & Testing Guide](development.md)
   - Code conventions, workspace navigation, testing patterns, and subagent collaboration rules.
7. [Architecture Decision Records (ADR)](adr/0001-workspace-crate-structure.md)
   - Recorded architectural decisions, trade-offs, and design rationale.
