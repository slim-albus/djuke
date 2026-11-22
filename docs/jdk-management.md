# JDK Management Specification

## 1. Remote Catalog: Foojay Disco API v3

`djuke` integrates with the official Foojay Disco API v3 (`https://api.foojay.io/disco/v3.0`).

### Query Endpoints
- **Packages Query**:
  `GET /packages?version={version}&distro={distro}&operating_system={os}&architecture={arch}&archive_type={ext}&package_type=jdk&release_status=ga`
- **Distributions Query**:
  `GET /distributions`
- **Major Versions Query**:
  `GET /major_versions`

### Supported Distributions
- `temurin` (Eclipse Temurin - default)
- `corretto` (Amazon Corretto)
- `zulu` (Azul Zulu)
- `microsoft` (Microsoft Build of OpenJDK)
- `semeru` (IBM Semeru OpenJ9)
- `graalvm_ce` (GraalVM Community)

---

## 2. Download, Integrity, and Storage

### Download Engine
- Implemented with `reqwest` streaming body into a local temporary file (`~/.djuke/tmp/`).
- Computes SHA-256 hash in a single pass while streaming bytes.
- Validates computed hash against Disco API package metadata checksum.

### Safe Extraction
- Supports `.tar.gz` and `.zip`.
- Strict path-traversal prevention: canonicalizes target file path and rejects entries starting with `..` or escaping the extraction root.
- Preserves Unix permission bits (`0o755` for `bin/java`, etc.).
- Performs atomic directory rename from temporary staging path to `~/.djuke/jdks/{distro}-{version}/`.

### Metadata Manifest (`.djuke-jdk.json`)
Each installed JDK directory contains `.djuke-jdk.json`:
```json
{
  "id": "temurin-21.0.4",
  "distribution": "temurin",
  "version": "21.0.4",
  "major_version": 21,
  "os": "windows",
  "arch": "x64",
  "java_home": "C:\\Users\\User\\.djuke\\jdks\\temurin-21.0.4",
  "installed_at": "2026-11-22T16:00:00Z"
}
```
