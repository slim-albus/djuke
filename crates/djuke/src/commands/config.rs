use crate::cli::{ConfigArgs, ConfigSubcommands};
use colored::Colorize;
use djuke_config::{load_global_config, save_global_config};
use djuke_core::DjukePaths;
use miette::Result;

pub fn handle_config(args: ConfigArgs, paths: &DjukePaths, json: bool) -> Result<()> {
    let config_path = paths.config_file();
    let mut cfg = load_global_config(&config_path).unwrap_or_default();

    match args.action {
        Some(ConfigSubcommands::Get { key }) => {
            if let Some(val) = cfg.get_property(&key) {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "key": key,
                            "value": val
                        }))
                        .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?
                    );
                } else {
                    println!("{}", val);
                }
            } else {
                return Err(djuke_core::DjukeError::Config(format!(
                    "Configuration key '{}' not found. Available keys: distribution, version, build_tool, test_framework",
                    key
                ))
                .into());
            }
        }
        Some(ConfigSubcommands::Set { key, value }) => {
            cfg.set_property(&key, &value)?;
            save_global_config(&config_path, &cfg)?;
            println!(
                "{} Set {} = {}",
                "✓".green().bold(),
                key.bold(),
                value.cyan()
            );
        }
        None | Some(ConfigSubcommands::List) => {
            let distro = cfg.default_distribution().to_string();
            let version = cfg.default_java_version();
            let build_tool = cfg
                .default_build_tool()
                .unwrap_or_else(|| "auto".to_string());
            let test_framework = cfg
                .default_test_framework()
                .unwrap_or_else(|| "auto".to_string());

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "config_file": config_path.to_string_lossy(),
                        "distribution": distro,
                        "version": version,
                        "build_tool": build_tool,
                        "test_framework": test_framework
                    }))
                    .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?
                );
            } else {
                println!("{}", "djuke Global Configuration".bold().cyan());
                println!(
                    "  Configuration file: {}",
                    config_path.display().to_string().dimmed()
                );
                println!();
                println!("  • {:<16} = {}", "distribution".bold(), distro.green());
                println!("  • {:<16} = {}", "version".bold(), version.green());
                println!("  • {:<16} = {}", "build_tool".bold(), build_tool.green());
                println!(
                    "  • {:<16} = {}",
                    "test_framework".bold(),
                    test_framework.green()
                );
            }
        }
    }

    Ok(())
}
