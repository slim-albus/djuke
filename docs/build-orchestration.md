# Build Tool Orchestration Specification

## 1. Wrapper-First Architecture

Java projects standardly use self-bootstrapping wrappers (`gradlew` / `mvnw`). `djuke` honors this standard completely:

1. **Gradle**:
   - Unix: `./gradlew`
   - Windows: `.\gradlew.bat`
2. **Maven**:
   - Unix: `./mvnw`
   - Windows: `.\mvnw.cmd`

`djuke` acts as an npm-style command normalizer. When a developer types `djuke test`, `djuke` maps the invocation to the correct wrapper script, passing through any extra flags seamlessly:

```bash
djuke test -- -Dtest=MyTest
```

---

## 2. Process Environment Injection

When invoking the build tool or executing `djuke exec <command>`, `djuke-process` prepares a dedicated child process environment:

- `JAVA_HOME`: Absolute path to resolved JDK root.
- `PATH`: `$JAVA_HOME/bin` prepended to the existing system `PATH`.
- `DJUKE_PROJECT_ROOT`: Absolute path to project root.
- `DJUKE_JAVA_VERSION`: Resolved Java version string.
- `DJUKE_BUILD_TOOL`: `gradle` or `maven`.

---

## 3. Wrapper Bootstrapping

If a repository lacks a wrapper and the developer runs `djuke setup` or `djuke gradle wrapper`:
1. `djuke` checks `~/.djuke/tools/{gradle|maven}/{version}/`.
2. If missing, it downloads the official distribution archive and extracts it.
3. It executes the official wrapper generation task (`gradle wrapper` or `mvn wrapper:wrapper`) inside the project folder.
