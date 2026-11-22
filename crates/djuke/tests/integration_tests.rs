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
