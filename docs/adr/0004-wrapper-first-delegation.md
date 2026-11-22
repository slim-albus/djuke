# ADR 0004: Wrapper-First Build Tool Delegation

## Context
Java projects using Gradle and Maven almost universally provide self-bootstrapping wrapper scripts (`gradlew`, `mvnw`). If a developer tool re-downloads full Gradle/Maven distributions and bypasses wrappers, it risks version discrepancies, daemon mismatches, and plugin incompatibility.

## Decision
Adopt a "wrapper-first" execution policy:
1. When running `djuke build`, `test`, `run`, or `clean`, inspect the project for `./gradlew` or `./mvnw` and invoke it directly.
2. Only download standalone build tools if the wrapper is absent and the developer explicitly requests wrapper creation or setup.

## Consequences
- Preserves project-specific Gradle/Maven daemon and wrapper caching.
- Instant execution with zero overhead.
- Friendly, npm-like task normalizer experience for developers.
