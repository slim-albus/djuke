use std::fs;
use std::path::Path;

use djuke_core::Result;

use crate::model::{BuildToolChoice, NewProjectOptions, TemplateType};

pub fn generate_project(opts: &NewProjectOptions) -> Result<()> {
    let dest = &opts.destination;
    fs::create_dir_all(dest)?;

    let pkg_path = opts.group_id.replace('.', "/");
    let main_src = dest.join("src").join("main").join("java").join(&pkg_path);
    let test_src = dest.join("src").join("test").join("java").join(&pkg_path);
    let resources = dest.join("src").join("main").join("resources");

    fs::create_dir_all(&main_src)?;
    fs::create_dir_all(&test_src)?;
    fs::create_dir_all(&resources)?;

    // 1. Generate source files based on template
    match opts.template {
        TemplateType::Cli => {
            generate_cli_source(&main_src, &test_src, opts)?;
        }
        TemplateType::Lib => {
            generate_lib_source(&main_src, &test_src, opts)?;
        }
        TemplateType::SpringBoot => {
            generate_spring_source(&main_src, &test_src, opts)?;
        }
        TemplateType::Quarkus => {
            generate_quarkus_source(&main_src, &test_src, opts)?;
        }
        TemplateType::Micronaut => {
            generate_micronaut_source(&main_src, &test_src, opts)?;
        }
    }

    // 2. Generate build files based on build tool
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

    // 3. Generate .gitignore
    let gitignore_content = r#"# Build outputs
/build/
/target/
/.gradle/
/.mvn/timing/

# IDE files
/.idea/
/*.iml
/.vscode/
/.settings/
/.project/
/.classpath/

# OS files
.DS_Store
Thumbs.db
"#;
    fs::write(dest.join(".gitignore"), gitignore_content)?;

    // 4. Generate djuke.toml
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

    // 5. Generate README.md
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

fn generate_cli_source(main: &Path, test: &Path, opts: &NewProjectOptions) -> Result<()> {
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

    let test_java = format!(
        r#"package {};

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class AppTest {{
    @Test
    void appHasAGreeting() {{
        App classUnderTest = new App();
        assertNotNull(classUnderTest.getGreeting(), "app should have a greeting");
    }}
}}
"#,
        opts.group_id
    );
    fs::write(test.join("AppTest.java"), test_java)?;
    Ok(())
}

fn generate_lib_source(main: &Path, test: &Path, opts: &NewProjectOptions) -> Result<()> {
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

    let test_java = format!(
        r#"package {};

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class LibraryTest {{
    @Test
    void someLibraryMethodReturnsTrue() {{
        Library classUnderTest = new Library();
        assertTrue(classUnderTest.someLibraryMethod(), "someLibraryMethod should return 'true'");
    }}
}}
"#,
        opts.group_id
    );
    fs::write(test.join("LibraryTest.java"), test_java)?;
    Ok(())
}

fn generate_spring_source(main: &Path, test: &Path, opts: &NewProjectOptions) -> Result<()> {
    let app_java = format!(
        r#"package {};

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;

@SpringBootApplication
@RestController
public class Application {{
    public static void main(String[] args) {{
        SpringApplication.run(Application.class, args);
    }}

    @GetMapping("/")
    public String index() {{
        return "Greetings from Spring Boot + djuke!";
    }}
}}
"#,
        opts.group_id
    );
    fs::write(main.join("Application.java"), app_java)?;

    let test_java = format!(
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
    );
    fs::write(test.join("ApplicationTests.java"), test_java)?;
    Ok(())
}

fn generate_quarkus_source(main: &Path, test: &Path, opts: &NewProjectOptions) -> Result<()> {
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

    let test_java = format!(
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
    );
    fs::write(test.join("GreetingResourceTest.java"), test_java)?;
    Ok(())
}

fn generate_micronaut_source(main: &Path, test: &Path, opts: &NewProjectOptions) -> Result<()> {
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

    let test_java = format!(
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
    );
    fs::write(test.join("ApplicationTest.java"), test_java)?;
    Ok(())
}

fn effective_java_version(v: &str) -> &str {
    if v.eq_ignore_ascii_case("latest") {
        "23"
    } else if v.eq_ignore_ascii_case("lts") {
        "21"
    } else {
        v
    }
}

fn generate_gradle_kts(dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let settings = format!("rootProject.name = \"{}\"\n", opts.name);
    fs::write(dest.join("settings.gradle.kts"), settings)?;

    let j_ver = effective_java_version(&opts.java_version);
    let (plugins, deps) = match opts.template {
        TemplateType::Cli => (
            "    application\n    `java-library`",
            format!(
                r#"    testImplementation(platform("org.junit:junit-bom:5.10.2"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")

application {{
    mainClass.set("{}.App")
}}"#,
                opts.group_id
            ),
        ),
        TemplateType::Lib => (
            "    `java-library`\n    `maven-publish`",
            r#"    testImplementation(platform("org.junit:junit-bom:5.10.2"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")"#
                .to_string(),
        ),
        TemplateType::SpringBoot => (
            r#"    java
    id("org.springframework.boot") version "3.3.4"
    id("io.spring.dependency-management") version "1.1.6""#,
            r#"    implementation("org.springframework.boot:spring-boot-starter-web")
    testImplementation("org.springframework.boot:spring-boot-starter-test")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")"#
                .to_string(),
        ),
        TemplateType::Quarkus => (
            r#"    java
    id("io.quarkus") version "3.15.1""#,
            r#"    implementation(enforcedPlatform("io.quarkus.platform:quarkus-bom:3.15.1"))
    implementation("io.quarkus:quarkus-resteasy-reactive")
    testImplementation("io.quarkus:quarkus-junit5")
    testImplementation("io.rest-assured:rest-assured")"#
                .to_string(),
        ),
        TemplateType::Micronaut => (
            r#"    java
    id("io.micronaut.application") version "4.4.2""#,
            r#"    annotationProcessor("io.micronaut:micronaut-inject-java")
    implementation("io.micronaut:micronaut-http-server-netty")
    runtimeOnly("ch.qos.logback:logback-classic")
    testImplementation("io.micronaut.test:micronaut-test-junit5")"#
                .to_string(),
        ),
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

dependencies {{
{}
}}

tasks.named<Test>("test") {{
    useJUnitPlatform()
}}
"#,
        plugins, opts.group_id, j_ver, deps
    );

    fs::write(dest.join("build.gradle.kts"), build)?;
    Ok(())
}

fn generate_gradle_groovy(dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let settings = format!("rootProject.name = '{}'\n", opts.name);
    fs::write(dest.join("settings.gradle"), settings)?;

    let j_ver = effective_java_version(&opts.java_version);
    let build = format!(
        r#"plugins {{
    id 'application'
    id 'java'
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

dependencies {{
    testImplementation platform('org.junit:junit-bom:5.10.2')
    testImplementation 'org.junit.jupiter:junit-jupiter'
    testRuntimeOnly 'org.junit.platform:junit-platform-launcher'
}}

application {{
    mainClass = '{}.App'
}}

test {{
    useJUnitPlatform()
}}
"#,
        opts.group_id, j_ver, opts.group_id
    );

    fs::write(dest.join("build.gradle"), build)?;
    Ok(())
}

fn generate_pom_xml(dest: &Path, opts: &NewProjectOptions) -> Result<()> {
    let j_ver = effective_java_version(&opts.java_version);
    let pom = match opts.template {
        TemplateType::SpringBoot => {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <parent>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-parent</artifactId>
        <version>3.3.4</version>
        <relativePath/>
    </parent>

    <groupId>{}</groupId>
    <artifactId>{}</artifactId>
    <version>0.1.0-SNAPSHOT</version>

    <properties>
        <java.version>{}</java.version>
    </properties>

    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-test</artifactId>
            <scope>test</scope>
        </dependency>
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>org.springframework.boot</groupId>
                <artifactId>spring-boot-maven-plugin</artifactId>
            </plugin>
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, j_ver
            )
        }
        TemplateType::Quarkus => {
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
        <quarkus.platform.version>3.15.1</quarkus.platform.version>
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
        <dependency>
            <groupId>io.quarkus</groupId>
            <artifactId>quarkus-junit5</artifactId>
            <scope>test</scope>
        </dependency>
        <dependency>
            <groupId>io.rest-assured</groupId>
            <artifactId>rest-assured</artifactId>
            <scope>test</scope>
        </dependency>
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>io.quarkus.platform</groupId>
                <artifactId>quarkus-maven-plugin</artifactId>
                <version>${{quarkus.platform.version}}</version>
                <extensions>true</extensions>
            </plugin>
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, j_ver
            )
        }
        TemplateType::Micronaut => {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <parent>
        <groupId>io.micronaut.platform</groupId>
        <artifactId>micronaut-parent</artifactId>
        <version>4.4.2</version>
    </parent>

    <groupId>{}</groupId>
    <artifactId>{}</artifactId>
    <version>0.1.0-SNAPSHOT</version>

    <properties>
        <jdk.version>{}</jdk.version>
        <release.version>{}</release.version>
    </properties>

    <dependencies>
        <dependency>
            <groupId>io.micronaut</groupId>
            <artifactId>micronaut-http-server-netty</artifactId>
        </dependency>
        <dependency>
            <groupId>io.micronaut.test</groupId>
            <artifactId>micronaut-test-junit5</artifactId>
            <scope>test</scope>
        </dependency>
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>io.micronaut.maven</groupId>
                <artifactId>micronaut-maven-plugin</artifactId>
            </plugin>
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, j_ver, j_ver
            )
        }
        _ => {
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
        <maven.compiler.source>{}</maven.compiler.source>
        <maven.compiler.target>{}</maven.compiler.target>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <junit.version>5.10.2</junit.version>
    </properties>

    <dependencies>
        <dependency>
            <groupId>org.junit.jupiter</groupId>
            <artifactId>junit-jupiter</artifactId>
            <version>${{junit.version}}</version>
            <scope>test</scope>
        </dependency>
    </dependencies>

    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-surefire-plugin</artifactId>
                <version>3.2.5</version>
            </plugin>
        </plugins>
    </build>
</project>
"#,
                opts.group_id, opts.name, j_ver, j_ver
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

    #[test]
    fn test_generate_cli_gradle_kts() {
        let dir = tempdir().unwrap();
        let opts = NewProjectOptions {
            name: "test-app".into(),
            group_id: "com.example.test".into(),
            template: TemplateType::Cli,
            build_tool: BuildToolChoice::GradleKts,
            java_version: "21".into(),
            distribution: None,
            destination: dir.path().to_path_buf(),
        };

        generate_project(&opts).unwrap();

        assert!(dir.path().join("build.gradle.kts").is_file());
        assert!(dir.path().join("settings.gradle.kts").is_file());
        assert!(dir.path().join("djuke.toml").is_file());
        assert!(dir
            .path()
            .join("src/main/java/com/example/test/App.java")
            .is_file());
        assert!(dir
            .path()
            .join("src/test/java/com/example/test/AppTest.java")
            .is_file());
    }

    #[test]
    fn test_generate_spring_boot_maven() {
        let dir = tempdir().unwrap();
        let opts = NewProjectOptions {
            name: "my-spring".into(),
            group_id: "com.acme.service".into(),
            template: TemplateType::SpringBoot,
            build_tool: BuildToolChoice::Maven,
            java_version: "21".into(),
            distribution: None,
            destination: dir.path().to_path_buf(),
        };

        generate_project(&opts).unwrap();

        assert!(dir.path().join("pom.xml").is_file());
        assert!(dir.path().join("djuke.toml").is_file());
        assert!(dir
            .path()
            .join("src/main/java/com/acme/service/Application.java")
            .is_file());
    }
}
