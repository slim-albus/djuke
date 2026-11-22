use std::collections::HashMap;
use std::path::Path;
use std::process::ExitStatus;

use djuke_core::{BuildTool, DjukeError, Result};
use djuke_process::{CommandContext, ProcessRunner};

use crate::wrapper::find_build_executable;

#[derive(Debug, Clone)]
pub struct TaskExecution<'a> {
    pub project_root: &'a Path,
    pub java_home: &'a Path,
    pub tool: BuildTool,
    pub task_name: &'a str,
    pub task_command: &'a str,
    pub extra_args: &'a [String],
    pub extra_env: &'a HashMap<String, String>,
}

pub async fn execute_task(exec: TaskExecution<'_>) -> Result<ExitStatus> {
    let resolved = find_build_executable(exec.project_root, exec.tool).ok_or_else(|| {
        DjukeError::NoBuildToolFound(format!(
            "Neither wrapper nor system executable found for '{}' in '{}'",
            exec.tool,
            exec.project_root.display()
        ))
    })?;

    let mut cmd_args: Vec<&str> = exec.task_command.split_whitespace().collect();

    for arg in exec.extra_args {
        cmd_args.push(arg.as_str());
    }

    let program_str = resolved.executable.to_string_lossy();

    let ctx = CommandContext {
        java_home: exec.java_home.to_path_buf(),
        working_dir: exec.project_root.to_path_buf(),
        extra_env: exec.extra_env.clone(),
    };

    ProcessRunner::run_command(&program_str, &cmd_args, &ctx).await
}
