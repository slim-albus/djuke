use std::path::Path;
use miette::Result;
use djuke_config::resolve_project_config;
use djuke_jdk::JdkStore;
use djuke_platform::DjukePaths;

pub fn handle_env(shell: Option<String>, current_dir: &Path, paths: &DjukePaths) -> Result<()> {
    let config = resolve_project_config(current_dir, None, paths);
    let store = JdkStore::new(paths.clone());
    if let Some(installed) = store.find_installed(&config.jdk_spec) {
        let java_home = installed.java_home.display().to_string();
        let bin_dir = installed.java_home.join("bin").display().to_string();
        let shell_type = shell.unwrap_or_else(|| {
            if cfg!(windows) {
                "powershell".into()
            } else {
                "bash".into()
            }
        });

        match shell_type.as_str() {
            "powershell" | "pwsh" => {
                println!("$env:JAVA_HOME = \"{}\"", java_home);
                println!("$env:PATH = \"{};$env:PATH\"", bin_dir);
            }
            "cmd" | "batch" => {
                println!("set \"JAVA_HOME={}\"", java_home);
                println!("set \"PATH={};%PATH%\"", bin_dir);
            }
            "fish" => {
                let posix_home = java_home.replace('\\', "/");
                let posix_bin = bin_dir.replace('\\', "/");
                println!("set -gx JAVA_HOME \"{}\"", posix_home);
                println!("set -gx PATH \"{}\" $PATH", posix_bin);
            }
            "nu" | "nushell" => {
                let posix_home = java_home.replace('\\', "/");
                let posix_bin = bin_dir.replace('\\', "/");
                println!("$env.JAVA_HOME = \"{}\"", posix_home);
                println!("$env.PATH = ($env.PATH | prepend \"{}\")", posix_bin);
            }
            _ => {
                let posix_home = java_home.replace('\\', "/");
                let posix_bin = bin_dir.replace('\\', "/");
                println!("export JAVA_HOME=\"{}\"", posix_home);
                println!("export PATH=\"{}:$PATH\"", posix_bin);
            }
        }
    } else {
        eprintln!(
            "No JDK installed for {}. Run 'djuke setup' first.",
            config.jdk_spec
        );
    }
    Ok(())
}
