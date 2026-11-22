use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::ExitStatus;
use tokio::process::Command;

use djuke_core::{DjukeError, Result};

#[derive(Debug, Clone)]
pub struct CommandContext {
    pub java_home: PathBuf,
    pub working_dir: PathBuf,
    pub extra_env: HashMap<String, String>,
}

pub struct ProcessRunner;

impl ProcessRunner {
    pub async fn run_command(
        program: &str,
        args: &[&str],
        ctx: &CommandContext,
    ) -> Result<ExitStatus> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        cmd.current_dir(&ctx.working_dir);

        // 1. Injected JAVA_HOME
        cmd.env("JAVA_HOME", &ctx.java_home);

        // 2. Injected PATH: prepend $JAVA_HOME/bin
        let bin_dir = ctx.java_home.join("bin");
        let current_path = env::var("PATH").unwrap_or_default();
        let new_path = if cfg!(windows) {
            format!("{};{}", bin_dir.display(), current_path)
        } else {
            format!("{}:{}", bin_dir.display(), current_path)
        };
        cmd.env("PATH", new_path);

        // 3. Extra user env vars
        for (k, v) in &ctx.extra_env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().map_err(|e| {
            DjukeError::Process(format!(
                "Failed to spawn program '{}' in '{}': {}",
                program,
                ctx.working_dir.display(),
                e
            ))
        })?;

        let status = child
            .wait()
            .await
            .map_err(|e| DjukeError::Process(format!("Process error: {}", e)))?;

        Ok(status)
    }
}
