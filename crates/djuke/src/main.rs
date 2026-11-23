use clap::Parser;
use miette::Result;
use std::env;
use std::path::PathBuf;

use djuke_core::DjukePaths;

mod cli;
mod commands;
mod prompts;
mod updater;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    djuke_build::init_terminal_safety();

    let cli = Cli::parse();

    if cli.no_color {
        colored::control::set_override(false);
    }

    let paths = DjukePaths::discover();
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    match cli.command {
        Commands::Doctor => {
            commands::doctor::handle_doctor(&current_dir, &paths, cli.json).await?;
        }
        Commands::Jdk(jdk_args) => {
            commands::java::handle_java(jdk_args, &current_dir, &paths, cli.json, cli.quiet)
                .await?;
        }
        Commands::New {
            name,
            template,
            build_tool,
            group,
            java,
            distro,
            test_framework,
            yes,
        } => {
            commands::project::handle_new(
                cli::CliProjectOptions {
                    is_init: false,
                    current_dir: current_dir.clone(),
                    name,
                    template,
                    build_tool,
                    group,
                    java,
                    distro,
                    test_framework,
                    yes,
                },
                &current_dir,
                &paths,
                cli.quiet,
            )
            .await?;
        }
        Commands::Init {
            name,
            template,
            build_tool,
            group,
            java,
            distro,
            test_framework,
            yes,
        } => {
            commands::project::handle_init(
                cli::CliProjectOptions {
                    is_init: true,
                    current_dir: current_dir.clone(),
                    name,
                    template,
                    build_tool,
                    group,
                    java,
                    distro,
                    test_framework,
                    yes,
                },
                &current_dir,
                &paths,
                cli.quiet,
            )
            .await?;
        }
        Commands::Use {
            spec,
            distro,
            global,
        } => {
            commands::java::handle_use_command(
                spec,
                distro,
                global,
                &paths,
                &current_dir,
                cli.quiet,
            )
            .await?;
        }
        Commands::Add {
            dependency,
            scope,
            test,
        } => {
            commands::pm::handle_add(dependency, scope, test, &current_dir, &paths, cli.quiet)
                .await?;
        }
        Commands::Remove { dependency } => {
            commands::pm::handle_remove(dependency, &current_dir, &paths)?;
        }
        Commands::Search { query, limit } => {
            commands::pm::handle_search(query, limit).await?;
        }
        Commands::Cache(cache_args) => {
            commands::cache::handle_cache(cache_args, &paths, cli.json)?;
        }
        Commands::Setup { tool, distro, spec } => {
            commands::project::handle_setup(tool, distro, spec, &current_dir, &paths, cli.quiet)
                .await?;
        }
        Commands::Build(build_args) => {
            commands::build::handle_build(build_args, &current_dir, &paths, cli.quiet).await?;
        }
        Commands::Test(test_args) => {
            commands::build::handle_test(test_args, &current_dir, &paths, cli.quiet).await?;
        }
        Commands::Run(run_args) => {
            commands::build::handle_run(run_args, &current_dir, &paths, cli.quiet).await?;
        }
        Commands::Dev(dev_args) => {
            commands::build::handle_dev(dev_args, &current_dir, &paths, cli.quiet).await?;
        }
        Commands::Tree => {
            commands::pm::handle_tree(&current_dir, &paths)?;
        }
        Commands::Outdated => {
            commands::pm::handle_outdated(&current_dir, &paths).await?;
        }
        Commands::Audit => {
            commands::pm::handle_audit(&current_dir, &paths).await?;
        }
        Commands::Clean(clean_args) => {
            commands::build::handle_clean(clean_args, &current_dir, &paths, cli.quiet).await?;
        }
        Commands::Exec { command, args } => {
            commands::build::handle_exec(command, args, &current_dir, &paths, cli.quiet).await?;
        }
        Commands::Env { shell } => {
            commands::env::handle_env(shell, &current_dir, &paths)?;
        }
        Commands::Completions { shell } => {
            use clap::CommandFactory;
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        }
        Commands::Gradle(gradle_args) => {
            commands::build::handle_gradle(gradle_args, &current_dir, &paths, cli.quiet).await?;
        }
        Commands::Maven(maven_args) => {
            commands::build::handle_maven(maven_args, &current_dir, &paths, cli.quiet).await?;
        }
        Commands::Upgrade { check, force } => {
            updater::run_self_upgrade(check, force, cli.quiet).await?;
        }
        Commands::External(args) => {
            commands::build::handle_external(args, &current_dir, &paths, cli.quiet).await?;
        }
    }

    Ok(())
}
