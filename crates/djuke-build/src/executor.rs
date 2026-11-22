use std::collections::HashMap;
use std::path::Path;
use std::process::ExitStatus;

use djuke_core::{BuildTool, DjukeError, Result};
use djuke_platform::DjukePaths;
use djuke_process::{CommandContext, ProcessRunner};

use crate::bootstrap::{create_gradle_wrapper, create_maven_wrapper};
use crate::wrapper::find_build_executable_with_paths;

#[derive(Debug, Clone)]
pub struct TaskExecution<'a> {
    pub project_root: &'a Path,
    pub java_home: &'a Path,
    pub tool: BuildTool,
    pub task_name: &'a str,
    pub task_command: &'a str,
    pub extra_args: &'a [String],
    pub extra_env: &'a HashMap<String, String>,
    pub paths: Option<&'a DjukePaths>,
}

pub async fn execute_task(exec: TaskExecution<'_>) -> Result<ExitStatus> {
    // 1. Ensure wrapper exists in project root. If missing, auto-create it instantly from embedded assets!
    let has_wrapper = match exec.tool {
        BuildTool::Gradle => {
            let bat = exec.project_root.join("gradlew.bat");
            let sh = exec.project_root.join("gradlew");
            (cfg!(windows) && bat.is_file()) || sh.is_file()
        }
        BuildTool::Maven => {
            let cmd = exec.project_root.join("mvnw.cmd");
            let sh = exec.project_root.join("mvnw");
            (cfg!(windows) && cmd.is_file()) || sh.is_file()
        }
    };

    if !has_wrapper {
        eprintln!(
            "No {} wrapper found in project. Auto-generating compatible wrapper...",
            exec.tool
        );
        match exec.tool {
            BuildTool::Gradle => {
                let java_ver = infer_java_version_from_path(exec.java_home);
                let g_ver = crate::bootstrap::compatible_gradle_version_for_java(&java_ver);
                create_gradle_wrapper(exec.project_root, Some(g_ver))?;
            }
            BuildTool::Maven => create_maven_wrapper(exec.project_root, None)?,
        }
    }

    let resolved = find_build_executable_with_paths(exec.project_root, exec.tool, exec.paths)
        .ok_or_else(|| {
            DjukeError::NoBuildToolFound(format!(
                "Failed to find or generate '{}' wrapper in '{}'",
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

fn infer_java_version_from_path(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    for part in name.split(['-', '_', '@']) {
        let digit_prefix: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digit_prefix.is_empty() {
            return digit_prefix;
        }
    }
    "21".to_string()
}
