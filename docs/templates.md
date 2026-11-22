# Project Templates & Scaffolding

`djuke new` provides project bootstrapping across modern Java frameworks, build tools, and language conventions.

## Supported Project Templates

1. **`cli`**: Minimal command-line interface application with standard entrypoint `Main.java`.
2. **`lib`**: Reusable library with sensible defaults for packaging and modular testing.
3. **`spring-boot`**: Spring Boot application pre-configured with `spring-boot-starter-web` and `spring-boot-starter-test`.
4. **`quarkus`**: Quarkus reactive application configured with RESTEasy Reactive and Quarkus JUnit 5 support.
5. **`micronaut`**: Micronaut framework application with Netty runtime, inject-java annotations, and JUnit 5 test runner.

## Supported Build Tools

- **`gradle-kts`**: Gradle Kotlin DSL (`build.gradle.kts`, `settings.gradle.kts`).
- **`gradle-groovy`**: Gradle Groovy DSL (`build.gradle`, `settings.gradle`).
- **`maven`**: Apache Maven (`pom.xml`).

## Generated Structure

Every bootstrapped project includes:
- Build configuration file (`build.gradle.kts` / `build.gradle` / `pom.xml`)
- Project manifest (`djuke.toml`) pinning the chosen Java version and build tool
- `.gitignore` tailored for Java, Gradle, Maven, and IDE artifacts
- Application skeleton under `src/main/java/<package_path>/`
- Test skeleton under `src/test/java/<package_path>/`
- Pre-bootstrapped wrapper executable (`gradlew` / `mvnw`)
