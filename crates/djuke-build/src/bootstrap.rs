use std::fs;
use std::path::{Path, PathBuf};

use djuke_archive::extract_archive;
use djuke_core::{BuildTool, DjukeError, Result};
use djuke_download::Downloader;
use djuke_platform::DjukePaths;

pub const DEFAULT_GRADLE_VERSION: &str = "8.10.2";
pub const DEFAULT_MAVEN_VERSION: &str = "3.9.9";

// Embedded standard wrapper assets
const GRADLEW_SH: &str = include_str!("../assets/gradlew");
const GRADLEW_BAT: &str = include_str!("../assets/gradlew.bat");
const GRADLE_WRAPPER_JAR: &[u8] = include_bytes!("../assets/gradle-wrapper.jar");

const MVNW_SH: &str = include_str!("../assets/mvnw");
const MVNW_CMD: &str = include_str!("../assets/mvnw.cmd");
const MAVEN_WRAPPER_JAR: &[u8] = include_bytes!("../assets/maven-wrapper.jar");

/// Directly creates a complete, fully functional Gradle wrapper without downloading a Gradle distribution.
pub fn create_gradle_wrapper(project_root: &Path, version: Option<&str>) -> Result<()> {
    let v = version.unwrap_or(DEFAULT_GRADLE_VERSION);
    let wrapper_dir = project_root.join("gradle").join("wrapper");
    fs::create_dir_all(&wrapper_dir)?;

    // 1. Write gradlew.bat (Windows)
    fs::write(project_root.join("gradlew.bat"), GRADLEW_BAT)?;

    // 2. Write gradlew (POSIX)
    let gradlew_path = project_root.join("gradlew");
    fs::write(&gradlew_path, GRADLEW_SH)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(&gradlew_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&gradlew_path, perms);
        }
    }

    // 3. Write gradle-wrapper.jar
    fs::write(wrapper_dir.join("gradle-wrapper.jar"), GRADLE_WRAPPER_JAR)?;

    // 4. Write gradle-wrapper.properties
    let props = format!(
        "distributionBase=GRADLE_USER_HOME\n\
distributionPath=wrapper/dists\n\
distributionUrl=https\\://services.gradle.org/distributions/gradle-{}-bin.zip\n\
networkTimeout=10000\n\
validateDistributionUrl=true\n\
zipStoreBase=GRADLE_USER_HOME\n\
zipStorePath=wrapper/dists\n",
        v
    );
    fs::write(wrapper_dir.join("gradle-wrapper.properties"), props)?;

    Ok(())
}

/// Directly creates a complete, fully functional Maven wrapper without downloading a Maven distribution.
pub fn create_maven_wrapper(project_root: &Path, version: Option<&str>) -> Result<()> {
    let v = version.unwrap_or(DEFAULT_MAVEN_VERSION);
    let wrapper_dir = project_root.join(".mvn").join("wrapper");
    fs::create_dir_all(&wrapper_dir)?;

    // 1. Write mvnw.cmd (Windows)
    fs::write(project_root.join("mvnw.cmd"), MVNW_CMD)?;

    // 2. Write mvnw (POSIX)
    let mvnw_path = project_root.join("mvnw");
    fs::write(&mvnw_path, MVNW_SH)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(&mvnw_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&mvnw_path, perms);
        }
    }

    // 3. Write maven-wrapper.jar
    fs::write(wrapper_dir.join("maven-wrapper.jar"), MAVEN_WRAPPER_JAR)?;

    // 4. Write maven-wrapper.properties
    let props = format!(
        "distributionUrl=https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/{}/apache-maven-{}-bin.zip\n\
wrapperVersion=3.3.2\n",
        v, v
    );
    fs::write(wrapper_dir.join("maven-wrapper.properties"), props)?;

    Ok(())
}

/// Creates a build wrapper for the given build tool directly in the project root.
pub async fn bootstrap_wrapper(
    tool: BuildTool,
    version: Option<&str>,
    project_root: &Path,
    _java_home: &Path,
    _paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    if !quiet {
        eprintln!("Creating {} wrapper in {}...", tool, project_root.display());
    }

    match tool {
        BuildTool::Gradle => create_gradle_wrapper(project_root, version)?,
        BuildTool::Maven => create_maven_wrapper(project_root, version)?,
    }

    if !quiet {
        eprintln!("✔ {} wrapper created successfully.", tool);
    }

    Ok(())
}

/// Utility to install a standalone tool distribution in ~/.djuke/tools/ if explicitly desired.
pub async fn ensure_build_tool_installed(
    tool: BuildTool,
    version: Option<&str>,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<PathBuf> {
    paths.ensure_dirs()?;

    let (ver, download_url, bin_name) = match tool {
        BuildTool::Gradle => {
            let v = version.unwrap_or(DEFAULT_GRADLE_VERSION);
            let url = format!(
                "https://services.gradle.org/distributions/gradle-{}-bin.zip",
                v
            );
            let bin = if cfg!(windows) {
                "bin/gradle.bat"
            } else {
                "bin/gradle"
            };
            (v, url, bin)
        }
        BuildTool::Maven => {
            let v = version.unwrap_or(DEFAULT_MAVEN_VERSION);
            let url = format!(
                "https://archive.apache.org/dist/maven/maven-3/{}/binaries/apache-maven-{}-bin.zip",
                v, v
            );
            let bin = if cfg!(windows) {
                "bin/mvn.cmd"
            } else {
                "bin/mvn"
            };
            (v, url, bin)
        }
    };

    let tool_dir = paths.tools_dir().join(tool.to_string()).join(ver);
    let exe_path = tool_dir.join(bin_name);

    if exe_path.is_file() {
        return Ok(exe_path);
    }

    if !quiet {
        eprintln!("Downloading official {} {} distribution...", tool, ver);
    }

    let archive_path = paths.cache_dir().join(format!("{}-{}.zip", tool, ver));
    let downloader = Downloader::new(quiet);
    downloader
        .download_file(
            &download_url,
            &archive_path,
            None,
            &format!("{} {}", tool, ver),
        )
        .await?;

    if !quiet {
        eprintln!("Extracting {} to {}...", tool, tool_dir.display());
    }

    extract_archive(&archive_path, &tool_dir)?;

    if !exe_path.is_file() {
        return Err(DjukeError::NoBuildToolFound(format!(
            "Failed to locate executable '{}' in extracted archive at '{}'",
            bin_name,
            tool_dir.display()
        )));
    }

    Ok(exe_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_gradle_wrapper_creates_all_files_offline() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        create_gradle_wrapper(root, None).unwrap();

        assert!(root.join("gradlew").is_file());
        assert!(root.join("gradlew.bat").is_file());
        assert!(root
            .join("gradle")
            .join("wrapper")
            .join("gradle-wrapper.jar")
            .is_file());
        let props_file = root
            .join("gradle")
            .join("wrapper")
            .join("gradle-wrapper.properties");
        assert!(props_file.is_file());

        let props = fs::read_to_string(&props_file).unwrap();
        assert!(props.contains("gradle-8.10.2-bin.zip"));
    }

    #[test]
    fn test_create_maven_wrapper_creates_all_files_offline() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        create_maven_wrapper(root, None).unwrap();

        assert!(root.join("mvnw").is_file());
        assert!(root.join("mvnw.cmd").is_file());
        assert!(root
            .join(".mvn")
            .join("wrapper")
            .join("maven-wrapper.jar")
            .is_file());
        let props_file = root
            .join(".mvn")
            .join("wrapper")
            .join("maven-wrapper.properties");
        assert!(props_file.is_file());

        let props = fs::read_to_string(&props_file).unwrap();
        assert!(props.contains("apache-maven-3.9.9-bin.zip"));
    }
}
