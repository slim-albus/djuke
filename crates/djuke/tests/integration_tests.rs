use std::fs;
use tempfile::tempdir;

use djuke_config::{detect_project_markers, resolve_project_config};
use djuke_core::{BuildTool, JdkDistribution};
use djuke_platform::DjukePaths;

#[test]
fn test_end_to_end_project_detection_and_resolution() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create a mock Maven project with pom.xml specifying Java 17
    let pom = root.join("pom.xml");
    fs::write(
        &pom,
        r#"
<project>
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>demo</artifactId>
  <version>1.0.0</version>
  <properties>
    <maven.compiler.target>17</maven.compiler.target>
  </properties>
</project>
"#,
    )
    .unwrap();

    // Create mock mvnw.cmd for Windows and mvnw for Unix
    let mvnw_cmd = root.join("mvnw.cmd");
    fs::write(&mvnw_cmd, "@echo off\necho Maven Wrapper\n").unwrap();

    let markers = detect_project_markers(root);
    assert_eq!(markers.build_tool, Some(BuildTool::Maven));
    assert!(markers.has_maven_wrapper);
    assert_eq!(markers.inferred_java_version, Some("17".to_string()));

    let paths = DjukePaths {
        root: root.join(".djuke-home"),
    };

    let resolved = resolve_project_config(root, None, &paths);
    assert_eq!(resolved.jdk_spec.version, "17");
    assert_eq!(resolved.jdk_spec.distribution, JdkDistribution::Temurin);
    assert_eq!(resolved.build_tool, Some(BuildTool::Maven));
    assert!(resolved.tasks.contains_key("build"));
    assert_eq!(resolved.tasks.get("build").unwrap(), "clean package");
}

#[test]
fn test_end_to_end_gradle_project_with_djuke_override() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let gradlew_bat = root.join("gradlew.bat");
    fs::write(&gradlew_bat, "@echo off\necho Gradle Wrapper\n").unwrap();

    // Custom djuke.toml
    let djuke_toml = root.join("djuke.toml");
    fs::write(
        &djuke_toml,
        r#"
[project]
name = "custom-gradle-app"

[java]
version = "21"
distribution = "corretto"

[tasks]
build = "assemble"
custom = "echo hello"
"#,
    )
    .unwrap();

    let paths = DjukePaths {
        root: root.join(".djuke-home"),
    };

    let resolved = resolve_project_config(root, None, &paths);
    assert_eq!(resolved.jdk_spec.version, "21");
    assert_eq!(resolved.jdk_spec.distribution, JdkDistribution::Corretto);
    assert_eq!(resolved.build_tool, Some(BuildTool::Gradle));
    assert_eq!(resolved.tasks.get("build").unwrap(), "assemble");
    assert_eq!(resolved.tasks.get("custom").unwrap(), "echo hello");
}

#[tokio::test]
async fn test_end_to_end_bare_repo_with_sdkmanrc() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let sdkmanrc = root.join(".sdkmanrc");
    fs::write(&sdkmanrc, "java=21.0.4-tem\n").unwrap();

    let build_kts = root.join("build.gradle.kts");
    fs::write(&build_kts, "// gradle build script\n").unwrap();

    let paths = DjukePaths {
        root: root.join(".djuke-home"),
    };

    let resolved = resolve_project_config(root, None, &paths);
    assert_eq!(resolved.jdk_spec.version, "21");
    assert_eq!(resolved.build_tool, Some(BuildTool::Gradle));
}

#[tokio::test]
async fn test_end_to_end_mock_wrapper_execution_and_env_injection() {
    use djuke_build::{execute_task, TaskExecution};
    use std::collections::HashMap;

    let dir = tempdir().unwrap();
    let root = dir.path();
    let mock_java_home = root.join("mock-jdk-21");
    fs::create_dir_all(mock_java_home.join("bin")).unwrap();

    // Create wrapper script that writes JAVA_HOME to a file
    let output_file = root.join("out.txt");
    let (wrapper_name, wrapper_content) = if cfg!(windows) {
        (
            "gradlew.bat",
            format!(
                "@echo off\necho %JAVA_HOME% > \"{}\"\nexit 0\n",
                output_file.display()
            ),
        )
    } else {
        (
            "gradlew",
            format!(
                "#!/bin/sh\necho \"$JAVA_HOME\" > \"{}\"\nexit 0\n",
                output_file.display()
            ),
        )
    };

    let wrapper_path = root.join(wrapper_name);
    fs::write(&wrapper_path, wrapper_content).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&wrapper_path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let extra_args = vec!["--info".to_string()];
    let extra_env = HashMap::new();

    let status = execute_task(TaskExecution {
        project_root: root,
        java_home: &mock_java_home,
        tool: BuildTool::Gradle,
        task_name: "build",
        task_command: "build",
        extra_args: &extra_args,
        extra_env: &extra_env,
        paths: None,
    })
    .await
    .unwrap();

    assert!(status.success());
    assert!(output_file.is_file());
    let recorded_java_home = fs::read_to_string(output_file).unwrap();
    assert!(recorded_java_home.contains("mock-jdk-21"));
}

#[test]
fn test_end_to_end_scaffold_and_maven_dependency_lifecycle() {
    use djuke_pm::{
        add_maven_dependency, remove_maven_dependency, DependencyScope, DependencySpec,
    };
    use djuke_templates::{generate_project, BuildToolChoice, NewProjectOptions, TemplateType};

    let dir = tempdir().unwrap();
    let project_dir = dir.path().join("my-spring-app");

    let opts = NewProjectOptions {
        name: "my-spring-app".to_string(),
        group_id: "com.example.springapp".to_string(),
        template: TemplateType::SpringBoot,
        build_tool: BuildToolChoice::Maven,
        java_version: "21".to_string(),
        distribution: None,
        destination: project_dir.clone(),
        framework_version: None,
    };

    generate_project(&opts).unwrap();

    let pom_file = project_dir.join("pom.xml");
    assert!(pom_file.is_file());
    assert!(project_dir.join("djuke.toml").is_file());

    let initial_pom = fs::read_to_string(&pom_file).unwrap();
    assert!(initial_pom.contains("<artifactId>my-spring-app</artifactId>"));

    // Add postgresql dependency
    let dep = DependencySpec {
        group_id: "org.postgresql".to_string(),
        artifact_id: "postgresql".to_string(),
        version: Some("42.7.2".to_string()),
        scope: DependencyScope::RuntimeOnly,
    };

    add_maven_dependency(&pom_file, &dep).unwrap();
    let pom_after_add = fs::read_to_string(&pom_file).unwrap();
    assert!(pom_after_add.contains("<groupId>org.postgresql</groupId>"));
    assert!(pom_after_add.contains("<artifactId>postgresql</artifactId>"));
    assert!(pom_after_add.contains("<version>42.7.2</version>"));
    assert!(pom_after_add.contains("<scope>runtime</scope>"));

    // Remove postgresql dependency
    remove_maven_dependency(&pom_file, "org.postgresql", "postgresql").unwrap();
    let pom_after_remove = fs::read_to_string(&pom_file).unwrap();
    assert!(!pom_after_remove.contains("<groupId>org.postgresql</groupId>"));
    assert!(!pom_after_remove.contains("<artifactId>postgresql</artifactId>"));
}

#[test]
fn test_end_to_end_scaffold_and_gradle_kts_dependency_lifecycle() {
    use djuke_pm::{
        add_gradle_dependency, remove_gradle_dependency, DependencyScope, DependencySpec,
    };
    use djuke_templates::{generate_project, BuildToolChoice, NewProjectOptions, TemplateType};

    let dir = tempdir().unwrap();
    let project_dir = dir.path().join("my-cli-app");

    let opts = NewProjectOptions {
        name: "my-cli-app".to_string(),
        group_id: "com.example.cliapp".to_string(),
        template: TemplateType::Cli,
        build_tool: BuildToolChoice::GradleKts,
        java_version: "21".to_string(),
        distribution: None,
        destination: project_dir.clone(),
        framework_version: None,
    };

    generate_project(&opts).unwrap();

    let build_file = project_dir.join("build.gradle.kts");
    assert!(build_file.is_file());

    // Add Jackson dependency
    let dep = DependencySpec {
        group_id: "com.fasterxml.jackson.core".to_string(),
        artifact_id: "jackson-databind".to_string(),
        version: Some("2.17.0".to_string()),
        scope: DependencyScope::Implementation,
    };

    add_gradle_dependency(&build_file, &dep).unwrap();
    let build_after_add = fs::read_to_string(&build_file).unwrap();
    assert!(build_after_add
        .contains("implementation(\"com.fasterxml.jackson.core:jackson-databind:2.17.0\")"));

    // Remove Jackson dependency
    remove_gradle_dependency(
        &build_file,
        "com.fasterxml.jackson.core",
        "jackson-databind",
    )
    .unwrap();
    let build_after_remove = fs::read_to_string(&build_file).unwrap();
    assert!(!build_after_remove.contains("com.fasterxml.jackson.core:jackson-databind"));
}

#[test]
fn test_end_to_end_project_init_in_place_and_dependency_tree() {
    use djuke_pm::{extract_project_dependencies, render_dependency_tree};
    use djuke_templates::{generate_project, BuildToolChoice, NewProjectOptions, TemplateType};

    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let opts = NewProjectOptions {
        name: "in-place-app".to_string(),
        group_id: "com.example.inplace".to_string(),
        template: TemplateType::SpringBoot,
        build_tool: BuildToolChoice::Maven,
        java_version: "21".to_string(),
        distribution: None,
        destination: root.clone(),
        framework_version: None,
    };

    generate_project(&opts).unwrap();
    assert!(root.join("pom.xml").is_file());
    assert!(root.join("djuke.toml").is_file());

    let deps = extract_project_dependencies(&root).unwrap();
    assert!(!deps.is_empty());
    assert!(deps
        .iter()
        .any(|d| d.artifact_id == "spring-boot-starter-web"));

    let tree = render_dependency_tree("in-place-app", &deps);
    assert!(tree.contains("in-place-app"));
    assert!(tree.contains("spring-boot-starter-web"));
}

#[test]
fn test_end_to_end_framework_dev_and_run_task_resolution() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let build_kts = root.join("build.gradle.kts");
    fs::write(
        &build_kts,
        r#"
plugins {
    id("org.springframework.boot") version "3.3.4"
    id("io.spring.dependency-management") version "1.1.6"
    kotlin("jvm") version "1.9.25"
}
dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web")
}
"#,
    )
    .unwrap();

    let paths = DjukePaths {
        root: root.join(".djuke-home"),
    };

    let resolved = resolve_project_config(root, None, &paths);
    assert_eq!(resolved.tasks.get("dev").unwrap(), "bootRun");
    assert_eq!(resolved.tasks.get("run").unwrap(), "bootRun");
    assert_eq!(resolved.tasks.get("build").unwrap(), "build");
    assert_eq!(resolved.tasks.get("test").unwrap(), "test");
}

#[tokio::test]
async fn test_doctor_reports_unready_when_no_jdk_installed() {
    use djuke_doctor::run_doctor_checks;

    let dir = tempdir().unwrap();
    let paths = DjukePaths {
        root: dir.path().join(".djuke-home"),
    };
    paths.ensure_dirs().unwrap();

    let report = run_doctor_checks(dir.path(), &paths).await.unwrap();
    assert!(
        !report.overall_status,
        "overall status must be false when no JDK is installed"
    );

    let jdk_check = report
        .checks
        .iter()
        .find(|c| c.name == "Installed JDKs")
        .expect("Installed JDKs check must exist");
    assert!(
        !jdk_check.status,
        "Installed JDKs check must fail when store is empty"
    );
    assert!(jdk_check
        .hint
        .as_ref()
        .unwrap()
        .contains("djuke java install"));
}

#[test]
fn test_jdk_distribution_support_all_10_distros() {
    use djuke_core::JdkDistribution;
    use std::str::FromStr;

    let distros = [
        ("temurin", JdkDistribution::Temurin),
        ("openjdk", JdkDistribution::OpenJdk),
        ("corretto", JdkDistribution::Corretto),
        ("zulu", JdkDistribution::Zulu),
        ("microsoft", JdkDistribution::Microsoft),
        ("semeru", JdkDistribution::Semeru),
        ("graalvm", JdkDistribution::Graalvm),
        ("liberica", JdkDistribution::Liberica),
        ("oracle", JdkDistribution::Oracle),
        ("sapmachine", JdkDistribution::Sapmachine),
    ];

    for (name, expected) in distros {
        let parsed = JdkDistribution::from_str(name).unwrap();
        assert_eq!(parsed, expected);
    }
}

#[test]
fn test_template_generation_with_custom_distro_and_latest_java() {
    use djuke_core::JdkDistribution;
    use djuke_templates::{generate_project, BuildToolChoice, NewProjectOptions, TemplateType};

    let dir = tempdir().unwrap();
    let project_dir = dir.path().join("liberica-app");

    let opts = NewProjectOptions {
        name: "liberica-app".to_string(),
        group_id: "com.example.liberica".to_string(),
        template: TemplateType::Cli,
        build_tool: BuildToolChoice::GradleKts,
        java_version: "latest".to_string(),
        distribution: Some(JdkDistribution::Liberica),
        destination: project_dir.clone(),
        framework_version: None,
    };

    generate_project(&opts).unwrap();

    let djuke_toml = fs::read_to_string(project_dir.join("djuke.toml")).unwrap();
    assert!(djuke_toml.contains("distribution = \"liberica\""));
    assert!(djuke_toml.contains("version = \"latest\""));

    let build_kts = fs::read_to_string(project_dir.join("build.gradle.kts")).unwrap();
    assert!(build_kts.contains("JavaLanguageVersion.of(25)"));
}

#[test]
fn test_gradle_and_maven_wrapper_auto_generation_in_project_without_wrapper() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Verify initially no wrapper
    assert!(!root.join("gradlew").exists());
    assert!(!root.join("gradlew.bat").exists());
    assert!(!root.join("mvnw").exists());
    assert!(!root.join("mvnw.cmd").exists());

    // Auto-generate Gradle wrapper directly from embedded assets
    djuke_build::create_gradle_wrapper(root, None).unwrap();
    assert!(root.join("gradlew").exists());
    assert!(root.join("gradlew.bat").exists());
    assert!(root
        .join("gradle")
        .join("wrapper")
        .join("gradle-wrapper.jar")
        .exists());
    assert!(root
        .join("gradle")
        .join("wrapper")
        .join("gradle-wrapper.properties")
        .exists());

    // Auto-generate Maven wrapper directly from embedded assets
    let mvn_dir = tempdir().unwrap();
    let mvn_root = mvn_dir.path();
    djuke_build::create_maven_wrapper(mvn_root, None).unwrap();
    assert!(mvn_root.join("mvnw").exists());
    assert!(mvn_root.join("mvnw.cmd").exists());
    assert!(mvn_root
        .join(".mvn")
        .join("wrapper")
        .join("maven-wrapper.jar")
        .exists());
    assert!(mvn_root
        .join(".mvn")
        .join("wrapper")
        .join("maven-wrapper.properties")
        .exists());
}

#[test]
fn test_gradle_wrapper_compatible_version_selection_for_java_27_and_modern_runtimes() {
    use djuke_build::compatible_gradle_version_for_java;

    // Verify version selection for various Java versions
    assert_eq!(compatible_gradle_version_for_java("27"), "9.9.2");
    assert_eq!(compatible_gradle_version_for_java("25"), "9.9.2");
    assert_eq!(compatible_gradle_version_for_java("23"), "9.9.2");
    assert_eq!(compatible_gradle_version_for_java("21"), "9.9.2");
    assert_eq!(compatible_gradle_version_for_java("17"), "9.9.2");
    assert_eq!(compatible_gradle_version_for_java("11"), "8.14.8");
    assert_eq!(compatible_gradle_version_for_java("8"), "8.14.8");

    // Verify wrapper generation writes exact Gradle 9.9.2 properties and gradle.properties
    let dir = tempdir().unwrap();
    let root = dir.path();

    let ver = compatible_gradle_version_for_java("27");
    djuke_build::create_gradle_wrapper(root, Some(ver)).unwrap();

    let props_path = root
        .join("gradle")
        .join("wrapper")
        .join("gradle-wrapper.properties");
    assert!(props_path.is_file());
    let props = fs::read_to_string(&props_path).unwrap();
    assert!(props.contains("gradle-9.9.2-bin.zip"));

    // Verify gradle.properties is created with modern jvmargs and performance tuning
    let gradle_props_path = root.join("gradle.properties");
    assert!(gradle_props_path.is_file());
    let gradle_props = fs::read_to_string(&gradle_props_path).unwrap();
    assert!(gradle_props.contains("--enable-native-access=ALL-UNNAMED"));
    assert!(gradle_props.contains("org.gradle.parallel=true"));
    assert!(gradle_props.contains("org.gradle.caching=true"));
}

#[tokio::test]
async fn test_dynamic_wrapper_version_resolution_and_caching() {
    use djuke_build::{resolve_dynamic_gradle_version, resolve_dynamic_maven_version};

    let dir = tempdir().unwrap();
    let paths = DjukePaths {
        root: dir.path().join(".djuke-home"),
    };
    paths.ensure_dirs().unwrap();

    // 1. Resolve Gradle dynamically online
    let g_ver = resolve_dynamic_gradle_version("21", Some(&paths), true).await;
    assert!(!g_ver.is_empty());
    assert!(g_ver.starts_with('9') || g_ver.starts_with('8'));

    // 2. Verify cache file was created in ~/.djuke-home/cache/tool_versions.json
    let cache_file = paths.cache_dir().join("tool_versions.json");
    assert!(cache_file.is_file());

    // 3. Resolve Maven dynamically online
    let m_ver = resolve_dynamic_maven_version(Some(&paths), true).await;
    assert!(!m_ver.is_empty());
    assert!(m_ver.starts_with("3.9"));

    // 4. Verify offline fallback resolves instantly without network
    let offline_g = resolve_dynamic_gradle_version("21", Some(&paths), false).await;
    assert_eq!(offline_g, g_ver);
}

#[test]
fn test_template_generation_with_custom_dynamic_framework_version() {
    use djuke_templates::{generate_project, BuildToolChoice, NewProjectOptions, TemplateType};

    let dir = tempdir().unwrap();
    let project_dir = dir.path().join("spring-custom-ver");

    let opts = NewProjectOptions {
        name: "spring-custom-ver".to_string(),
        group_id: "com.example.spring".to_string(),
        template: TemplateType::SpringBoot,
        build_tool: BuildToolChoice::GradleKts,
        java_version: "21".to_string(),
        distribution: None,
        destination: project_dir.clone(),
        framework_version: Some("3.5.3".to_string()),
    };

    generate_project(&opts).unwrap();

    let build_kts = fs::read_to_string(project_dir.join("build.gradle.kts")).unwrap();
    assert!(build_kts.contains("3.5.3"));
}
