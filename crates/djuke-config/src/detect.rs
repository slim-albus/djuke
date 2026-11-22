use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

use djuke_core::BuildTool;

#[derive(Debug, Clone, Default)]
pub struct ProjectMarkers {
    pub root: PathBuf,
    pub build_tool: Option<BuildTool>,
    pub has_gradle_wrapper: bool,
    pub has_maven_wrapper: bool,
    pub inferred_java_version: Option<String>,
}

pub fn detect_project_markers(start_dir: &Path) -> ProjectMarkers {
    let mut current = start_dir.to_path_buf();
    let mut markers = ProjectMarkers {
        root: start_dir.to_path_buf(),
        ..Default::default()
    };

    loop {
        // 1. Check wrappers
        let gradlew = current.join("gradlew");
        let gradlew_bat = current.join("gradlew.bat");
        if gradlew.exists() || gradlew_bat.exists() {
            markers.has_gradle_wrapper = true;
            markers.build_tool = Some(BuildTool::Gradle);
        }

        let mvnw = current.join("mvnw");
        let mvnw_cmd = current.join("mvnw.cmd");
        if mvnw.exists() || mvnw_cmd.exists() {
            markers.has_maven_wrapper = true;
            if markers.build_tool.is_none() {
                markers.build_tool = Some(BuildTool::Maven);
            }
        }

        // 2. Check build files
        let build_gradle = current.join("build.gradle");
        let build_gradle_kts = current.join("build.gradle.kts");
        if build_gradle.is_file() || build_gradle_kts.is_file() {
            if markers.build_tool.is_none() {
                markers.build_tool = Some(BuildTool::Gradle);
            }
            if markers.inferred_java_version.is_none() {
                if let Some(ver) = parse_gradle_java_version(&build_gradle)
                    .or_else(|| parse_gradle_java_version(&build_gradle_kts))
                {
                    markers.inferred_java_version = Some(ver);
                }
            }
        }

        let pom_xml = current.join("pom.xml");
        if pom_xml.is_file() {
            if markers.build_tool.is_none() {
                markers.build_tool = Some(BuildTool::Maven);
            }
            if markers.inferred_java_version.is_none() {
                if let Some(ver) = parse_pom_java_version(&pom_xml) {
                    markers.inferred_java_version = Some(ver);
                }
            }
        }

        // 3. Check version files (.java-version, .sdkmanrc)
        let java_version_file = current.join(".java-version");
        if java_version_file.is_file() && markers.inferred_java_version.is_none() {
            if let Ok(content) = fs::read_to_string(&java_version_file) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    markers.inferred_java_version = Some(normalize_version_string(trimmed));
                }
            }
        }

        let sdkmanrc = current.join(".sdkmanrc");
        if sdkmanrc.is_file() && markers.inferred_java_version.is_none() {
            if let Ok(content) = fs::read_to_string(&sdkmanrc) {
                for line in content.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        if k.trim() == "java" {
                            markers.inferred_java_version =
                                Some(normalize_sdkman_version(v.trim()));
                            break;
                        }
                    }
                }
            }
        }

        // If we found any markers, consider this the root
        if markers.build_tool.is_some() || markers.inferred_java_version.is_some() {
            markers.root = current;
            break;
        }

        // Walk up directory tree
        if !current.pop() {
            break;
        }
    }

    markers
}

fn parse_gradle_java_version(file: &Path) -> Option<String> {
    let content = fs::read_to_string(file).ok()?;
    // match: jvmToolchain(21) or jvmToolchain(JavaLanguageVersion.of(21))
    let toolchain_re =
        Regex::new(r"jvmToolchain\s*\(\s*(?:JavaLanguageVersion\.of\s*\(\s*)?(\d+)").ok()?;
    if let Some(caps) = toolchain_re.captures(&content) {
        if let Some(m) = caps.get(1) {
            return Some(m.as_str().to_string());
        }
    }

    // match: sourceCompatibility = '17' or JavaVersion.VERSION_17
    let compat_re = Regex::new(
        r#"(?:sourceCompatibility|targetCompatibility)\s*=\s*['"]?(?:JavaVersion\.VERSION_)?(\d+)"#,
    )
    .ok()?;
    if let Some(caps) = compat_re.captures(&content) {
        if let Some(m) = caps.get(1) {
            return Some(m.as_str().to_string());
        }
    }

    None
}

fn parse_pom_java_version(file: &Path) -> Option<String> {
    let content = fs::read_to_string(file).ok()?;
    // match: <java.version>21</java.version> or <maven.compiler.target>17</maven.compiler.target>
    let pom_re = Regex::new(
        r"<(?:java\.version|maven\.compiler\.target|maven\.compiler\.release)>\s*(\d+)\s*</",
    )
    .ok()?;
    if let Some(caps) = pom_re.captures(&content) {
        if let Some(m) = caps.get(1) {
            return Some(m.as_str().to_string());
        }
    }
    None
}

fn normalize_version_string(raw: &str) -> String {
    // raw could be "temurin-21.0.4" or "openjdk-17" or "21"
    let re = Regex::new(r"(\d+(?:\.\d+)*)").unwrap();
    if let Some(caps) = re.captures(raw) {
        if let Some(m) = caps.get(1) {
            let ver = m.as_str();
            // If major version like "21.0.4", take the major "21" if it's modern
            if let Some((major, _)) = ver.split_once('.') {
                if major != "1" {
                    return major.to_string();
                }
            }
            return ver.to_string();
        }
    }
    raw.to_string()
}

fn normalize_sdkman_version(raw: &str) -> String {
    // sdkman version format: "21.0.4-tem" or "17.0.12-amzn"
    if let Some((ver, _)) = raw.split_once('-') {
        normalize_version_string(ver)
    } else {
        normalize_version_string(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_gradle_project() {
        let dir = tempdir().unwrap();
        let gradlew = dir.path().join("gradlew");
        fs::write(&gradlew, "#!/bin/sh\n").unwrap();

        let build_gradle = dir.path().join("build.gradle");
        fs::write(&build_gradle, "java { jvmToolchain(21) }").unwrap();

        let markers = detect_project_markers(dir.path());
        assert_eq!(markers.build_tool, Some(BuildTool::Gradle));
        assert!(markers.has_gradle_wrapper);
        assert_eq!(markers.inferred_java_version, Some("21".to_string()));
    }

    #[test]
    fn test_detect_maven_project() {
        let dir = tempdir().unwrap();
        let mvnw = dir.path().join("mvnw");
        fs::write(&mvnw, "#!/bin/sh\n").unwrap();

        let pom = dir.path().join("pom.xml");
        fs::write(
            &pom,
            "<project><properties><java.version>17</java.version></properties></project>",
        )
        .unwrap();

        let markers = detect_project_markers(dir.path());
        assert_eq!(markers.build_tool, Some(BuildTool::Maven));
        assert!(markers.has_maven_wrapper);
        assert_eq!(markers.inferred_java_version, Some("17".to_string()));
    }

    #[test]
    fn test_detect_java_version_file() {
        let dir = tempdir().unwrap();
        let jv = dir.path().join(".java-version");
        fs::write(&jv, "21.0.4\n").unwrap();

        let markers = detect_project_markers(dir.path());
        assert_eq!(markers.inferred_java_version, Some("21".to_string()));
    }

    #[test]
    fn test_detect_sdkmanrc_file() {
        let dir = tempdir().unwrap();
        let rc = dir.path().join(".sdkmanrc");
        fs::write(&rc, "java=17.0.12-amzn\n").unwrap();

        let markers = detect_project_markers(dir.path());
        assert_eq!(markers.inferred_java_version, Some("17".to_string()));
    }
}
