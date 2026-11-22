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
