use std::collections::HashMap;
use std::path::{Path, PathBuf};

use djuke_archive::extract_archive;
use djuke_core::{BuildTool, DjukeError, Result};
use djuke_download::Downloader;
use djuke_platform::DjukePaths;
use djuke_process::{CommandContext, ProcessRunner};

pub const DEFAULT_GRADLE_VERSION: &str = "8.10.2";
pub const DEFAULT_MAVEN_VERSION: &str = "3.9.9";

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

pub async fn bootstrap_wrapper(
    tool: BuildTool,
    version: Option<&str>,
    project_root: &Path,
    java_home: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let tool_exe = ensure_build_tool_installed(tool, version, paths, quiet).await?;

    if !quiet {
        eprintln!(
            "Generating {} wrapper in {}...",
            tool,
            project_root.display()
        );
    }

    let program_str = tool_exe.to_string_lossy();
    let ctx = CommandContext {
        java_home: java_home.to_path_buf(),
        working_dir: project_root.to_path_buf(),
        extra_env: HashMap::new(),
    };

    let status = match tool {
        BuildTool::Gradle => {
            let v = version.unwrap_or(DEFAULT_GRADLE_VERSION);
            let args = ["wrapper", "--gradle-version", v];
            ProcessRunner::run_command(&program_str, &args, &ctx).await?
        }
        BuildTool::Maven => {
            let args = ["wrapper:wrapper"];
            ProcessRunner::run_command(&program_str, &args, &ctx).await?
        }
    };

    if !status.success() {
        return Err(DjukeError::Process(format!(
            "Failed to generate {} wrapper (exit status: {})",
            tool, status
        )));
    }

    if !quiet {
        eprintln!("✔ {} wrapper generated successfully.", tool);
    }

    Ok(())
}
