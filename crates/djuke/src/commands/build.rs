use clap::CommandFactory;
use colored::Colorize;
use miette::Result;
use std::path::Path;

use djuke_build::{execute_task, TaskExecution};
use djuke_config::resolve_project_config_with_tool;
use djuke_core::{BuildTool, DjukeError};
use djuke_jdk::resolve_jdk;
use djuke_platform::DjukePaths;
use djuke_process::{CommandContext, ProcessRunner};

use crate::cli::{BuildTaskArgs, GradleArgs, GradleSubcommands, MavenArgs, MavenSubcommands, RunTaskArgs};
use crate::commands::script::run_single_file_script;

pub async fn run_task(
    task_name: &str,
    extra_args: Vec<String>,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
    tool_override: Option<BuildTool>,
) -> Result<()> {
    let config = resolve_project_config_with_tool(current_dir, None, tool_override, paths);
    let installed_jdk = resolve_jdk(&config.jdk_spec, paths, true, quiet).await?;

    let tool = config.build_tool.ok_or_else(|| {
        DjukeError::NoBuildToolFound(format!(
            "Cannot run task '{}': no Maven or Gradle project detected in '{}'. Tip: pass '--tool maven' or '--tool gradle', or run 'djuke new' / 'djuke init'.",
            task_name,
            current_dir.display()
        ))
    })?;

    let task_cmd = config
        .tasks
        .get(task_name)
        .cloned()
        .unwrap_or_else(|| task_name.to_string());

    let status = execute_task(TaskExecution {
        project_root: &config.project_root,
        java_home: &installed_jdk.java_home,
        tool,
        task_name,
        task_command: &task_cmd,
        extra_args: &extra_args,
        extra_env: &config.env,
        paths: Some(paths),
    })
    .await?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

pub async fn run_raw_tool(
    tool: BuildTool,
    args: &[String],
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let config = resolve_project_config_with_tool(current_dir, None, Some(tool), paths);
    let installed_jdk = resolve_jdk(&config.jdk_spec, paths, true, quiet).await?;

    let status = execute_task(TaskExecution {
        project_root: &config.project_root,
        java_home: &installed_jdk.java_home,
        tool,
        task_name: &tool.to_string(),
        task_command: "",
        extra_args: args,
        extra_env: &config.env,
        paths: Some(paths),
    })
    .await?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

pub async fn handle_build(
    args: BuildTaskArgs,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let tool = resolve_project_config_with_tool(current_dir, None, args.tool, paths)
        .build_tool
        .unwrap_or(BuildTool::Gradle);
    let tool_args = args.clone().into_tool_args(tool);
    run_task("build", tool_args, current_dir, paths, quiet, args.tool).await
}

pub async fn handle_test(
    args: BuildTaskArgs,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let tool = resolve_project_config_with_tool(current_dir, None, args.tool, paths)
        .build_tool
        .unwrap_or(BuildTool::Gradle);
    let tool_args = args.clone().into_tool_args(tool);
    run_task("test", tool_args, current_dir, paths, quiet, args.tool).await
}

pub async fn handle_clean(
    args: BuildTaskArgs,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let tool = resolve_project_config_with_tool(current_dir, None, args.tool, paths)
        .build_tool
        .unwrap_or(BuildTool::Gradle);
    let tool_args = args.clone().into_tool_args(tool);
    run_task("clean", tool_args, current_dir, paths, quiet, args.tool).await
}

pub async fn handle_dev(
    args: BuildTaskArgs,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let tool = resolve_project_config_with_tool(current_dir, None, args.tool, paths)
        .build_tool
        .unwrap_or(BuildTool::Gradle);
    let tool_args = args.clone().into_tool_args(tool);
    run_task("dev", tool_args, current_dir, paths, quiet, args.tool).await
}

pub async fn handle_run(
    args: RunTaskArgs,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    if let Some(first_arg) = args.args.first() {
        if first_arg.ends_with(".java") {
            let file_path = std::path::PathBuf::from(first_arg);
            if !file_path.is_file() {
                return Err(DjukeError::Generic(format!(
                    "Java script file '{}' not found",
                    first_arg
                ))
                .into());
            }
            let remaining_args = if args.args.len() > 1 {
                &args.args[1..]
            } else {
                &[]
            };
            return run_single_file_script(
                &file_path,
                remaining_args,
                args.tool,
                paths,
                quiet,
            )
            .await;
        }
    }

    let tool = resolve_project_config_with_tool(current_dir, None, args.tool, paths)
        .build_tool
        .unwrap_or(BuildTool::Gradle);
    let tool_args = args.clone().into_tool_args(tool);
    run_task("run", tool_args, current_dir, paths, quiet, args.tool).await
}

pub async fn handle_exec(
    command: String,
    args: Vec<String>,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let config = resolve_project_config_with_tool(current_dir, None, None, paths);
    let installed_jdk = resolve_jdk(&config.jdk_spec, paths, true, quiet).await?;

    let ctx = CommandContext {
        java_home: installed_jdk.java_home,
        working_dir: current_dir.to_path_buf(),
        extra_env: config.env,
    };

    let arg_slices: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let status = ProcessRunner::run_command(&command, &arg_slices, &ctx).await?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

pub async fn handle_gradle(
    args: GradleArgs,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    match args.command {
        Some(GradleSubcommands::Wrapper { version }) => {
            let config = resolve_project_config_with_tool(
                current_dir,
                None,
                Some(BuildTool::Gradle),
                paths,
            );
            let v = match version {
                Some(ver) => ver,
                None => {
                    let java_str = &config.jdk_spec.version;
                    djuke_build::resolve_dynamic_gradle_version(java_str, Some(paths), true).await
                }
            };
            djuke_build::create_gradle_wrapper(&config.project_root, Some(&v))?;
            if let Ok(installed_jdk) = resolve_jdk(&config.jdk_spec, paths, true, quiet).await {
                let _ = djuke_build::prime_wrapper(
                    BuildTool::Gradle,
                    &config.project_root,
                    &installed_jdk.java_home,
                    paths,
                    quiet,
                )
                .await;
            }
            if !quiet {
                println!(
                    "{}",
                    "✔ Gradle wrapper created and primed successfully."
                        .green()
                        .bold()
                );
            }
        }
        Some(GradleSubcommands::Stop) => {
            run_raw_tool(
                BuildTool::Gradle,
                &["--stop".into()],
                current_dir,
                paths,
                quiet,
            )
            .await?;
        }
        Some(GradleSubcommands::Status) => {
            run_raw_tool(
                BuildTool::Gradle,
                &["--status".into()],
                current_dir,
                paths,
                quiet,
            )
            .await?;
        }
        None => {
            let cmd_args = if args.args.is_empty() {
                vec!["--help".to_string()]
            } else {
                args.args
            };
            run_raw_tool(
                BuildTool::Gradle,
                &cmd_args,
                current_dir,
                paths,
                quiet,
            )
            .await?;
        }
    }
    Ok(())
}

pub async fn handle_maven(
    args: MavenArgs,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    match args.command {
        Some(MavenSubcommands::Wrapper { version }) => {
            let config = resolve_project_config_with_tool(
                current_dir,
                None,
                Some(BuildTool::Maven),
                paths,
            );
            let v = match version {
                Some(ver) => ver,
                None => djuke_build::resolve_dynamic_maven_version(Some(paths), true).await,
            };
            djuke_build::create_maven_wrapper(&config.project_root, Some(&v))?;
            if let Ok(installed_jdk) = resolve_jdk(&config.jdk_spec, paths, true, quiet).await {
                let _ = djuke_build::prime_wrapper(
                    BuildTool::Maven,
                    &config.project_root,
                    &installed_jdk.java_home,
                    paths,
                    quiet,
                )
                .await;
            }
            if !quiet {
                println!(
                    "{}",
                    "✔ Maven wrapper created and primed successfully."
                        .green()
                        .bold()
                );
            }
        }
        None => {
            let cmd_args = if args.args.is_empty() {
                vec!["--version".to_string()]
            } else {
                args.args
            };
            run_raw_tool(
                BuildTool::Maven,
                &cmd_args,
                current_dir,
                paths,
                quiet,
            )
            .await?;
        }
    }
    Ok(())
}

pub async fn handle_external(
    args: Vec<String>,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    if args.is_empty() {
        crate::cli::Cli::command().print_help().ok();
        println!();
        return Ok(());
    }
    if args[0].ends_with(".java") {
        let file_path = std::path::PathBuf::from(&args[0]);
        if !file_path.is_file() {
            return Err(DjukeError::Generic(format!(
                "Java script file '{}' not found",
                args[0]
            ))
            .into());
        }
        let remaining_args = if args.len() > 1 { &args[1..] } else { &[] };
        return run_single_file_script(&file_path, remaining_args, None, paths, quiet).await;
    }
    let task_name = &args[0];
    let task_args = args[1..].to_vec();
    run_task(task_name, task_args, current_dir, paths, quiet, None).await
}

