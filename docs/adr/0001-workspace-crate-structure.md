# ADR 0001: Multi-Crate Cargo Workspace Structure

## Context
`djuke` is a CLI tool with multiple distinct responsibilities: network downloads, archive extraction, remote API communication, project configuration discovery, process execution, and diagnostics. A single monolithic crate leads to high compile times, coupled components, and prevents multiple subagents from working on isolated areas without git merge conflicts.

## Decision
Organize `djuke` as a multi-crate Cargo workspace partitioned into 10 focused crates with clear dependency boundaries.

## Consequences
- High parallelizability for subagent work.
- Strict isolation of concerns (e.g. `djuke-archive` has no network dependencies).
- Fast incremental compilation.
