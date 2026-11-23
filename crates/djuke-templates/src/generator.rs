use std::fs;
use std::path::Path;

use djuke_core::Result;

use crate::model::{BuildToolChoice, NewProjectOptions, TemplateType, TestFramework};

pub async fn generate_project(opts: &NewProjectOptions) -> Result<()> {
    let dest = &opts.destination;
    fs::create_dir_all(dest)?;

    // 1. If Spring Boot template and initializr is enabled, try live Spring Initializr first
    if opts.template == TemplateType::SpringBoot && !opts.skip_initializr {
        let initializr = crate::initializr::SpringInitializrClient::new();
        match initializr.generate_project(dest, opts).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                eprintln!(
                    "warning: Spring Initializr API unreachable ({}). Falling back to offline embedded Spring Boot template...",
                    e
                );
            }
        }
    }

    // 2. Special case: Multi-module enterprise architecture
    if opts.template == TemplateType::MultiModule {
        return generate_multi_module_project(dest, opts);
    }

    let pkg_path = opts.group_id.replace('.', "/");
    let main_src = dest.join("src").join("main").join("java").join(&pkg_path);
    let resources = dest.join("src").join("main").join("resources");

    fs::create_dir_all(&main_src)?;
    fs::create_dir_all(&resources)?;

    // Generate source files based on template
    match opts.template {
        TemplateType::Cli => {
            generate_cli_source(&main_src, dest, opts)?;
        }
        TemplateType::Lib => {
            generate_lib_source(&main_src, dest, opts)?;
        }
        TemplateType::SpringBoot => {
            generate_spring_source(&main_src, dest, opts)?;
        }
        TemplateType::Quarkus => {
            generate_quarkus_source(&main_src, dest, opts)?;
        }
        TemplateType::Micronaut => {
            generate_micronaut_source(&main_src, dest, opts)?;
        }
        TemplateType::Javalin => {
            generate_javalin_source(&main_src, dest, opts)?;
        }
        TemplateType::Grpc => {
            generate_grpc_source(&main_src, dest, opts)?;
        }
        TemplateType::Picocli => {
            generate_picocli_source(&main_src, dest, opts)?;
        }
        TemplateType::MultiModule => unreachable!(),
    }

    // Generate build files based on build tool
    match opts.build_tool {
        BuildToolChoice::GradleKts => {
            generate_gradle_kts(dest, opts)?;
        }
        BuildToolChoice::GradleGroovy => {
            generate_gradle_groovy(dest, opts)?;
        }
        BuildToolChoice::Maven => {
            generate_pom_xml(dest, opts)?;
        }
    }

    // Generate standard enterprise configurations
    generate_standard_configs(dest, opts)?;

    // Generate logback.xml for logging
    let logback_xml = r#"<configuration>
    <appender name="CONSOLE" class="ch.qos.logback.core.ConsoleAppender">
        <encoder>
            <pattern>%d{HH:mm:ss.SSS} [%thread] %highlight(%-5level) %cyan(%logger{36}) - %msg%n</pattern>
        </encoder>
    </appender>
    <root level="INFO">
        <appender-ref ref="CONSOLE" />
    </root>
</configuration>
"#;
    let _ = fs::write(resources.join("logback.xml"), logback_xml);

    // Generate djuke.toml
    let tool_name = match opts.build_tool {
        BuildToolChoice::GradleKts | BuildToolChoice::GradleGroovy => "gradle",
        BuildToolChoice::Maven => "maven",
    };

    let distro = opts
        .distribution
        .unwrap_or_else(djuke_core::JdkDistribution::default_distro);

    let djuke_toml = format!(
        r#"[project]
name = "{}"

[java]
version = "{}"
distribution = "{}"

[build]
tool = "{}"
wrapper = true
"#,
        opts.name, opts.java_version, distro, tool_name
    );
    fs::write(dest.join("djuke.toml"), djuke_toml)?;

    // Generate README.md
    let readme = format!(
        r#"# {}

Created with **djuke** ({}) using {}.

## Quick Start with djuke

```bash
# Setup environment (downloads JDK and generates wrapper if needed)
djuke setup

# Build project
djuke build

# Run tests
djuke test

# Run application
djuke run
```
"#,
        opts.name, opts.template, opts.build_tool
    );
    fs::write(dest.join("README.md"), readme)?;

    Ok(())
}

fn generate_standard_configs(dest: &Path, _opts: &NewProjectOptions) -> Result<()> {
    let gitignore = r#"# Build outputs
/build/
/target/
/.gradle/
/.mvn/timing/
/out/

# IDE files
/.idea/
/*.iml
/.vscode/
/.settings/
/.project/
/.classpath/
*.ipr
*.iws

# Logs & diagnostics
*.log
/logs/
hs_err_pid*
replay_pid*

# OS metadata
.DS_Store
Thumbs.db
"#;
    fs::write(dest.join(".gitignore"), gitignore)?;

    let editorconfig = r#"root = true

[*]
charset = utf-8
end_of_line = lf
indent_style = space
indent_size = 4
insert_final_newline = true
trim_trailing_whitespace = true

[*.{yml,yaml,json}]
indent_size = 2

[*.md]
trim_trailing_whitespace = false

[*.{bat,cmd}]
end_of_line = crlf
"#;
    fs::write(dest.join(".editorconfig"), editorconfig)?;

    let gitattributes = r#"# Auto detect text files and perform LF normalization
* text=auto eol=lf

# Java and build scripts
*.java text diff=java eol=lf
*.kt text eol=lf
*.kts text eol=lf
*.gradle text eol=lf
*.xml text eol=lf
*.properties text eol=lf
*.md text eol=lf

# Wrappers
gradlew text eol=lf
mvnw text eol=lf

# Windows scripts
*.bat text eol=crlf
*.cmd text eol=crlf
"#;
    fs::write(dest.join(".gitattributes"), gitattributes)?;

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum TestKind {
    Cli,
    Lib,
    SpringBoot,
    Quarkus,
    Micronaut,
    Javalin,
    Grpc,
    Picocli,
    MultiModuleCore,
    MultiModuleApi,
}

fn generate_test_for_template(dest: &Path, opts: &NewProjectOptions, kind: TestKind) -> Result<()> {
    if opts.test_framework == TestFramework::None {
        return Ok(());
    }

    let pkg_path = opts.group_id.replace('.', "/");

    if opts.test_framework == TestFramework::Spock {
        let test_groovy = match kind {
            TestKind::MultiModuleCore => dest
                .join("core")
                .join("src")
                .join("test")
                .join("groovy")
                .join(&pkg_path)
                .join("core"),
            TestKind::MultiModuleApi => dest
                .join("api")
                .join("src")
                .join("test")
                .join("groovy")
                .join(&pkg_path)
                .join("api"),
            _ => dest.join("src").join("test").join("groovy").join(&pkg_path),
        };
        fs::create_dir_all(&test_groovy)?;

        let (spec_name, content) = match kind {
            TestKind::Cli => (
                "AppSpec.groovy",
                format!(
                    r#"package {}

import spock.lang.Specification

class AppSpec extends Specification {{
    def "greeting returns expected message"() {{
        given:
        def app = new App()

        expect:
        app.getGreeting() == "Hello World!"
    }}
}}
"#,
                    opts.group_id
                ),
            ),
            TestKind::Lib => (
                "LibrarySpec.groovy",
                format!(
                    r#"package {}

import spock.lang.Specification

class LibrarySpec extends Specification {{
    def "library method works"() {{
        given:
        def lib = new Library()

        expect:
        lib.someLibraryMethod() == true
    }}
}}
"#,
                    opts.group_id
                ),
            ),
            TestKind::SpringBoot => (
                "ApplicationSpec.groovy",
                format!(
                    r#"package {}

import org.springframework.boot.test.context.SpringBootTest
import spock.lang.Specification

@SpringBootTest
class ApplicationSpec extends Specification {{
    def "context loads"() {{
        expect:
        true
    }}
}}
"#,
                    opts.group_id
                ),
            ),
            TestKind::Quarkus => (
                "GreetingResourceSpec.groovy",
                format!(
                    r#"package {}

import io.quarkus.test.junit.QuarkusTest
import spock.lang.Specification

@QuarkusTest
class GreetingResourceSpec extends Specification {{
    def "hello endpoint test"() {{
        expect:
        true
    }}
}}
"#,
                    opts.group_id
                ),
            ),
            TestKind::Micronaut => (
                "ApplicationSpec.groovy",
                format!(
                    r#"package {}

import io.micronaut.runtime.EmbeddedApplication
import io.micronaut.test.extensions.junit5.annotation.MicronautTest
import jakarta.inject.Inject
import spock.lang.Specification

@MicronautTest
class ApplicationSpec extends Specification {{
    @Inject
    EmbeddedApplication<?> application

    def "application is running"() {{
        expect:
        application.isRunning()
    }}
}}
"#,
                    opts.group_id
                ),
            ),
            TestKind::Javalin => (
                "AppSpec.groovy",
                format!(
                    r#"package {}

import spock.lang.Specification

class AppSpec extends Specification {{
    def "greeting returns expected message"() {{
        given:
        def app = new App()

        expect:
        app.getGreeting() == "Hello from Javalin!"
    }}
}}
"#,
                    opts.group_id
                ),
            ),
            TestKind::Grpc => (
                "AppSpec.groovy",
                format!(
                    r#"package {}

import spock.lang.Specification

class AppSpec extends Specification {{
    def "format greeting works"() {{
        given:
        def app = new App()

        expect:
        app.formatGreeting("Alice") == "Hello, Alice! Welcome to gRPC."
    }}
}}
"#,
                    opts.group_id
                ),
            ),
            TestKind::Picocli => (
                "AppSpec.groovy",
                format!(
                    r#"package {}

import picocli.CommandLine
import spock.lang.Specification

class AppSpec extends Specification {{
    def "help flag executes cleanly"() {{
        given:
        def app = new App()
        def cmd = new CommandLine(app)

        expect:
        cmd.execute("--help") == 0
    }}
}}
"#,
                    opts.group_id
                ),
            ),
            TestKind::MultiModuleCore => (
                "DomainServiceSpec.groovy",
                format!(
                    r#"package {}.core

import spock.lang.Specification

class DomainServiceSpec extends Specification {{
    def "process input works"() {{
        given:
        def service = new DomainService()

        expect:
        service.process("test") == "Processed: test"
    }}
}}
"#,
                    opts.group_id
                ),
            ),
            TestKind::MultiModuleApi => (
                "AppSpec.groovy",
                format!(
                    r#"package {}.api

import spock.lang.Specification

class AppSpec extends Specification {{
    def "app sanity test"() {{
        expect:
        true
    }}
}}
"#,
                    opts.group_id
                ),
            ),
        };

        fs::write(test_groovy.join(spec_name), content)?;
        return Ok(());
    }

    let test_java_dir = match kind {
        TestKind::MultiModuleCore => dest
            .join("core")
            .join("src")
            .join("test")
            .join("java")
            .join(&pkg_path)
            .join("core"),
        TestKind::MultiModuleApi => dest
            .join("api")
            .join("src")
            .join("test")
            .join("java")
            .join(&pkg_path)
            .join("api"),
        _ => dest.join("src").join("test").join("java").join(&pkg_path),
    };
    fs::create_dir_all(&test_java_dir)?;

    match opts.test_framework {
        TestFramework::None | TestFramework::Spock => unreachable!(),
        TestFramework::JUnit5 => {
            let (file_name, content) = match kind {
                TestKind::Cli => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class AppTest {{
    @Test
    void testAppSanity() {{
        App app = new App();
        assertEquals("Hello World!", app.getGreeting());
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Lib => (
                    "LibraryTest.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class LibraryTest {{
    @Test
    void testSomeLibraryMethod() {{
        Library classUnderTest = new Library();
        assertTrue(classUnderTest.someLibraryMethod(), "someLibraryMethod should return 'true'");
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::SpringBoot => (
                    "ApplicationTests.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import org.springframework.boot.test.context.SpringBootTest;

@SpringBootTest
class ApplicationTests {{
    @Test
    void contextLoads() {{
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Quarkus => (
                    "GreetingResourceTest.java",
                    format!(
                        r#"package {};

import io.quarkus.test.junit.QuarkusTest;
import org.junit.jupiter.api.Test;
import static io.restassured.RestAssured.given;
import static org.hamcrest.CoreMatchers.is;

@QuarkusTest
class GreetingResourceTest {{
    @Test
    void testHelloEndpoint() {{
        given()
          .when().get("/hello")
          .then()
             .statusCode(200)
             .body(is("Hello from Quarkus + djuke"));
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Micronaut => (
                    "ApplicationTest.java",
                    format!(
                        r#"package {};

import io.micronaut.test.extensions.junit5.annotation.MicronautTest;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Assertions;
import jakarta.inject.Inject;
import io.micronaut.runtime.EmbeddedApplication;

@MicronautTest
class ApplicationTest {{
    @Inject
    EmbeddedApplication<?> application;

    @Test
    void testItWorks() {{
        Assertions.assertTrue(application.isRunning());
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Javalin => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class AppTest {{
    @Test
    void testGreeting() {{
        App app = new App();
        assertEquals("Hello from Javalin!", app.getGreeting());
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Grpc => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class AppTest {{
    @Test
    void testFormatGreeting() {{
        App app = new App();
        assertEquals("Hello, Alice! Welcome to gRPC.", app.formatGreeting("Alice"));
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Picocli => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import picocli.CommandLine;
import static org.junit.jupiter.api.Assertions.*;

class AppTest {{
    @Test
    void testHelpOption() {{
        App app = new App();
        CommandLine cmd = new CommandLine(app);
        int exitCode = cmd.execute("--help");
        assertEquals(0, exitCode);
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::MultiModuleCore => (
                    "DomainServiceTest.java",
                    format!(
                        r#"package {}.core;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class DomainServiceTest {{
    @Test
    void testProcess() {{
        DomainService service = new DomainService();
        assertEquals("Processed: test", service.process("test"));
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::MultiModuleApi => (
                    "AppTest.java",
                    format!(
                        r#"package {}.api;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class AppTest {{
    @Test
    void testAppLoads() {{
        assertTrue(true);
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
            };
            fs::write(test_java_dir.join(file_name), content)?;
        }
        TestFramework::AssertJ => {
            let (file_name, content) = match kind {
                TestKind::Cli => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import static org.assertj.core.api.Assertions.assertThat;

class AppTest {{
    @Test
    void testAppSanity() {{
        App app = new App();
        assertThat(app.getGreeting()).isEqualTo("Hello World!");
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Lib => (
                    "LibraryTest.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import static org.assertj.core.api.Assertions.assertThat;

class LibraryTest {{
    @Test
    void testSomeLibraryMethod() {{
        Library classUnderTest = new Library();
        assertThat(classUnderTest.someLibraryMethod()).isTrue();
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::SpringBoot => (
                    "ApplicationTests.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import org.springframework.boot.test.context.SpringBootTest;
import static org.assertj.core.api.Assertions.assertThat;

@SpringBootTest
class ApplicationTests {{
    @Test
    void contextLoads() {{
        assertThat(1 + 1).isEqualTo(2);
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Quarkus => (
                    "GreetingResourceTest.java",
                    format!(
                        r#"package {};

import io.quarkus.test.junit.QuarkusTest;
import org.junit.jupiter.api.Test;
import static io.restassured.RestAssured.given;
import static org.hamcrest.CoreMatchers.is;

@QuarkusTest
class GreetingResourceTest {{
    @Test
    void testHelloEndpoint() {{
        given()
          .when().get("/hello")
          .then()
             .statusCode(200)
             .body(is("Hello from Quarkus + djuke"));
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Micronaut => (
                    "ApplicationTest.java",
                    format!(
                        r#"package {};

import io.micronaut.test.extensions.junit5.annotation.MicronautTest;
import org.junit.jupiter.api.Test;
import static org.assertj.core.api.Assertions.assertThat;
import jakarta.inject.Inject;
import io.micronaut.runtime.EmbeddedApplication;

@MicronautTest
class ApplicationTest {{
    @Inject
    EmbeddedApplication<?> application;

    @Test
    void testItWorks() {{
        assertThat(application.isRunning()).isTrue();
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Javalin => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import static org.assertj.core.api.Assertions.assertThat;

class AppTest {{
    @Test
    void testGreeting() {{
        App app = new App();
        assertThat(app.getGreeting()).isEqualTo("Hello from Javalin!");
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Grpc => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import static org.assertj.core.api.Assertions.assertThat;

class AppTest {{
    @Test
    void testFormatGreeting() {{
        App app = new App();
        assertThat(app.formatGreeting("Alice")).isEqualTo("Hello, Alice! Welcome to gRPC.");
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Picocli => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.junit.jupiter.api.Test;
import picocli.CommandLine;
import static org.assertj.core.api.Assertions.assertThat;

class AppTest {{
    @Test
    void testHelpOption() {{
        App app = new App();
        CommandLine cmd = new CommandLine(app);
        int exitCode = cmd.execute("--help");
        assertThat(exitCode).isZero();
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::MultiModuleCore => (
                    "DomainServiceTest.java",
                    format!(
                        r#"package {}.core;

import org.junit.jupiter.api.Test;
import static org.assertj.core.api.Assertions.assertThat;

class DomainServiceTest {{
    @Test
    void testProcess() {{
        DomainService service = new DomainService();
        assertThat(service.process("test")).isEqualTo("Processed: test");
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::MultiModuleApi => (
                    "AppTest.java",
                    format!(
                        r#"package {}.api;

import org.junit.jupiter.api.Test;
import static org.assertj.core.api.Assertions.assertThat;

class AppTest {{
    @Test
    void testAppLoads() {{
        assertThat(true).isTrue();
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
            };
            fs::write(test_java_dir.join(file_name), content)?;
        }
        TestFramework::TestNG => {
            let (file_name, content) = match kind {
                TestKind::Cli => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.testng.Assert;
import org.testng.annotations.Test;

public class AppTest {{
    @Test
    public void testAppSanity() {{
        App app = new App();
        Assert.assertEquals(app.getGreeting(), "Hello World!");
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Lib => (
                    "LibraryTest.java",
                    format!(
                        r#"package {};

import org.testng.Assert;
import org.testng.annotations.Test;

public class LibraryTest {{
    @Test
    public void testSomeLibraryMethod() {{
        Library classUnderTest = new Library();
        Assert.assertTrue(classUnderTest.someLibraryMethod());
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::SpringBoot => (
                    "ApplicationTests.java",
                    format!(
                        r#"package {};

import org.springframework.boot.test.context.SpringBootTest;
import org.testng.Assert;
import org.testng.annotations.Test;

@SpringBootTest
public class ApplicationTests {{
    @Test
    public void contextLoads() {{
        Assert.assertTrue(true);
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Quarkus => (
                    "GreetingResourceTest.java",
                    format!(
                        r#"package {};

import io.quarkus.test.junit.QuarkusTest;
import org.testng.Assert;
import org.testng.annotations.Test;

@QuarkusTest
public class GreetingResourceTest {{
    @Test
    public void testHelloEndpoint() {{
        Assert.assertTrue(true);
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Micronaut => (
                    "ApplicationTest.java",
                    format!(
                        r#"package {};

import io.micronaut.test.extensions.junit5.annotation.MicronautTest;
import org.testng.Assert;
import org.testng.annotations.Test;
import jakarta.inject.Inject;
import io.micronaut.runtime.EmbeddedApplication;

@MicronautTest
public class ApplicationTest {{
    @Inject
    EmbeddedApplication<?> application;

    @Test
    public void testItWorks() {{
        Assert.assertTrue(application.isRunning());
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Javalin => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.testng.Assert;
import org.testng.annotations.Test;

public class AppTest {{
    @Test
    public void testGreeting() {{
        App app = new App();
        Assert.assertEquals(app.getGreeting(), "Hello from Javalin!");
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Grpc => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.testng.Assert;
import org.testng.annotations.Test;

public class AppTest {{
    @Test
    public void testFormatGreeting() {{
        App app = new App();
        Assert.assertEquals(app.formatGreeting("Alice"), "Hello, Alice! Welcome to gRPC.");
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::Picocli => (
                    "AppTest.java",
                    format!(
                        r#"package {};

import org.testng.Assert;
import org.testng.annotations.Test;
import picocli.CommandLine;

public class AppTest {{
    @Test
    public void testHelpOption() {{
        App app = new App();
        CommandLine cmd = new CommandLine(app);
        int exitCode = cmd.execute("--help");
        Assert.assertEquals(exitCode, 0);
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::MultiModuleCore => (
                    "DomainServiceTest.java",
                    format!(
                        r#"package {}.core;

import org.testng.Assert;
import org.testng.annotations.Test;

public class DomainServiceTest {{
    @Test
    public void testProcess() {{
        DomainService service = new DomainService();
        Assert.assertEquals(service.process("test"), "Processed: test");
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
                TestKind::MultiModuleApi => (
                    "AppTest.java",
                    format!(
                        r#"package {}.api;

import org.testng.Assert;
import org.testng.annotations.Test;

public class AppTest {{
    @Test
    public void testAppLoads() {{
        Assert.assertTrue(true);
    }}
}}
"#,
                        opts.group_id
                    ),
                ),
            };
            fs::write(test_java_dir.join(file_name), content)?;
        }
    }

    Ok(())
}

fn generate_cli_source(main: &Path, dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let app_java = format!(
        r#"package {};

public class App {{
    public static void main(String[] args) {{
        System.out.println("Hello from {} powered by djuke!");
    }}

    public String getGreeting() {{
        return "Hello World!";
    }}
}}
"#,
        opts.group_id, opts.name
    );
    fs::write(main.join("App.java"), app_java)?;
    generate_test_for_template(dest, opts, TestKind::Cli)?;
    Ok(())
}

fn generate_lib_source(main: &Path, dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let lib_java = format!(
        r#"package {};

public class Library {{
    public boolean someLibraryMethod() {{
        return true;
    }}
}}
"#,
        opts.group_id
    );
    fs::write(main.join("Library.java"), lib_java)?;
    generate_test_for_template(dest, opts, TestKind::Lib)?;
    Ok(())
}

fn generate_spring_source(main: &Path, dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let app_java = format!(
        r#"package {};

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;
import java.util.Map;

@SpringBootApplication
@RestController
public class Application {{
    public static void main(String[] args) {{
        SpringApplication.run(Application.class, args);
    }}

    @GetMapping("/")
    public Map<String, Object> index() {{
        return Map.of(
            "status", "UP",
            "message", "Welcome to your Spring Boot application managed by djuke!",
            "framework", "Spring Boot",
            "runtime", System.getProperty("java.version"),
            "toolchain", "djuke"
        );
    }}
}}
"#,
        opts.group_id
    );
    fs::write(main.join("Application.java"), app_java)?;
    generate_test_for_template(dest, opts, TestKind::SpringBoot)?;
    Ok(())
}

fn generate_quarkus_source(main: &Path, dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let resource_java = format!(
        r#"package {};

import jakarta.ws.rs.GET;
import jakarta.ws.rs.Path;
import jakarta.ws.rs.Produces;
import jakarta.ws.rs.core.MediaType;

@Path("/hello")
public class GreetingResource {{
    @GET
    @Produces(MediaType.TEXT_PLAIN)
    public String hello() {{
        return "Hello from Quarkus + djuke";
    }}
}}
"#,
        opts.group_id
    );
    fs::write(main.join("GreetingResource.java"), resource_java)?;
    generate_test_for_template(dest, opts, TestKind::Quarkus)?;
    Ok(())
}

fn generate_micronaut_source(main: &Path, dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let app_java = format!(
        r#"package {};

import io.micronaut.runtime.Micronaut;

public class Application {{
    public static void main(String[] args) {{
        Micronaut.run(Application.class, args);
    }}
}}
"#,
        opts.group_id
    );
    fs::write(main.join("Application.java"), app_java)?;
    generate_test_for_template(dest, opts, TestKind::Micronaut)?;
    Ok(())
}

fn generate_javalin_source(main: &Path, dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let app_java = format!(
        r#"package {};

import io.javalin.Javalin;
import java.util.Map;

public class App {{
    public static void main(String[] args) {{
        var app = Javalin.create(config -> {{
            config.bundledPlugins.enableCors(cors -> {{
                cors.addRule(it -> it.anyHost());
            }});
        }}).start(8080);

        app.get("/", ctx -> ctx.json(Map.of(
            "message", "Hello from Javalin powered by djuke!",
            "service", "{}",
            "status", "healthy"
        )));

        app.get("/health", ctx -> ctx.json(Map.of("status", "UP")));

        System.out.println("Javalin server running at http://localhost:8080");
    }}

    public String getGreeting() {{
        return "Hello from Javalin!";
    }}
}}
"#,
        opts.group_id, opts.name
    );
    fs::write(main.join("App.java"), app_java)?;
    generate_test_for_template(dest, opts, TestKind::Javalin)?;
    Ok(())
}

fn generate_grpc_source(main: &Path, dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let proto_dir = dest.join("src").join("main").join("proto");
    fs::create_dir_all(&proto_dir)?;

    let proto_file = format!(
        r#"syntax = "proto3";

option java_multiple_files = true;
option java_package = "{}.grpc";
option java_outer_classname = "GreetingProto";

package greeting;

service GreetingService {{
  rpc SayHello (HelloRequest) returns (HelloReply) {{}}
}}

message HelloRequest {{
  string name = 1;
}}

message HelloReply {{
  string message = 1;
}}
"#,
        opts.group_id
    );
    fs::write(proto_dir.join("greeting.proto"), proto_file)?;

    let app_java = format!(
        r#"package {};

public class App {{
    public static void main(String[] args) {{
        System.out.println("gRPC Service initialized for {}");
        System.out.println("Proto file configured in src/main/proto/greeting.proto");
    }}

    public String formatGreeting(String name) {{
        return "Hello, " + name + "! Welcome to gRPC.";
    }}
}}
"#,
        opts.group_id, opts.name
    );
    fs::write(main.join("App.java"), app_java)?;
    generate_test_for_template(dest, opts, TestKind::Grpc)?;
    Ok(())
}

fn generate_picocli_source(main: &Path, dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let app_java = format!(
        r#"package {};

import picocli.CommandLine;
import picocli.CommandLine.Command;
import picocli.CommandLine.Option;

import java.util.concurrent.Callable;

@Command(
    name = "{}",
    mixinStandardHelpOptions = true,
    version = "1.0.0",
    description = "Production-grade CLI application powered by djuke and Picocli."
)
public class App implements Callable<Integer> {{

    @Option(names = {{"-n", "--name"}}, description = "Target name to greet", defaultValue = "World")
    private String name;

    @Option(names = {{"-v", "--verbose"}}, description = "Enable verbose output")
    private boolean verbose;

    @Override
    public Integer call() {{
        if (verbose) {{
            System.out.println("[DEBUG] Running in verbose mode");
        }}
        System.out.printf("Hello, %s! Welcome to %s.%n", name, "{}");
        return 0;
    }}

    public static void main(String[] args) {{
        int exitCode = new CommandLine(new App()).execute(args);
        System.exit(exitCode);
    }}
}}
"#,
        opts.group_id, opts.name, opts.name
    );
    fs::write(main.join("App.java"), app_java)?;
    generate_test_for_template(dest, opts, TestKind::Picocli)?;
    Ok(())
}

fn generate_multi_module_project(dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let pkg_path = opts.group_id.replace('.', "/");
    let core_main = dest
        .join("core")
        .join("src")
        .join("main")
        .join("java")
        .join(&pkg_path)
        .join("core");
    let api_main = dest
        .join("api")
        .join("src")
        .join("main")
        .join("java")
        .join(&pkg_path)
        .join("api");
    let api_resources = dest.join("api").join("src").join("main").join("resources");

    fs::create_dir_all(&core_main)?;
    fs::create_dir_all(&api_main)?;
    fs::create_dir_all(&api_resources)?;

    // Core DomainService
    let service_java = format!(
        r#"package {}.core;

public class DomainService {{
    public String process(String input) {{
        return "Processed: " + input;
    }}
}}
"#,
        opts.group_id
    );
    fs::write(core_main.join("DomainService.java"), service_java)?;
    generate_test_for_template(dest, opts, TestKind::MultiModuleCore)?;

    // API App
    let app_java = format!(
        r#"package {}.api;

import {}.core.DomainService;

public class App {{
    public static void main(String[] args) {{
        DomainService service = new DomainService();
        System.out.println(service.process("Hello from {} Multi-Module API"));
    }}
}}
"#,
        opts.group_id, opts.group_id, opts.name
    );
    fs::write(api_main.join("App.java"), app_java)?;
    generate_test_for_template(dest, opts, TestKind::MultiModuleApi)?;

    // Logback for API module
    let logback_xml = r#"<configuration>
    <appender name="CONSOLE" class="ch.qos.logback.core.ConsoleAppender">
        <encoder>
            <pattern>%d{HH:mm:ss.SSS} [%thread] %highlight(%-5level) %cyan(%logger{36}) - %msg%n</pattern>
        </encoder>
    </appender>
    <root level="INFO">
        <appender-ref ref="CONSOLE" />
    </root>
</configuration>
"#;
    let _ = fs::write(api_resources.join("logback.xml"), logback_xml);

    let j_ver = effective_java_version(&opts.java_version);

    // Build files for MultiModule
    match opts.build_tool {
        BuildToolChoice::GradleKts => {
            let root_settings = format!(
                r#"rootProject.name = "{}"
include("core", "api")
"#,
                opts.name
            );
            fs::write(dest.join("settings.gradle.kts"), root_settings)?;

            let groovy_plugin = if opts.test_framework == TestFramework::Spock {
                "    apply(plugin = \"groovy\")\n"
            } else {
                ""
            };
            let test_deps = test_dependencies_gradle_kts(opts.test_framework);
            let test_cfg = test_task_gradle_kts(opts.test_framework);

            let root_build = format!(
                r#"plugins {{
    java
}}

allprojects {{
    group = "{}"
    version = "0.1.0-SNAPSHOT"

    repositories {{
        mavenCentral()
    }}
}}

subprojects {{
    apply(plugin = "java")
{}
    java {{
        toolchain {{
            languageVersion.set(JavaLanguageVersion.of({}))
        }}
    }}

    dependencies {{
{}
    }}

    {}
}}
"#,
                opts.group_id, groovy_plugin, j_ver, test_deps, test_cfg
            );
            fs::write(dest.join("build.gradle.kts"), root_build)?;

            let core_build = r#"plugins {
    `java-library`
}

dependencies {
    api("org.slf4j:slf4j-api:2.0.16")
}
"#;
            fs::write(dest.join("core").join("build.gradle.kts"), core_build)?;

            let api_build = format!(
                r#"plugins {{
    application
}}

application {{
    mainClass.set("{}.api.App")
}}

dependencies {{
    implementation(project(":core"))
    implementation("ch.qos.logback:logback-classic:1.5.16")
}}
"#,
                opts.group_id
            );
            fs::write(dest.join("api").join("build.gradle.kts"), api_build)?;

            let j_num = j_ver.parse::<u32>().unwrap_or(21);
            let jvmargs = if j_num >= 24 {
                "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8 --enable-native-access=ALL-UNNAMED"
            } else {
                "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8"
            };
            let props = format!(
                "{}\norg.gradle.parallel=true\norg.gradle.caching=true\n",
                jvmargs
            );
            let _ = fs::write(dest.join("gradle.properties"), props);
        }
        BuildToolChoice::GradleGroovy => {
            let root_settings = format!(
                r#"rootProject.name = '{}'
include 'core', 'api'
"#,
                opts.name
            );
            fs::write(dest.join("settings.gradle"), root_settings)?;

            let groovy_plugin = if opts.test_framework == TestFramework::Spock {
                "    apply plugin: 'groovy'\n"
            } else {
                ""
            };
            let test_deps = test_dependencies_gradle_groovy(opts.test_framework);
            let test_cfg = test_task_gradle_groovy(opts.test_framework);

            let root_build = format!(
                r#"allprojects {{
    group = '{}'
    version = '0.1.0-SNAPSHOT'

    repositories {{
        mavenCentral()
    }}
}}

subprojects {{
    apply plugin: 'java'
{}
    java {{
        toolchain {{
            languageVersion = JavaLanguageVersion.of({})
        }}
    }}

    dependencies {{
{}
    }}

    {}
}}
"#,
                opts.group_id, groovy_plugin, j_ver, test_deps, test_cfg
            );
            fs::write(dest.join("build.gradle"), root_build)?;

            let core_build = r#"plugins {
    id 'java-library'
}

dependencies {
    api 'org.slf4j:slf4j-api:2.0.16'
}
"#;
            fs::write(dest.join("core").join("build.gradle"), core_build)?;

            let api_build = format!(
                r#"plugins {{
    id 'application'
}}

application {{
    mainClass = '{}.api.App'
}}

dependencies {{
    implementation project(':core')
    implementation 'ch.qos.logback:logback-classic:1.5.16'
}}
"#,
                opts.group_id
            );
            fs::write(dest.join("api").join("build.gradle"), api_build)?;

            let j_num = j_ver.parse::<u32>().unwrap_or(21);
            let jvmargs = if j_num >= 24 {
                "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8 --enable-native-access=ALL-UNNAMED"
            } else {
                "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8"
            };
            let props = format!(
                "{}\norg.gradle.parallel=true\norg.gradle.caching=true\n",
                jvmargs
            );
            let _ = fs::write(dest.join("gradle.properties"), props);
        }
        BuildToolChoice::Maven => {
            let root_pom = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <groupId>{}</groupId>
    <artifactId>{}</artifactId>
    <version>0.1.0-SNAPSHOT</version>
    <packaging>pom</packaging>

    <modules>
        <module>core</module>
        <module>api</module>
    </modules>

    <properties>
        <maven.compiler.release>{}</maven.compiler.release>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <junit.version>5.11.4</junit.version>
    </properties>

    <dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>org.junit</groupId>
                <artifactId>junit-bom</artifactId>
                <version>${{junit.version}}</version>
                <type>pom</type>
                <scope>import</scope>
            </dependency>
        </dependencies>
    </dependencyManagement>
</project>
"#,
                opts.group_id, opts.name, j_ver
            );
            fs::write(dest.join("pom.xml"), root_pom)?;

            let test_deps = test_dependencies_maven(opts.test_framework);
            let spock_plugin = gmavenplus_plugin_maven(opts.test_framework);

            let core_pom = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <parent>
        <groupId>{}</groupId>
        <artifactId>{}</artifactId>
        <version>0.1.0-SNAPSHOT</version>
    </parent>

    <artifactId>core</artifactId>

    <dependencies>
        <dependency>
            <groupId>org.slf4j</groupId>
            <artifactId>slf4j-api</artifactId>
            <version>2.0.16</version>
        </dependency>
{}
    </dependencies>
    <build>
        <plugins>
{}
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, test_deps, spock_plugin
            );
            fs::write(dest.join("core").join("pom.xml"), core_pom)?;

            let api_pom = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <parent>
        <groupId>{}</groupId>
        <artifactId>{}</artifactId>
        <version>0.1.0-SNAPSHOT</version>
    </parent>

    <artifactId>api</artifactId>

    <dependencies>
        <dependency>
            <groupId>{}</groupId>
            <artifactId>core</artifactId>
            <version>${{project.version}}</version>
        </dependency>
        <dependency>
            <groupId>ch.qos.logback</groupId>
            <artifactId>logback-classic</artifactId>
            <version>1.5.16</version>
        </dependency>
{}
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>org.codehaus.mojo</groupId>
                <artifactId>exec-maven-plugin</artifactId>
                <version>3.5.0</version>
                <configuration>
                    <mainClass>{}.api.App</mainClass>
                </configuration>
            </plugin>
{}
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, opts.group_id, test_deps, opts.group_id, spock_plugin
            );
            fs::write(dest.join("api").join("pom.xml"), api_pom)?;
        }
    }

    generate_standard_configs(dest, opts)?;

    // djuke.toml
    let tool_name = match opts.build_tool {
        BuildToolChoice::GradleKts | BuildToolChoice::GradleGroovy => "gradle",
        BuildToolChoice::Maven => "maven",
    };
    let distro = opts
        .distribution
        .unwrap_or_else(djuke_core::JdkDistribution::default_distro);

    let djuke_toml = format!(
        r#"[project]
name = "{}"

[java]
version = "{}"
distribution = "{}"

[build]
tool = "{}"
wrapper = true
"#,
        opts.name, opts.java_version, distro, tool_name
    );
    fs::write(dest.join("djuke.toml"), djuke_toml)?;

    let readme = format!(
        r#"# {} (Multi-Module Project)

Enterprise multi-module project with `core` library and `api` service.

## Architecture

- `core`: Shared business logic and domain services.
- `api`: Executable application service depending on `core`.

## Commands

```bash
djuke build
djuke test
djuke run
```
"#,
        opts.name
    );
    fs::write(dest.join("README.md"), readme)?;

    Ok(())
}

fn effective_java_version(v: &str) -> &str {
    let trimmed = v.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("lts") {
        "21"
    } else if trimmed.eq_ignore_ascii_case("latest") {
        "25"
    } else {
        trimmed
    }
}

fn test_dependencies_gradle_kts(tf: TestFramework) -> &'static str {
    match tf {
        TestFramework::JUnit5 => {
            "    testImplementation(platform(\"org.junit:junit-bom:5.11.4\"))\n    testImplementation(\"org.junit.jupiter:junit-jupiter\")\n    testRuntimeOnly(\"org.junit.platform:junit-platform-launcher\")"
        }
        TestFramework::AssertJ => {
            "    testImplementation(platform(\"org.junit:junit-bom:5.11.4\"))\n    testImplementation(\"org.junit.jupiter:junit-jupiter\")\n    testImplementation(\"org.assertj:assertj-core:3.27.3\")\n    testRuntimeOnly(\"org.junit.platform:junit-platform-launcher\")"
        }
        TestFramework::Spock => {
            "    testImplementation(\"org.spockframework:spock-core:2.4-M5-groovy-4.0\")\n    testImplementation(\"org.apache.groovy:groovy:4.0.24\")"
        }
        TestFramework::TestNG => {
            "    testImplementation(\"org.testng:testng:7.10.2\")"
        }
        TestFramework::None => "",
    }
}

fn test_task_gradle_kts(tf: TestFramework) -> &'static str {
    match tf {
        TestFramework::JUnit5 | TestFramework::AssertJ | TestFramework::Spock => {
            "tasks.withType<Test> {\n    useJUnitPlatform()\n    testLogging {\n        events(\"passed\", \"skipped\", \"failed\")\n    }\n}"
        }
        TestFramework::TestNG => {
            "tasks.withType<Test> {\n    useTestNG()\n    testLogging {\n        events(\"passed\", \"skipped\", \"failed\")\n    }\n}"
        }
        TestFramework::None => "tasks.withType<Test> {\n    enabled = false\n}",
    }
}

fn test_dependencies_gradle_groovy(tf: TestFramework) -> &'static str {
    match tf {
        TestFramework::JUnit5 => {
            "    testImplementation platform('org.junit:junit-bom:5.11.4')\n    testImplementation 'org.junit.jupiter:junit-jupiter'\n    testRuntimeOnly 'org.junit.platform:junit-platform-launcher'"
        }
        TestFramework::AssertJ => {
            "    testImplementation platform('org.junit:junit-bom:5.11.4')\n    testImplementation 'org.junit.jupiter:junit-jupiter'\n    testImplementation 'org.assertj:assertj-core:3.27.3'\n    testRuntimeOnly 'org.junit.platform:junit-platform-launcher'"
        }
        TestFramework::Spock => {
            "    testImplementation 'org.spockframework:spock-core:2.4-M5-groovy-4.0'\n    testImplementation 'org.apache.groovy:groovy:4.0.24'"
        }
        TestFramework::TestNG => {
            "    testImplementation 'org.testng:testng:7.10.2'"
        }
        TestFramework::None => "",
    }
}

fn test_task_gradle_groovy(tf: TestFramework) -> &'static str {
    match tf {
        TestFramework::JUnit5 | TestFramework::AssertJ | TestFramework::Spock => {
            "tasks.withType(Test).configureEach {\n    useJUnitPlatform()\n    testLogging {\n        events('passed', 'skipped', 'failed')\n    }\n}"
        }
        TestFramework::TestNG => {
            "tasks.withType(Test).configureEach {\n    useTestNG()\n    testLogging {\n        events('passed', 'skipped', 'failed')\n    }\n}"
        }
        TestFramework::None => "tasks.withType(Test).configureEach {\n    enabled = false\n}",
    }
}

fn test_dependencies_maven(tf: TestFramework) -> &'static str {
    match tf {
        TestFramework::JUnit5 => {
            r#"        <dependency>
            <groupId>org.junit.jupiter</groupId>
            <artifactId>junit-jupiter</artifactId>
            <version>${junit.version}</version>
            <scope>test</scope>
        </dependency>"#
        }
        TestFramework::AssertJ => {
            r#"        <dependency>
            <groupId>org.junit.jupiter</groupId>
            <artifactId>junit-jupiter</artifactId>
            <version>${junit.version}</version>
            <scope>test</scope>
        </dependency>
        <dependency>
            <groupId>org.assertj</groupId>
            <artifactId>assertj-core</artifactId>
            <version>3.27.3</version>
            <scope>test</scope>
        </dependency>"#
        }
        TestFramework::Spock => {
            r#"        <dependency>
            <groupId>org.spockframework</groupId>
            <artifactId>spock-core</artifactId>
            <version>2.4-M5-groovy-4.0</version>
            <scope>test</scope>
        </dependency>
        <dependency>
            <groupId>org.apache.groovy</groupId>
            <artifactId>groovy</artifactId>
            <version>4.0.24</version>
            <scope>test</scope>
        </dependency>"#
        }
        TestFramework::TestNG => {
            r#"        <dependency>
            <groupId>org.testng</groupId>
            <artifactId>testng</artifactId>
            <version>7.10.2</version>
            <scope>test</scope>
        </dependency>"#
        }
        TestFramework::None => "",
    }
}

fn gmavenplus_plugin_maven(tf: TestFramework) -> &'static str {
    if tf == TestFramework::Spock {
        r#"            <plugin>
                <groupId>org.codehaus.gmavenplus</groupId>
                <artifactId>gmavenplus-plugin</artifactId>
                <version>3.0.2</version>
                <executions>
                    <execution>
                        <goals>
                            <goal>compileTests</goal>
                        </goals>
                    </execution>
                </executions>
            </plugin>"#
    } else {
        ""
    }
}

fn initializr_deps_gradle_kts(deps: Option<&[String]>) -> String {
    let default_deps = ["web", "actuator"];
    let default_vec: Vec<String> = default_deps.iter().map(|s| s.to_string()).collect();
    let list: &[String] = match deps {
        Some(d) if !d.is_empty() => d,
        _ => &default_vec,
    };
    let mut lines = Vec::new();
    for dep in list {
        match dep.as_str() {
            "web" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-web\")"
                    .to_string(),
            ),
            "actuator" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-actuator\")"
                    .to_string(),
            ),
            "data-jpa" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-data-jpa\")"
                    .to_string(),
            ),
            "security" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-security\")"
                    .to_string(),
            ),
            "validation" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-validation\")"
                    .to_string(),
            ),
            "devtools" => lines.push(
                "    developmentOnly(\"org.springframework.boot:spring-boot-devtools\")"
                    .to_string(),
            ),
            "thymeleaf" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-thymeleaf\")"
                    .to_string(),
            ),
            "data-redis" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-data-redis\")"
                    .to_string(),
            ),
            "kafka" => lines
                .push("    implementation(\"org.springframework.kafka:spring-kafka\")".to_string()),
            "postgresql" => {
                lines.push("    runtimeOnly(\"org.postgresql:postgresql\")".to_string())
            }
            "mysql" => lines.push("    runtimeOnly(\"com.mysql:mysql-connector-j\")".to_string()),
            "h2" => lines.push("    runtimeOnly(\"com.h2database:h2\")".to_string()),
            "lombok" => {
                lines.push("    compileOnly(\"org.projectlombok:lombok\")".to_string());
                lines.push("    annotationProcessor(\"org.projectlombok:lombok\")".to_string());
            }
            "docker-compose" => lines.push(
                "    developmentOnly(\"org.springframework.boot:spring-boot-docker-compose\")"
                    .to_string(),
            ),
            "testcontainers" => lines.push(
                "    testImplementation(\"org.springframework.boot:spring-boot-testcontainers\")"
                    .to_string(),
            ),
            "graphql" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-graphql\")"
                    .to_string(),
            ),
            "websocket" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-websocket\")"
                    .to_string(),
            ),
            "mail" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-mail\")"
                    .to_string(),
            ),
            "quartz" => lines.push(
                "    implementation(\"org.springframework.boot:spring-boot-starter-quartz\")"
                    .to_string(),
            ),
            other => {
                if other.starts_with("org.") || other.contains(':') {
                    lines.push(format!("    implementation(\"{}\")", other));
                } else {
                    lines.push(format!(
                        "    implementation(\"org.springframework.boot:spring-boot-starter-{}\")",
                        other
                    ));
                }
            }
        }
    }
    lines.join("\n")
}

fn initializr_deps_gradle_groovy(deps: Option<&[String]>) -> String {
    let default_deps = ["web", "actuator"];
    let default_vec: Vec<String> = default_deps.iter().map(|s| s.to_string()).collect();
    let list: &[String] = match deps {
        Some(d) if !d.is_empty() => d,
        _ => &default_vec,
    };
    let mut lines = Vec::new();
    for dep in list {
        match dep.as_str() {
            "web" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-web'".to_string(),
            ),
            "actuator" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-actuator'"
                    .to_string(),
            ),
            "data-jpa" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-data-jpa'"
                    .to_string(),
            ),
            "security" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-security'"
                    .to_string(),
            ),
            "validation" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-validation'"
                    .to_string(),
            ),
            "devtools" => lines.push(
                "    developmentOnly 'org.springframework.boot:spring-boot-devtools'".to_string(),
            ),
            "thymeleaf" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-thymeleaf'"
                    .to_string(),
            ),
            "data-redis" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-data-redis'"
                    .to_string(),
            ),
            "kafka" => lines
                .push("    implementation 'org.springframework.kafka:spring-kafka'".to_string()),
            "postgresql" => lines.push("    runtimeOnly 'org.postgresql:postgresql'".to_string()),
            "mysql" => lines.push("    runtimeOnly 'com.mysql:mysql-connector-j'".to_string()),
            "h2" => lines.push("    runtimeOnly 'com.h2database:h2'".to_string()),
            "lombok" => {
                lines.push("    compileOnly 'org.projectlombok:lombok'".to_string());
                lines.push("    annotationProcessor 'org.projectlombok:lombok'".to_string());
            }
            "docker-compose" => lines.push(
                "    developmentOnly 'org.springframework.boot:spring-boot-docker-compose'"
                    .to_string(),
            ),
            "testcontainers" => lines.push(
                "    testImplementation 'org.springframework.boot:spring-boot-testcontainers'"
                    .to_string(),
            ),
            "graphql" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-graphql'"
                    .to_string(),
            ),
            "websocket" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-websocket'"
                    .to_string(),
            ),
            "mail" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-mail'"
                    .to_string(),
            ),
            "quartz" => lines.push(
                "    implementation 'org.springframework.boot:spring-boot-starter-quartz'"
                    .to_string(),
            ),
            other => {
                if other.starts_with("org.") || other.contains(':') {
                    lines.push(format!("    implementation '{}'", other));
                } else {
                    lines.push(format!(
                        "    implementation 'org.springframework.boot:spring-boot-starter-{}'",
                        other
                    ));
                }
            }
        }
    }
    lines.join("\n")
}

fn initializr_deps_maven(deps: Option<&[String]>) -> String {
    let default_deps = ["web", "actuator"];
    let default_vec: Vec<String> = default_deps.iter().map(|s| s.to_string()).collect();
    let list: &[String] = match deps {
        Some(d) if !d.is_empty() => d,
        _ => &default_vec,
    };
    let mut entries = Vec::new();
    for dep in list {
        match dep.as_str() {
            "web" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "actuator" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-actuator</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "data-jpa" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-data-jpa</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "security" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-security</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "validation" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-validation</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "devtools" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-devtools</artifactId>
            <scope>runtime</scope>
            <optional>true</optional>
        </dependency>"#
                    .to_string(),
            ),
            "thymeleaf" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-thymeleaf</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "data-redis" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-data-redis</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "kafka" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.kafka</groupId>
            <artifactId>spring-kafka</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "postgresql" => entries.push(
                r#"        <dependency>
            <groupId>org.postgresql</groupId>
            <artifactId>postgresql</artifactId>
            <scope>runtime</scope>
        </dependency>"#
                    .to_string(),
            ),
            "mysql" => entries.push(
                r#"        <dependency>
            <groupId>com.mysql</groupId>
            <artifactId>mysql-connector-j</artifactId>
            <scope>runtime</scope>
        </dependency>"#
                    .to_string(),
            ),
            "h2" => entries.push(
                r#"        <dependency>
            <groupId>com.h2database</groupId>
            <artifactId>h2</artifactId>
            <scope>runtime</scope>
        </dependency>"#
                    .to_string(),
            ),
            "lombok" => entries.push(
                r#"        <dependency>
            <groupId>org.projectlombok</groupId>
            <artifactId>lombok</artifactId>
            <optional>true</optional>
        </dependency>"#
                    .to_string(),
            ),
            "docker-compose" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-docker-compose</artifactId>
            <scope>runtime</scope>
            <optional>true</optional>
        </dependency>"#
                    .to_string(),
            ),
            "testcontainers" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-testcontainers</artifactId>
            <scope>test</scope>
        </dependency>"#
                    .to_string(),
            ),
            "graphql" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-graphql</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "websocket" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-websocket</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "mail" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-mail</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            "quartz" => entries.push(
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-quartz</artifactId>
        </dependency>"#
                    .to_string(),
            ),
            other => {
                if other.starts_with("org.") || other.contains(':') {
                    let parts: Vec<&str> = other.split(':').collect();
                    if parts.len() >= 2 {
                        entries.push(format!(
                            r#"        <dependency>
            <groupId>{}</groupId>
            <artifactId>{}</artifactId>
        </dependency>"#,
                            parts[0], parts[1]
                        ));
                    } else {
                        entries.push(format!(
                            r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-{}</artifactId>
        </dependency>"#,
                            other
                        ));
                    }
                } else {
                    entries.push(format!(
                        r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-{}</artifactId>
        </dependency>"#,
                        other
                    ));
                }
            }
        }
    }
    entries.join("\n")
}

fn generate_gradle_kts(dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let settings = format!("rootProject.name = \"{}\"\n", opts.name);
    fs::write(dest.join("settings.gradle.kts"), settings)?;

    let j_ver = effective_java_version(&opts.java_version);
    let test_deps = test_dependencies_gradle_kts(opts.test_framework);
    let test_cfg = test_task_gradle_kts(opts.test_framework);

    let groovy_plugin = if opts.test_framework == TestFramework::Spock {
        "\n    groovy"
    } else {
        ""
    };

    let (plugins, deps) = match opts.template {
        TemplateType::Cli => (
            format!("    application\n    `java-library`{}", groovy_plugin),
            format!(
                r#"{}

application {{
    mainClass.set("{}.App")
}}"#,
                test_deps, opts.group_id
            ),
        ),
        TemplateType::Lib => (
            format!("    `java-library`\n    `maven-publish`{}", groovy_plugin),
            test_deps.to_string(),
        ),
        TemplateType::SpringBoot => {
            let sb_ver = opts.framework_version.as_deref().unwrap_or("3.4.0");
            let sb_test = if opts.test_framework != TestFramework::None {
                r#"    testImplementation("org.springframework.boot:spring-boot-starter-test")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")"#
            } else {
                ""
            };
            let initializr_lines =
                initializr_deps_gradle_kts(opts.initializr_dependencies.as_deref());
            let spock_extra = if opts.test_framework == TestFramework::Spock {
                format!("\n{}", test_deps)
            } else {
                String::new()
            };
            (
                format!(
                    r#"    java
    id("org.springframework.boot") version "{}"
    id("io.spring.dependency-management") version "1.1.6"{}"#,
                    sb_ver, groovy_plugin
                ),
                format!(
                    r#"{}
{}{}"#,
                    initializr_lines, sb_test, spock_extra
                ),
            )
        }
        TemplateType::Quarkus => {
            let q_ver = opts.framework_version.as_deref().unwrap_or("3.15.1");
            let q_test = if opts.test_framework != TestFramework::None {
                r#"    testImplementation("io.quarkus:quarkus-junit5")
    testImplementation("io.rest-assured:rest-assured")"#
            } else {
                ""
            };
            let spock_extra = if opts.test_framework == TestFramework::Spock {
                format!("\n{}", test_deps)
            } else {
                String::new()
            };
            (
                format!(
                    r#"    java
    id("io.quarkus") version "{}"{}"#,
                    q_ver, groovy_plugin
                ),
                format!(
                    r#"    implementation(enforcedPlatform("io.quarkus.platform:quarkus-bom:{}"))
    implementation("io.quarkus:quarkus-resteasy-reactive")
{}{}"#,
                    q_ver, q_test, spock_extra
                ),
            )
        }
        TemplateType::Micronaut => {
            let mn_ver = opts.framework_version.as_deref().unwrap_or("4.4.2");
            let mn_test = if opts.test_framework != TestFramework::None {
                r#"    testImplementation("io.micronaut.test:micronaut-test-junit5")"#
            } else {
                ""
            };
            let spock_extra = if opts.test_framework == TestFramework::Spock {
                format!("\n{}", test_deps)
            } else {
                String::new()
            };
            (
                format!(
                    r#"    java
    id("io.micronaut.application") version "{}"{}"#,
                    mn_ver, groovy_plugin
                ),
                format!(
                    r#"    annotationProcessor("io.micronaut:micronaut-inject-java")
    implementation("io.micronaut:micronaut-http-server-netty")
    runtimeOnly("ch.qos.logback:logback-classic")
{}{}"#,
                    mn_test, spock_extra
                ),
            )
        }
        TemplateType::Javalin => (
            format!("    application\n    java{}", groovy_plugin),
            format!(
                r#"    implementation("io.javalin:javalin:6.3.0")
    implementation("com.fasterxml.jackson.core:jackson-databind:2.18.2")
    implementation("ch.qos.logback:logback-classic:1.5.16")
{}

application {{
    mainClass.set("{}.App")
}}"#,
                test_deps, opts.group_id
            ),
        ),
        TemplateType::Grpc => (
            format!("    application\n    java{}", groovy_plugin),
            format!(
                r#"    implementation("io.grpc:grpc-netty-shaded:1.69.0")
    implementation("io.grpc:grpc-protobuf:1.69.0")
    implementation("io.grpc:grpc-stub:1.69.0")
    compileOnly("javax.annotation:javax.annotation-api:1.3.2")
    implementation("ch.qos.logback:logback-classic:1.5.16")
{}

application {{
    mainClass.set("{}.App")
}}"#,
                test_deps, opts.group_id
            ),
        ),
        TemplateType::Picocli => (
            format!("    application\n    java{}", groovy_plugin),
            format!(
                r#"    implementation("info.picocli:picocli:4.7.6")
    annotationProcessor("info.picocli:picocli-codegen:4.7.6")
{}

application {{
    mainClass.set("{}.App")
}}"#,
                test_deps, opts.group_id
            ),
        ),
        TemplateType::MultiModule => unreachable!(),
    };

    let build = format!(
        r#"plugins {{
{}
}}

group = "{}"
version = "0.1.0-SNAPSHOT"

repositories {{
    mavenCentral()
}}

java {{
    toolchain {{
        languageVersion.set(JavaLanguageVersion.of({}))
    }}
}}

tasks.withType<JavaCompile> {{
    options.encoding = "UTF-8"
    options.compilerArgs.addAll(listOf("-parameters", "-Xlint:all"))
}}

tasks.withType<Jar> {{
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
    manifest {{
        attributes["Implementation-Title"] = project.name
        attributes["Implementation-Version"] = project.version
    }}
}}

dependencies {{
{}
}}

{}
"#,
        plugins, opts.group_id, j_ver, deps, test_cfg
    );

    fs::write(dest.join("build.gradle.kts"), build)?;

    let j_num = j_ver.parse::<u32>().unwrap_or(21);
    let jvmargs = if j_num >= 24 {
        "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8 --enable-native-access=ALL-UNNAMED"
    } else {
        "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8"
    };
    let props = format!(
        "{}\norg.gradle.parallel=true\norg.gradle.caching=true\norg.gradle.configuration-cache=true\n",
        jvmargs
    );
    let _ = fs::write(dest.join("gradle.properties"), props);

    Ok(())
}

fn generate_gradle_groovy(dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let settings = format!("rootProject.name = '{}'\n", opts.name);
    fs::write(dest.join("settings.gradle"), settings)?;

    let j_ver = effective_java_version(&opts.java_version);
    let test_deps = test_dependencies_gradle_groovy(opts.test_framework);
    let test_cfg = test_task_gradle_groovy(opts.test_framework);

    let groovy_plugin = if opts.test_framework == TestFramework::Spock {
        "\n    id 'groovy'"
    } else {
        ""
    };

    let (plugins, deps, app_block) = match opts.template {
        TemplateType::Cli => (
            format!(
                "    id 'application'\n    id 'java-library'{}",
                groovy_plugin
            ),
            test_deps.to_string(),
            format!(
                r#"application {{
    mainClass = '{}.App'
}}"#,
                opts.group_id
            ),
        ),
        TemplateType::Lib => (
            format!(
                "    id 'java-library'\n    id 'maven-publish'{}",
                groovy_plugin
            ),
            test_deps.to_string(),
            String::new(),
        ),
        TemplateType::SpringBoot => {
            let sb_ver = opts.framework_version.as_deref().unwrap_or("3.4.0");
            let sb_test = if opts.test_framework != TestFramework::None {
                r#"    testImplementation 'org.springframework.boot:spring-boot-starter-test'
    testRuntimeOnly 'org.junit.platform:junit-platform-launcher'"#
            } else {
                ""
            };
            let initializr_lines =
                initializr_deps_gradle_groovy(opts.initializr_dependencies.as_deref());
            let spock_extra = if opts.test_framework == TestFramework::Spock {
                format!("\n{}", test_deps)
            } else {
                String::new()
            };
            (
                format!(
                    r#"    id 'java'
    id 'org.springframework.boot' version '{}'
    id 'io.spring.dependency-management' version '1.1.6'{}"#,
                    sb_ver, groovy_plugin
                ),
                format!(
                    r#"{}
{}{}"#,
                    initializr_lines, sb_test, spock_extra
                ),
                String::new(),
            )
        }
        TemplateType::Quarkus => {
            let q_ver = opts.framework_version.as_deref().unwrap_or("3.15.1");
            let q_test = if opts.test_framework != TestFramework::None {
                r#"    testImplementation 'io.quarkus:quarkus-junit5'
    testImplementation 'io.rest-assured:rest-assured'"#
            } else {
                ""
            };
            let spock_extra = if opts.test_framework == TestFramework::Spock {
                format!("\n{}", test_deps)
            } else {
                String::new()
            };
            (
                format!(
                    r#"    id 'java'
    id 'io.quarkus' version '{}'{}"#,
                    q_ver, groovy_plugin
                ),
                format!(
                    r#"    implementation enforcedPlatform("io.quarkus.platform:quarkus-bom:{}")
    implementation 'io.quarkus:quarkus-resteasy-reactive'
{}{}"#,
                    q_ver, q_test, spock_extra
                ),
                String::new(),
            )
        }
        TemplateType::Micronaut => {
            let mn_ver = opts.framework_version.as_deref().unwrap_or("4.4.2");
            let mn_test = if opts.test_framework != TestFramework::None {
                r#"    testImplementation 'io.micronaut.test:micronaut-test-junit5'"#
            } else {
                ""
            };
            let spock_extra = if opts.test_framework == TestFramework::Spock {
                format!("\n{}", test_deps)
            } else {
                String::new()
            };
            (
                format!(
                    r#"    id 'java'
    id 'io.micronaut.application' version '{}'{}"#,
                    mn_ver, groovy_plugin
                ),
                format!(
                    r#"    annotationProcessor 'io.micronaut:micronaut-inject-java'
    implementation 'io.micronaut:micronaut-http-server-netty'
    runtimeOnly 'ch.qos.logback:logback-classic'
{}{}"#,
                    mn_test, spock_extra
                ),
                String::new(),
            )
        }
        TemplateType::Javalin => (
            format!("    id 'application'\n    id 'java'{}", groovy_plugin),
            format!(
                r#"    implementation 'io.javalin:javalin:6.3.0'
    implementation 'com.fasterxml.jackson.core:jackson-databind:2.18.2'
    implementation 'ch.qos.logback:logback-classic:1.5.16'
{}"#,
                test_deps
            ),
            format!(
                r#"application {{
    mainClass = '{}.App'
}}"#,
                opts.group_id
            ),
        ),
        TemplateType::Grpc => (
            format!("    id 'application'\n    id 'java'{}", groovy_plugin),
            format!(
                r#"    implementation 'io.grpc:grpc-netty-shaded:1.69.0'
    implementation 'io.grpc:grpc-protobuf:1.69.0'
    implementation 'io.grpc:grpc-stub:1.69.0'
    compileOnly 'javax.annotation:javax.annotation-api:1.3.2'
    implementation 'ch.qos.logback:logback-classic:1.5.16'
{}"#,
                test_deps
            ),
            format!(
                r#"application {{
    mainClass = '{}.App'
}}"#,
                opts.group_id
            ),
        ),
        TemplateType::Picocli => (
            format!("    id 'application'\n    id 'java'{}", groovy_plugin),
            format!(
                r#"    implementation 'info.picocli:picocli:4.7.6'
    annotationProcessor 'info.picocli:picocli-codegen:4.7.6'
{}"#,
                test_deps
            ),
            format!(
                r#"application {{
    mainClass = '{}.App'
}}"#,
                opts.group_id
            ),
        ),
        TemplateType::MultiModule => unreachable!(),
    };

    let app_section = if app_block.is_empty() {
        String::new()
    } else {
        format!("\n{}\n", app_block)
    };

    let build = format!(
        r#"plugins {{
{}
}}

group = '{}'
version = '0.1.0-SNAPSHOT'

repositories {{
    mavenCentral()
}}

java {{
    toolchain {{
        languageVersion = JavaLanguageVersion.of({})
    }}
}}

tasks.withType(JavaCompile).configureEach {{
    options.encoding = 'UTF-8'
    options.compilerArgs.addAll(['-parameters', '-Xlint:all'])
}}

tasks.withType(Jar).configureEach {{
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
    manifest {{
        attributes('Implementation-Title': project.name, 'Implementation-Version': project.version)
    }}
}}

dependencies {{
{}
}}
{}
{}
"#,
        plugins, opts.group_id, j_ver, deps, app_section, test_cfg
    );

    fs::write(dest.join("build.gradle"), build)?;

    let j_num = j_ver.parse::<u32>().unwrap_or(21);
    let jvmargs = if j_num >= 24 {
        "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8 --enable-native-access=ALL-UNNAMED"
    } else {
        "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8"
    };
    let props = format!(
        "{}\norg.gradle.parallel=true\norg.gradle.caching=true\norg.gradle.configuration-cache=true\n",
        jvmargs
    );
    let _ = fs::write(dest.join("gradle.properties"), props);

    Ok(())
}

fn generate_pom_xml(dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let j_ver = effective_java_version(&opts.java_version);
    let test_deps = test_dependencies_maven(opts.test_framework);
    let spock_plugin = gmavenplus_plugin_maven(opts.test_framework);

    let pom = match opts.template {
        TemplateType::SpringBoot => {
            let sb_ver = opts.framework_version.as_deref().unwrap_or("3.4.0");
            let sb_test = if opts.test_framework != TestFramework::None {
                r#"        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-test</artifactId>
            <scope>test</scope>
        </dependency>"#
            } else {
                ""
            };
            let initializr_entries = initializr_deps_maven(opts.initializr_dependencies.as_deref());
            let spock_extra = if opts.test_framework == TestFramework::Spock {
                format!("\n{}", test_deps)
            } else {
                String::new()
            };
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <parent>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-parent</artifactId>
        <version>{}</version>
        <relativePath/>
    </parent>

    <groupId>{}</groupId>
    <artifactId>{}</artifactId>
    <version>0.1.0-SNAPSHOT</version>

    <properties>
        <java.version>{}</java.version>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <project.reporting.outputEncoding>UTF-8</project.reporting.outputEncoding>
    </properties>

    <dependencies>
{}
{}
{}
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.14.0</version>
                <configuration>
                    <compilerArgs>
                        <arg>-parameters</arg>
                        <arg>-Xlint:all</arg>
                    </compilerArgs>
                </configuration>
            </plugin>
            <plugin>
                <groupId>org.springframework.boot</groupId>
                <artifactId>spring-boot-maven-plugin</artifactId>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-surefire-plugin</artifactId>
                <version>3.5.2</version>
            </plugin>
{}
        </plugins>
    </build>
</project>
"#,
                sb_ver,
                opts.group_id,
                opts.name,
                j_ver,
                initializr_entries,
                sb_test,
                spock_extra,
                spock_plugin
            )
        }
        TemplateType::Quarkus => {
            let q_ver = opts.framework_version.as_deref().unwrap_or("3.15.1");
            let q_test = if opts.test_framework != TestFramework::None {
                r#"        <dependency>
            <groupId>io.quarkus</groupId>
            <artifactId>quarkus-junit5</artifactId>
            <scope>test</scope>
        </dependency>
        <dependency>
            <groupId>io.rest-assured</groupId>
            <artifactId>rest-assured</artifactId>
            <scope>test</scope>
        </dependency>"#
            } else {
                ""
            };
            let spock_extra = if opts.test_framework == TestFramework::Spock {
                format!("\n{}", test_deps)
            } else {
                String::new()
            };
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <groupId>{}</groupId>
    <artifactId>{}</artifactId>
    <version>0.1.0-SNAPSHOT</version>

    <properties>
        <maven.compiler.release>{}</maven.compiler.release>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <project.reporting.outputEncoding>UTF-8</project.reporting.outputEncoding>
        <quarkus.platform.version>{}</quarkus.platform.version>
    </properties>

    <dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>io.quarkus.platform</groupId>
                <artifactId>quarkus-bom</artifactId>
                <version>${{quarkus.platform.version}}</version>
                <type>pom</type>
                <scope>import</scope>
            </dependency>
        </dependencies>
    </dependencyManagement>

    <dependencies>
        <dependency>
            <groupId>io.quarkus</groupId>
            <artifactId>quarkus-resteasy-reactive</artifactId>
        </dependency>
{}
{}
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>io.quarkus.platform</groupId>
                <artifactId>quarkus-maven-plugin</artifactId>
                <version>${{quarkus.platform.version}}</version>
                <extensions>true</extensions>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.14.0</version>
                <configuration>
                    <compilerArgs>
                        <arg>-parameters</arg>
                        <arg>-Xlint:all</arg>
                    </compilerArgs>
                </configuration>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-surefire-plugin</artifactId>
                <version>3.5.2</version>
            </plugin>
{}
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, j_ver, q_ver, q_test, spock_extra, spock_plugin
            )
        }
        TemplateType::Micronaut => {
            let mn_ver = opts.framework_version.as_deref().unwrap_or("4.4.2");
            let mn_test = if opts.test_framework != TestFramework::None {
                r#"        <dependency>
            <groupId>io.micronaut.test</groupId>
            <artifactId>micronaut-test-junit5</artifactId>
            <scope>test</scope>
        </dependency>"#
            } else {
                ""
            };
            let spock_extra = if opts.test_framework == TestFramework::Spock {
                format!("\n{}", test_deps)
            } else {
                String::new()
            };
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <parent>
        <groupId>io.micronaut.platform</groupId>
        <artifactId>micronaut-parent</artifactId>
        <version>{}</version>
    </parent>

    <groupId>{}</groupId>
    <artifactId>{}</artifactId>
    <version>0.1.0-SNAPSHOT</version>

    <properties>
        <jdk.version>{}</jdk.version>
        <release.version>{}</release.version>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <project.reporting.outputEncoding>UTF-8</project.reporting.outputEncoding>
    </properties>

    <dependencies>
        <dependency>
            <groupId>io.micronaut</groupId>
            <artifactId>micronaut-http-server-netty</artifactId>
        </dependency>
{}
{}
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>io.micronaut.maven</groupId>
                <artifactId>micronaut-maven-plugin</artifactId>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.14.0</version>
                <configuration>
                    <compilerArgs>
                        <arg>-parameters</arg>
                        <arg>-Xlint:all</arg>
                    </compilerArgs>
                </configuration>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-surefire-plugin</artifactId>
                <version>3.5.2</version>
            </plugin>
{}
        </plugins>
    </build>
</project>
"#,
                mn_ver, opts.group_id, opts.name, j_ver, j_ver, mn_test, spock_extra, spock_plugin
            )
        }
        TemplateType::Javalin => {
            let compiler_prop = if j_ver == "8" || j_ver == "1.8" {
                "        <maven.compiler.source>8</maven.compiler.source>\n        <maven.compiler.target>8</maven.compiler.target>".to_string()
            } else {
                format!(
                    "        <maven.compiler.release>{}</maven.compiler.release>",
                    j_ver
                )
            };
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <groupId>{}</groupId>
    <artifactId>{}</artifactId>
    <version>0.1.0-SNAPSHOT</version>

    <properties>
{}
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <project.reporting.outputEncoding>UTF-8</project.reporting.outputEncoding>
        <junit.version>5.11.4</junit.version>
    </properties>

    <dependencies>
        <dependency>
            <groupId>io.javalin</groupId>
            <artifactId>javalin</artifactId>
            <version>6.3.0</version>
        </dependency>
        <dependency>
            <groupId>com.fasterxml.jackson.core</groupId>
            <artifactId>jackson-databind</artifactId>
            <version>2.18.2</version>
        </dependency>
        <dependency>
            <groupId>ch.qos.logback</groupId>
            <artifactId>logback-classic</artifactId>
            <version>1.5.16</version>
        </dependency>
{}
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.14.0</version>
                <configuration>
                    <compilerArgs>
                        <arg>-parameters</arg>
                        <arg>-Xlint:all</arg>
                    </compilerArgs>
                </configuration>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-surefire-plugin</artifactId>
                <version>3.5.2</version>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-jar-plugin</artifactId>
                <version>3.4.2</version>
            </plugin>
            <plugin>
                <groupId>org.codehaus.mojo</groupId>
                <artifactId>exec-maven-plugin</artifactId>
                <version>3.5.0</version>
                <configuration>
                    <mainClass>{}.App</mainClass>
                </configuration>
            </plugin>
{}
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, compiler_prop, test_deps, opts.group_id, spock_plugin
            )
        }
        TemplateType::Picocli => {
            let compiler_prop = if j_ver == "8" || j_ver == "1.8" {
                "        <maven.compiler.source>8</maven.compiler.source>\n        <maven.compiler.target>8</maven.compiler.target>".to_string()
            } else {
                format!(
                    "        <maven.compiler.release>{}</maven.compiler.release>",
                    j_ver
                )
            };
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <groupId>{}</groupId>
    <artifactId>{}</artifactId>
    <version>0.1.0-SNAPSHOT</version>

    <properties>
{}
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <project.reporting.outputEncoding>UTF-8</project.reporting.outputEncoding>
        <junit.version>5.11.4</junit.version>
    </properties>

    <dependencies>
        <dependency>
            <groupId>info.picocli</groupId>
            <artifactId>picocli</artifactId>
            <version>4.7.6</version>
        </dependency>
{}
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.14.0</version>
                <configuration>
                    <compilerArgs>
                        <arg>-parameters</arg>
                        <arg>-Xlint:all</arg>
                    </compilerArgs>
                    <annotationProcessorPaths>
                        <path>
                            <groupId>info.picocli</groupId>
                            <artifactId>picocli-codegen</artifactId>
                            <version>4.7.6</version>
                        </path>
                    </annotationProcessorPaths>
                </configuration>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-surefire-plugin</artifactId>
                <version>3.5.2</version>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-jar-plugin</artifactId>
                <version>3.4.2</version>
            </plugin>
            <plugin>
                <groupId>org.codehaus.mojo</groupId>
                <artifactId>exec-maven-plugin</artifactId>
                <version>3.5.0</version>
                <configuration>
                    <mainClass>{}.App</mainClass>
                </configuration>
            </plugin>
{}
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, compiler_prop, test_deps, opts.group_id, spock_plugin
            )
        }
        TemplateType::Grpc => {
            let compiler_prop = if j_ver == "8" || j_ver == "1.8" {
                "        <maven.compiler.source>8</maven.compiler.source>\n        <maven.compiler.target>8</maven.compiler.target>".to_string()
            } else {
                format!(
                    "        <maven.compiler.release>{}</maven.compiler.release>",
                    j_ver
                )
            };
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <groupId>{}</groupId>
    <artifactId>{}</artifactId>
    <version>0.1.0-SNAPSHOT</version>

    <properties>
{}
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <project.reporting.outputEncoding>UTF-8</project.reporting.outputEncoding>
        <junit.version>5.11.4</junit.version>
    </properties>

    <dependencies>
        <dependency>
            <groupId>io.grpc</groupId>
            <artifactId>grpc-netty-shaded</artifactId>
            <version>1.69.0</version>
        </dependency>
        <dependency>
            <groupId>io.grpc</groupId>
            <artifactId>grpc-protobuf</artifactId>
            <version>1.69.0</version>
        </dependency>
        <dependency>
            <groupId>io.grpc</groupId>
            <artifactId>grpc-stub</artifactId>
            <version>1.69.0</version>
        </dependency>
        <dependency>
            <groupId>javax.annotation</groupId>
            <artifactId>javax.annotation-api</artifactId>
            <version>1.3.2</version>
            <scope>provided</scope>
        </dependency>
        <dependency>
            <groupId>ch.qos.logback</groupId>
            <artifactId>logback-classic</artifactId>
            <version>1.5.16</version>
        </dependency>
{}
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.14.0</version>
                <configuration>
                    <compilerArgs>
                        <arg>-parameters</arg>
                        <arg>-Xlint:all</arg>
                    </compilerArgs>
                </configuration>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-surefire-plugin</artifactId>
                <version>3.5.2</version>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-jar-plugin</artifactId>
                <version>3.4.2</version>
            </plugin>
            <plugin>
                <groupId>org.codehaus.mojo</groupId>
                <artifactId>exec-maven-plugin</artifactId>
                <version>3.5.0</version>
                <configuration>
                    <mainClass>{}.App</mainClass>
                </configuration>
            </plugin>
{}
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, compiler_prop, test_deps, opts.group_id, spock_plugin
            )
        }
        _ => {
            let extra_plugin = if opts.template == TemplateType::Cli {
                format!(
                    r#"            <plugin>
                <groupId>org.codehaus.mojo</groupId>
                <artifactId>exec-maven-plugin</artifactId>
                <version>3.5.0</version>
                <configuration>
                    <mainClass>{}.App</mainClass>
                </configuration>
            </plugin>"#,
                    opts.group_id
                )
            } else {
                String::new()
            };

            let compiler_prop = if j_ver == "8" || j_ver == "1.8" {
                "        <maven.compiler.source>8</maven.compiler.source>\n        <maven.compiler.target>8</maven.compiler.target>".to_string()
            } else {
                format!(
                    "        <maven.compiler.release>{}</maven.compiler.release>",
                    j_ver
                )
            };

            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <groupId>{}</groupId>
    <artifactId>{}</artifactId>
    <version>0.1.0-SNAPSHOT</version>

    <properties>
{}
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <project.reporting.outputEncoding>UTF-8</project.reporting.outputEncoding>
        <junit.version>5.11.4</junit.version>
    </properties>

    <dependencies>
{}
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.14.0</version>
                <configuration>
                    <compilerArgs>
                        <arg>-parameters</arg>
                        <arg>-Xlint:all</arg>
                    </compilerArgs>
                </configuration>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-surefire-plugin</artifactId>
                <version>3.5.2</version>
            </plugin>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-jar-plugin</artifactId>
                <version>3.4.2</version>
            </plugin>
{}
{}
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, compiler_prop, test_deps, extra_plugin, spock_plugin
            )
        }
    };

    fs::write(dest.join("pom.xml"), pom)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_generate_cli_gradle_kts() {
        let dir = tempdir().unwrap();
        let opts = NewProjectOptions {
            name: "test-app".into(),
            group_id: "com.example.test".into(),
            template: TemplateType::Cli,
            build_tool: BuildToolChoice::GradleKts,
            java_version: "21".into(),
            distribution: None,
            destination: dir.path().to_path_buf(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::JUnit5,
        };

        generate_project(&opts).await.unwrap();

        assert!(dir.path().join("build.gradle.kts").is_file());
        assert!(dir.path().join(".editorconfig").is_file());
        assert!(dir.path().join(".gitattributes").is_file());
        assert!(dir.path().join("djuke.toml").is_file());
        assert!(dir
            .path()
            .join("src/main/java/com/example/test/App.java")
            .is_file());
        assert!(dir
            .path()
            .join("src/test/java/com/example/test/AppTest.java")
            .is_file());
        let kts = fs::read_to_string(dir.path().join("build.gradle.kts")).unwrap();
        assert!(kts.contains("-parameters"));
        assert!(kts.contains("-Xlint:all"));
        assert!(kts.contains("duplicatesStrategy = DuplicatesStrategy.EXCLUDE"));
        assert!(kts.contains("useJUnitPlatform()"));
    }

    #[tokio::test]
    async fn test_generate_javalin_gradle() {
        let dir = tempdir().unwrap();
        let opts = NewProjectOptions {
            name: "javalin-app".into(),
            group_id: "com.example.rest".into(),
            template: TemplateType::Javalin,
            build_tool: BuildToolChoice::GradleKts,
            java_version: "21".into(),
            distribution: None,
            destination: dir.path().to_path_buf(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::JUnit5,
        };

        generate_project(&opts).await.unwrap();

        let kts = fs::read_to_string(dir.path().join("build.gradle.kts")).unwrap();
        assert!(kts.contains("io.javalin:javalin"));
        assert!(dir.path().join("src/main/resources/logback.xml").is_file());
    }

    #[tokio::test]
    async fn test_generate_picocli_maven() {
        let dir = tempdir().unwrap();
        let opts = NewProjectOptions {
            name: "cli-tool".into(),
            group_id: "com.example.tool".into(),
            template: TemplateType::Picocli,
            build_tool: BuildToolChoice::Maven,
            java_version: "21".into(),
            distribution: None,
            destination: dir.path().to_path_buf(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::JUnit5,
        };

        generate_project(&opts).await.unwrap();

        let pom = fs::read_to_string(dir.path().join("pom.xml")).unwrap();
        assert!(pom.contains("picocli"));
        assert!(pom.contains("picocli-codegen"));
        assert!(pom.contains("<arg>-parameters</arg>"));
        assert!(pom.contains("<arg>-Xlint:all</arg>"));
        assert!(pom.contains("maven-surefire-plugin"));
    }

    #[tokio::test]
    async fn test_generate_multi_module_gradle() {
        let dir = tempdir().unwrap();
        let opts = NewProjectOptions {
            name: "enterprise-system".into(),
            group_id: "com.corp.modular".into(),
            template: TemplateType::MultiModule,
            build_tool: BuildToolChoice::GradleKts,
            java_version: "21".into(),
            distribution: None,
            destination: dir.path().to_path_buf(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::JUnit5,
        };

        generate_project(&opts).await.unwrap();

        assert!(dir.path().join("settings.gradle.kts").is_file());
        assert!(dir.path().join("core/build.gradle.kts").is_file());
        assert!(dir.path().join("api/build.gradle.kts").is_file());
        assert!(dir
            .path()
            .join("core/src/main/java/com/corp/modular/core/DomainService.java")
            .is_file());
        assert!(dir
            .path()
            .join("api/src/main/java/com/corp/modular/api/App.java")
            .is_file());
    }

    #[tokio::test]
    async fn test_generate_multi_module_maven() {
        let dir = tempdir().unwrap();
        let opts = NewProjectOptions {
            name: "corp-maven-mod".into(),
            group_id: "com.corp.mvn".into(),
            template: TemplateType::MultiModule,
            build_tool: BuildToolChoice::Maven,
            java_version: "21".into(),
            distribution: None,
            destination: dir.path().to_path_buf(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::JUnit5,
        };

        generate_project(&opts).await.unwrap();

        assert!(dir.path().join("pom.xml").is_file());
        assert!(dir.path().join("core/pom.xml").is_file());
        assert!(dir.path().join("api/pom.xml").is_file());
        let root_pom = fs::read_to_string(dir.path().join("pom.xml")).unwrap();
        assert!(root_pom.contains("<module>core</module>"));
        assert!(root_pom.contains("<module>api</module>"));
    }

    #[tokio::test]
    async fn test_generate_project_with_skip_test_and_assertj() {
        let dir = tempdir().unwrap();

        // 1. None (Skip test library)
        let none_dir = dir.path().join("none-test");
        let none_opts = NewProjectOptions {
            name: "none-test".into(),
            group_id: "com.example.notest".into(),
            template: TemplateType::Cli,
            build_tool: BuildToolChoice::GradleKts,
            java_version: "21".into(),
            distribution: None,
            destination: none_dir.clone(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::None,
        };
        generate_project(&none_opts).await.unwrap();
        assert!(!none_dir
            .join("src/test/java/com/example/notest/AppTest.java")
            .exists());
        let kts = fs::read_to_string(none_dir.join("build.gradle.kts")).unwrap();
        assert!(kts.contains("enabled = false"));

        // 2. AssertJ
        let assertj_dir = dir.path().join("assertj-test");
        let assertj_opts = NewProjectOptions {
            name: "assertj-test".into(),
            group_id: "com.example.assertj".into(),
            template: TemplateType::Cli,
            build_tool: BuildToolChoice::Maven,
            java_version: "21".into(),
            distribution: None,
            destination: assertj_dir.clone(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::AssertJ,
        };
        generate_project(&assertj_opts).await.unwrap();
        let test_file =
            fs::read_to_string(assertj_dir.join("src/test/java/com/example/assertj/AppTest.java"))
                .unwrap();
        assert!(test_file.contains("import static org.assertj.core.api.Assertions.assertThat;"));
        let pom = fs::read_to_string(assertj_dir.join("pom.xml")).unwrap();
        assert!(pom.contains("assertj-core"));

        // 3. Spock with Gradle Kts
        let spock_gradle_dir = dir.path().join("spock-gradle");
        let spock_gradle_opts = NewProjectOptions {
            name: "spock-gradle".into(),
            group_id: "com.example.spock".into(),
            template: TemplateType::Cli,
            build_tool: BuildToolChoice::GradleKts,
            java_version: "21".into(),
            distribution: None,
            destination: spock_gradle_dir.clone(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::Spock,
        };
        generate_project(&spock_gradle_opts).await.unwrap();
        assert!(spock_gradle_dir
            .join("src/test/groovy/com/example/spock/AppSpec.groovy")
            .is_file());
        let kts = fs::read_to_string(spock_gradle_dir.join("build.gradle.kts")).unwrap();
        assert!(kts.contains("groovy"));
        assert!(kts.contains("spock-core"));

        // 4. Spock with Maven
        let spock_mvn_dir = dir.path().join("spock-mvn");
        let spock_mvn_opts = NewProjectOptions {
            name: "spock-mvn".into(),
            group_id: "com.example.spockmvn".into(),
            template: TemplateType::Cli,
            build_tool: BuildToolChoice::Maven,
            java_version: "21".into(),
            distribution: None,
            destination: spock_mvn_dir.clone(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::Spock,
        };
        generate_project(&spock_mvn_opts).await.unwrap();
        assert!(spock_mvn_dir
            .join("src/test/groovy/com/example/spockmvn/AppSpec.groovy")
            .is_file());
        let pom = fs::read_to_string(spock_mvn_dir.join("pom.xml")).unwrap();
        assert!(pom.contains("spock-core"));
        assert!(pom.contains("gmavenplus-plugin"));

        // 5. TestNG with Maven
        let testng_dir = dir.path().join("testng-mvn");
        let testng_opts = NewProjectOptions {
            name: "testng-mvn".into(),
            group_id: "com.example.testng".into(),
            template: TemplateType::Cli,
            build_tool: BuildToolChoice::Maven,
            java_version: "21".into(),
            distribution: None,
            destination: testng_dir.clone(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::TestNG,
        };
        generate_project(&testng_opts).await.unwrap();
        let test_file =
            fs::read_to_string(testng_dir.join("src/test/java/com/example/testng/AppTest.java"))
                .unwrap();
        assert!(test_file.contains("import org.testng.annotations.Test;"));
        let pom = fs::read_to_string(testng_dir.join("pom.xml")).unwrap();
        assert!(pom.contains("<groupId>org.testng</groupId>"));

        // 6. Spring Boot with custom initializr dependencies
        let spring_dir = dir.path().join("spring-deps");
        let spring_opts = NewProjectOptions {
            name: "spring-deps".into(),
            group_id: "com.example.springdeps".into(),
            template: TemplateType::SpringBoot,
            build_tool: BuildToolChoice::GradleKts,
            java_version: "21".into(),
            distribution: None,
            destination: spring_dir.clone(),
            framework_version: None,
            initializr_dependencies: Some(vec![
                "web".into(),
                "data-jpa".into(),
                "postgresql".into(),
                "security".into(),
            ]),
            skip_initializr: true,
            test_framework: TestFramework::JUnit5,
        };
        generate_project(&spring_opts).await.unwrap();
        let kts = fs::read_to_string(spring_dir.join("build.gradle.kts")).unwrap();
        assert!(kts.contains("spring-boot-starter-data-jpa"));
        assert!(kts.contains("postgresql"));
        assert!(kts.contains("spring-boot-starter-security"));

        // 7. Gradle Groovy for Spring Boot
        let groovy_sb_dir = dir.path().join("groovy-sb");
        let groovy_sb_opts = NewProjectOptions {
            name: "groovy-sb".into(),
            group_id: "com.example.groovysb".into(),
            template: TemplateType::SpringBoot,
            build_tool: BuildToolChoice::GradleGroovy,
            java_version: "21".into(),
            distribution: None,
            destination: groovy_sb_dir.clone(),
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: true,
            test_framework: TestFramework::JUnit5,
        };
        generate_project(&groovy_sb_opts).await.unwrap();
        let build_gradle = fs::read_to_string(groovy_sb_dir.join("build.gradle")).unwrap();
        assert!(build_gradle.contains("id 'org.springframework.boot'"));
        assert!(build_gradle.contains("spring-boot-starter-web"));
    }
}
