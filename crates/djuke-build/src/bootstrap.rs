use std::fs;
use std::path::{Path, PathBuf};

use djuke_core::{BuildTool, DjukeError, DjukePaths, Result};
use djuke_io::{extract_archive, Downloader};
use crate::process::{CommandContext, ProcessRunner};

pub const DEFAULT_GRADLE_VERSION: &str = "8.14.0";
pub const DEFAULT_MAVEN_VERSION: &str = "3.9.9";

/// Extracts the integer major version from a version specification string (e.g. "21", "27.0.1.1", "17.0.10", "latest", "lts").
pub fn extract_java_major(version: &str) -> Option<u32> {
    let trimmed = version.trim();
    if trimmed.eq_ignore_ascii_case("latest") || trimmed.eq_ignore_ascii_case("lts") {
        return Some(21);
    }
    let s = if let Some(stripped) = trimmed.strip_prefix("1.") {
        stripped
    } else {
        trimmed
    };
    let num: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse::<u32>().ok()
}

/// Resolves the optimal, officially compatible Gradle version for a given target Java version.
/// Defaults to stable Gradle 8.14.0 for Java 17+, and Gradle 8.5 for legacy Java 8-16.
pub fn compatible_gradle_version_for_java(java_version: &str) -> &'static str {
    let major = extract_java_major(java_version).unwrap_or(21);
    match major {
        0..=16 => "8.5",
        17..=u32::MAX => "8.14.0",
    }
}

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

    // 5. Ensure gradle.properties exists with modern performance & native access tuning
    let gradle_props = project_root.join("gradle.properties");
    if !gradle_props.exists() {
        let is_modern = v.starts_with('9') || v.starts_with("8.14") || v.starts_with("8.13");
        let jvmargs = if is_modern {
            "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8 --enable-native-access=ALL-UNNAMED"
        } else {
            "org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8"
        };
        let props_content = format!(
            "{}\norg.gradle.parallel=true\norg.gradle.caching=true\n",
            jvmargs
        );
        let _ = fs::write(gradle_props, props_content);
    }

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
    java_home: &Path,
    paths: &DjukePaths,
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
        eprintln!("✓ {} wrapper created successfully.", tool);
    }

    // Automatically prime and pre-download the wrapper distribution so future commands run instantly
    if java_home.is_dir() {
        let _ = prime_wrapper(tool, project_root, java_home, paths, quiet).await;
    }

    Ok(())
}

/// Pre-downloads and verifies the build tool wrapper distribution so that runtime commands run instantly.
pub async fn prime_wrapper(
    tool: BuildTool,
    project_root: &Path,
    java_home: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let resolved =
        crate::wrapper::find_build_executable_with_paths(project_root, tool, Some(paths))
            .ok_or_else(|| {
                DjukeError::NoBuildToolFound(format!(
                    "Cannot prime wrapper: no {} wrapper executable found in '{}'",
                    tool,
                    project_root.display()
                ))
            })?;

    if !quiet {
        eprintln!("Priming and caching {} wrapper distribution...", tool);
    }

    let program_str = resolved.executable.to_string_lossy();
    let args: Vec<&str> = match tool {
        BuildTool::Gradle => vec!["--version"],
        BuildTool::Maven => vec!["-v"],
    };

    let ctx = CommandContext {
        java_home: java_home.to_path_buf(),
        working_dir: project_root.to_path_buf(),
        extra_env: std::collections::HashMap::new(),
    };

    let status = ProcessRunner::run_command(&program_str, &args, &ctx).await?;
    if !status.success() {
        return Err(DjukeError::Generic(format!(
            "Failed to prime {} wrapper: exit code {}",
            tool,
            status.code().unwrap_or(1)
        )));
    }

    if !quiet {
        eprintln!("✓ {} wrapper distribution primed and ready.", tool);
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
                "https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/{}/apache-maven-{}-bin.zip",
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
        assert!(props.contains("gradle-8.14.0-bin.zip"));
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
