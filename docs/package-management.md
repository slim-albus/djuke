# Package Management & Central Search

`djuke` bridges the gap between Maven/Gradle build definitions and an npm-like CLI experience, allowing developers to search, inspect, add, and remove dependencies without manually hand-editing XML or Gradle files.

## Maven Central Solr API Search

`djuke search <query>` queries the official Maven Central Solr index (`https://search.maven.org/solrsearch/select`).
- Automatically formats and tabulates GroupId:ArtifactId, latest stable version, and packaging.
- Supports query filtering, wildcards, and `--limit`.

## Adding Dependencies (`djuke add`)

```bash
# Form 1: Automatically resolve latest version from Maven Central
djuke add org.postgresql:postgresql

# Form 2: Pinned version with custom scope
djuke add org.junit.jupiter:junit-jupiter:5.10.2 --scope test
```

### Scope Mapping Matrix

| `djuke --scope` | Gradle (`build.gradle[.kts]`) | Maven (`pom.xml`) |
|:---|:---|:---|
| `implementation` (default) | `implementation` | *(omitted / compile)* |
| `compile-only` | `compileOnly` | `<scope>provided</scope>` |
| `runtime` | `runtimeOnly` | `<scope>runtime</scope>` |
| `test` | `testImplementation` | `<scope>test</scope>` |

## Removing Dependencies (`djuke remove`)

`djuke remove <query>` searches the project's build file (`pom.xml` or `build.gradle[.kts]`) and strips the matching dependency declaration cleanly while preserving surrounding formatting and other dependencies.
