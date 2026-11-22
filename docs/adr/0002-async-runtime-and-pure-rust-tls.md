# ADR 0002: Async Runtime and Pure-Rust TLS

## Context
JDK downloads involve streaming multi-hundred megabyte archives with real-time terminal progress indicators and checksum hashing. In addition, `djuke` needs to query remote APIs concurrently without blocking user interactions.

## Decision
Use `tokio` as the asynchronous runtime and `reqwest` with pure-Rust `rustls-tls` instead of native OpenSSL.

## Consequences
- Eliminates Windows and Linux dependencies on system OpenSSL libraries.
- Smooth terminal progress bar integration via `indicatif`.
- Highly concurrent operations for remote catalog querying.
