use clap::{Args, Parser, Subcommand};
use colored::Colorize;
use miette::Result;
use std::env;
use std::path::{Path, PathBuf};

use djuke_build::{execute_task, TaskExecution};
use djuke_config::{load_manifest, resolve_project_config};
use djuke_core::{DjukeError, JdkDistribution, JdkSpec, ProjectManifest};
use djuke_doctor::run_doctor_checks;
use djuke_jdk::{install_jdk, resolve_jdk, DiscoClient, JdkStore};
use djuke_platform::{DjukePaths, Platform};
use djuke_process::{CommandContext, ProcessRunner};

#[derive(Parser)]
#[command(
    name = "djuke",
    author = "djuke contributors",
    version,
    about = "Fast, seamless JDK manager and build-tool orchestrator for Java ecosystems",
    long_about = "A developer can clone any Java project, run djuke, and get the correct JDK and build environment immediately."
)]
struct Cli {
    #[arg(short, long, global = true, help = "Enable verbose debug logging")]
    verbose: bool,

    #[arg(short, long, global = true, help = "Suppress non-essential output")]
    quiet: bool,

    #[arg(
        long,
        global = true,
        help = "Output machine-readable JSON where applicable"
    )]
    json: bool,

    #[arg(long, global = true, help = "Disable ANSI color output")]
    no_color: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Check development environment health and diagnostic status")]
    Doctor,

    #[command(about = "JDK installation, listing, and switching")]
    Java(JavaArgs),

    #[command(about = "Prepare project dependencies, JDK, and build tool wrappers")]
    Setup,

    #[command(about = "Execute the project build task")]
    Build {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    #[command(about = "Execute the project test task")]
    Test {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    #[command(about = "Execute the project run task")]
    Run {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    #[command(about = "Execute the project clean task")]
    Clean {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    #[command(about = "Execute an arbitrary command with project Java environment injected")]
    Exec {
        #[arg(required = true)]
        command: String,

        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    #[command(about = "Generate shell export commands for PATH and JAVA_HOME")]
    Env {
        #[arg(help = "Shell type: bash, zsh, fish, powershell")]
        shell: Option<String>,
    },
}

#[derive(Args)]
struct JavaArgs {
    #[command(subcommand)]
    command: JavaSubcommands,
}

#[derive(Subcommand)]
enum JavaSubcommands {
    #[command(about = "List installed or remote available JDKs")]
    List {
        #[arg(long, help = "Query remote Foojay Disco API for available versions")]
        remote: bool,

        #[arg(long, help = "Filter remote query for LTS versions only")]
        lts: bool,
    },

    #[command(about = "Install a JDK (e.g. '21', 'temurin@21', 'corretto@17')")]
    Install {
        #[arg(help = "JDK specification e.g. '21' or 'temurin@21'")]
        spec: String,
    },

    #[command(about = "Set the active Java version for project or globally")]
    Use {
        #[arg(help = "JDK specification e.g. '21' or 'temurin@21'")]
        spec: String,

        #[arg(
            long,
            help = "Set as global default instead of modifying local project"
        )]
        global: bool,
    },

    #[command(about = "Show the currently resolved Java runtime")]
    Current,

    #[command(about = "Uninstall a locally managed JDK")]
    Uninstall {
        #[arg(help = "JDK id, version, or folder name to remove")]
        target: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_color {
        colored::control::set_override(false);
    }

    let paths = DjukePaths::discover();
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match cli.command {
        Commands::Doctor => {
            let report = run_doctor_checks(&current_dir, &paths).await?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?
                );
            } else {
                report.print_human();
            }
        }

        Commands::Java(java_args) => match java_args.command {
            JavaSubcommands::List { remote, lts } => {
                let store = JdkStore::new(paths.clone());
                if remote {
                    let disco = DiscoClient::new();
                    let platform = Platform::current();
                    let target_versions = if lts {
                        vec!["21", "17", "11", "8"]
                    } else {
                        vec!["23", "22", "21", "17", "11"]
                    };

                    println!(
                        "{}",
                        format!("Available remote JDKs for {} (Foojay Disco API):", platform)
                            .bold()
                            .cyan()
                    );
                    println!(
                        "{}",
                        "--------------------------------------------------------".cyan()
                    );

                    for ver in target_versions {
                        if let Ok(packages) = disco
                            .query_packages(ver, JdkDistribution::Temurin, platform)
                            .await
                        {
                            for pkg in packages.into_iter().take(2) {
                                let distro = pkg.distribution.unwrap_or_else(|| "temurin".into());
                                let java_ver = pkg.java_version.unwrap_or_else(|| ver.to_string());
                                println!("  {}@{}", distro.green(), java_ver);
                            }
                        }
                    }
                } else {
                    let installed = store.list_installed()?;
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&installed).map_err(|e| {
                                djuke_core::DjukeError::Serialization(e.to_string())
                            })?
                        );
                    } else if installed.is_empty() {
                        println!("No JDKs installed in {}.", paths.jdks_dir().display());
                        println!(
                            "Run '{}' to install a recommended JDK.",
                            "djuke java install 21".green()
                        );
                    } else {
                        println!("{}", "Installed JDKs:".bold().cyan());
                        for jdk in installed {
                            println!(
                                "  {} {} ({}) -> {}",
                                "•".cyan(),
                                format!("{}@{}", jdk.distribution, jdk.version)
                                    .green()
                                    .bold(),
                                jdk.major_version,
                                jdk.java_home.display()
                            );
                        }
                    }
                }
            }

            JavaSubcommands::Install { spec } => {
                let parsed_spec: JdkSpec = spec.parse()?;
                let installed = install_jdk(&parsed_spec, &paths, cli.quiet).await?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&installed)
                            .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?
                    );
                }
            }

            JavaSubcommands::Use { spec, global } => {
                let parsed_spec: JdkSpec = spec.parse()?;
                // Ensure installed
                let installed = resolve_jdk(&parsed_spec, &paths, true, cli.quiet).await?;

                if global {
                    let mut cfg = djuke_config::global::load_global_config(&paths.config_file())?;
                    let mut j = cfg.java.unwrap_or_default();
                    j.default_version = Some(parsed_spec.version.clone());
                    j.default_distribution = Some(parsed_spec.distribution);
                    cfg.java = Some(j);
                    djuke_config::global::save_global_config(&paths.config_file(), &cfg)?;
                    println!(
                        "✔ Set global default Java to {}@{}",
                        parsed_spec.distribution, parsed_spec.version
                    );
                } else {
                    let manifest_file = current_dir.join("djuke.toml");
                    let mut manifest = if manifest_file.is_file() {
                        load_manifest(&manifest_file)?
                    } else {
                        ProjectManifest::default()
                    };

                    manifest.java = Some(djuke_core::model::JavaSection {
                        version: parsed_spec.version.clone(),
                        distribution: Some(parsed_spec.distribution),
                    });

                    djuke_config::manifest::save_manifest(&manifest_file, &manifest)?;
                    println!(
                        "✔ Configured project in '{}' to use {}@{}",
                        manifest_file.display(),
                        parsed_spec.distribution,
                        parsed_spec.version
                    );
                }
                println!("  JAVA_HOME: {}", installed.java_home.display());
            }

            JavaSubcommands::Current => {
                let config = resolve_project_config(&current_dir, None, &paths);
                let store = JdkStore::new(paths.clone());
                let installed = store.find_installed(&config.jdk_spec);

                if cli.json {
                    let json_data = serde_json::json!({
                        "spec": format!("{}", config.jdk_spec),
                        "distribution": config.jdk_spec.distribution.to_string(),
                        "version": config.jdk_spec.version,
                        "installed": installed.is_some(),
                        "java_home": installed.as_ref().map(|i| i.java_home.display().to_string())
                    });
                    println!("{}", serde_json::to_string_pretty(&json_data).unwrap());
                } else if let Some(i) = installed {
                    println!(
                        "Java runtime: {} -> {}",
                        format!("{}@{}", i.distribution, i.version).green().bold(),
                        i.java_home.display()
                    );
                } else {
                    println!(
                        "Configured Java: {} (not installed locally)",
                        format!("{}", config.jdk_spec).yellow()
                    );
                    println!("Run '{}' to install.", "djuke setup".green());
                }
            }

            JavaSubcommands::Uninstall { target } => {
                let store = JdkStore::new(paths.clone());
                let removed_path = store.uninstall(&target)?;
                println!("✔ Removed JDK at {}", removed_path.display());
            }
        },

        Commands::Setup => {
            let config = resolve_project_config(&current_dir, None, &paths);
            println!("{}", "🚀 Running djuke setup...".bold().cyan());

            // 1. Ensure JDK installed
            let installed_jdk = resolve_jdk(&config.jdk_spec, &paths, true, cli.quiet).await?;

            // 2. Validate build tool
            let tool_str = config
                .build_tool
                .map(|t| t.to_string())
                .unwrap_or_else(|| "none".into());

            println!("\n{}", "Project Environment Ready:".green().bold());
            println!(
                "  • Java:       {} ({})",
                installed_jdk.version.green(),
                installed_jdk.distribution
            );
            println!("  • JAVA_HOME:  {}", installed_jdk.java_home.display());
            println!("  • Build Tool: {}", tool_str.cyan());
            println!("\nRun '{}' to build the project.", "djuke build".bold());
        }

        Commands::Build { args } => {
            run_task("build", args, &current_dir, &paths, cli.quiet).await?;
        }

        Commands::Test { args } => {
            run_task("test", args, &current_dir, &paths, cli.quiet).await?;
        }

        Commands::Run { args } => {
            run_task("run", args, &current_dir, &paths, cli.quiet).await?;
        }

        Commands::Clean { args } => {
            run_task("clean", args, &current_dir, &paths, cli.quiet).await?;
        }

        Commands::Exec { command, args } => {
            let config = resolve_project_config(&current_dir, None, &paths);
            let installed_jdk = resolve_jdk(&config.jdk_spec, &paths, true, cli.quiet).await?;

            let arg_slices: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let ctx = CommandContext {
                java_home: installed_jdk.java_home,
                working_dir: config.project_root,
                extra_env: config.env,
            };

            let status = ProcessRunner::run_command(&command, &arg_slices, &ctx).await?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }

        Commands::Env { shell } => {
            let config = resolve_project_config(&current_dir, None, &paths);
            let store = JdkStore::new(paths.clone());
            if let Some(installed) = store.find_installed(&config.jdk_spec) {
                let java_home = installed.java_home.display();
                let bin_dir = installed.java_home.join("bin").display().to_string();
                let shell_type = shell.unwrap_or_else(|| "bash".into());

                match shell_type.as_str() {
                    "powershell" | "pwsh" => {
                        println!("$env:JAVA_HOME = \"{}\"", java_home);
                        println!("$env:PATH = \"{};$env:PATH\"", bin_dir);
                    }
                    _ => {
                        println!("export JAVA_HOME=\"{}\"", java_home);
                        println!("export PATH=\"{}:$PATH\"", bin_dir);
                    }
                }
            } else {
                eprintln!(
                    "No JDK installed for {}. Run 'djuke setup' first.",
                    config.jdk_spec
                );
            }
        }
    }

    Ok(())
}

async fn run_task(
    task_name: &str,
    extra_args: Vec<String>,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let config = resolve_project_config(current_dir, None, paths);
    let installed_jdk = resolve_jdk(&config.jdk_spec, paths, true, quiet).await?;

    let tool = config.build_tool.ok_or_else(|| {
        DjukeError::NoBuildToolFound(format!(
            "Cannot run task '{}': no Maven or Gradle project detected in '{}'",
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
    })
    .await?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
