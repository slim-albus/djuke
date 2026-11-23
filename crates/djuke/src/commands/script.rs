use colored::Colorize;
use miette::Result;
use std::env;
use std::path::{Path, PathBuf};

use djuke_config::resolve_project_config_with_tool;
use djuke_core::{BuildTool, DjukeError};
use djuke_jdk::resolve_jdk;
use djuke_build::{CommandContext, ProcessRunner};
use djuke_core::DjukePaths;

pub async fn run_single_file_script(
    file_path: &Path,
    script_args: &[String],
    tool_override: Option<BuildTool>,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let script_content = std::fs::read_to_string(file_path).map_err(|e| {
        DjukeError::Generic(format!(
            "Failed to read script '{}': {}",
            file_path.display(),
            e
        ))
    })?;
    let directives = djuke_pm::ScriptDirectives::parse(&script_content);

    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut config = resolve_project_config_with_tool(&current_dir, None, tool_override, paths);

    // If script specifies //JAVA <ver>, override config's jdk_spec
    if let Some(ref jv) = directives.java_version {
        config.jdk_spec.version = jv.clone();
    }

    let installed_jdk = resolve_jdk(&config.jdk_spec, paths, true, quiet).await?;
    let java_bin = DjukePaths::java_executable(&installed_jdk.java_home);
    let bin_str = java_bin.to_string_lossy();

    // Resolve dependencies if any
    let mut resolved_jars = Vec::new();
    if !directives.dependencies.is_empty() {
        if !quiet {
            println!(
                "{} Resolving {} script dependencies for {}...",
                "⚡".cyan(),
                directives.dependencies.len(),
                file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .green()
            );
        }
        resolved_jars = djuke_pm::resolve_script_dependencies(
            &directives.dependencies,
            &paths.cache_dir(),
            quiet,
        )
        .await?;
    }

    // Build final java invocation arguments:
    let mut java_args: Vec<String> = Vec::new();

    let has_preview = directives
        .jvm_options
        .iter()
        .any(|o| o == "--enable-preview");
    let has_source = directives
        .jvm_options
        .iter()
        .any(|o| o == "--source" || o.starts_with("--source="));

    for opt in &directives.jvm_options {
        java_args.push(opt.clone());
    }

    // Java source launcher requires --source <version> when --enable-preview is present
    if has_preview && !has_source {
        java_args.push("--source".to_string());
        java_args.push(config.jdk_spec.version.clone());
    }

    if !resolved_jars.is_empty() {
        java_args.push("-cp".to_string());
        java_args.push(djuke_pm::build_classpath(&resolved_jars));
    }

    // The main script file to execute must be specified first
    java_args.push(file_path.to_string_lossy().to_string());

    for src in &directives.sources {
        java_args.push(src.clone());
    }

    for arg in script_args {
        java_args.push(arg.clone());
    }

    let arg_slices: Vec<&str> = java_args.iter().map(|s| s.as_str()).collect();
    let ctx = CommandContext {
        java_home: installed_jdk.java_home,
        working_dir: current_dir,
        extra_env: config.env,
    };

    let status = ProcessRunner::run_command(&bin_str, &arg_slices, &ctx).await?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
