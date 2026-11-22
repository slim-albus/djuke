# ADR 0003: Foojay Disco API for JDK Catalog Discovery

## Context
Java has numerous distributions (Temurin, Corretto, Zulu, Microsoft OpenJDK, IBM Semeru, GraalVM). Implementing custom scrapers or vendor-specific APIs for each distribution increases maintenance cost and failure modes.

## Decision
Use the community-standard Foojay Disco API v3 (`https://api.foojay.io/disco/v3.0`) as the primary discovery source for JDK releases, package URLs, architecture mappings, and SHA-256 checksums.

## Consequences
- Out-of-the-box support for all major OpenJDK distributions.
- Official direct package download links and precomputed checksums.
- Clean uniform query interface across operating systems and architectures.
