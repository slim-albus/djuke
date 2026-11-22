use clap::{Args, CommandFactory, Parser, Subcommand};
use colored::Colorize;
use miette::Result;
use std::env;
use std::path::{Path, PathBuf};

use djuke_build::{execute_task, TaskExecution};
use djuke_config::{load_manifest, resolve_project_config};
use djuke_core::{BuildTool, DjukeError, JdkDistribution, JdkSpec, ProjectManifest};
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

    #[command(about = "Create a new Java project from a template")]
    New {
        #[arg(help = "Project name or destination directory")]
        name: String,

        #[arg(
            short,
            long,
            default_value = "cli",
            help = "Template type: cli, lib, spring-boot, quarkus, micronaut"
        )]
        template: String,

        #[arg(
            short,
            long,
            default_value = "gradle-kts",
            help = "Build tool: gradle-kts, gradle-groovy, maven"
        )]
        build_tool: String,

        #[arg(long, default_value = "com.example", help = "Root package / group ID")]
        group: String,

        #[arg(long, default_value = "21", help = "Target Java version")]
        java: String,
    },

    #[command(about = "Initialize a new Java project in the current directory")]
    Init {
        #[arg(
            short,
            long,
            help = "Project name (defaults to current directory name)"
        )]
        name: Option<String>,

        #[arg(
            short,
            long,
            default_value = "cli",
            help = "Template type: cli, lib, spring-boot, quarkus, micronaut"
        )]
        template: String,

        #[arg(
            short,
            long,
            default_value = "gradle-kts",
            help = "Build tool: gradle-kts, gradle-groovy, maven"
        )]
        build_tool: String,

        #[arg(long, default_value = "com.example", help = "Root package / group ID")]
        group: String,

        #[arg(long, default_value = "21", help = "Target Java version")]
        java: String,
    },

    #[command(about = "Add a dependency to the current project (Maven or Gradle)")]
    Add {
        #[arg(
            help = "Dependency coordinate (e.g. 'org.postgresql:postgresql', 'com.google.guava:guava:33.3.1-jre', or 'guava')"
        )]
        dependency: String,

        #[arg(
            short,
            long,
            default_value = "implementation",
            help = "Scope: implementation, test, compileOnly, runtimeOnly"
        )]
        scope: String,

        #[arg(long, help = "Add as test dependency")]
        test: bool,
    },

    #[command(about = "Remove a dependency from the current project")]
    Remove {
        #[arg(help = "Dependency coordinate or groupId:artifactId to remove")]
        dependency: String,
    },

    #[command(about = "Search Maven Central for packages and libraries")]
    Search {
        #[arg(help = "Search query string")]
        query: String,

        #[arg(short, long, default_value = "10", help = "Maximum results to display")]
        limit: u32,
    },

    #[command(about = "Manage downloaded archives and temporary cache files")]
    Cache(CacheArgs),

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

    #[command(about = "Execute project application or single Java source file")]
    Run {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    #[command(about = "Start the project application in development/live-reload mode")]
    Dev {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    #[command(about = "Display a visual tree of project dependencies")]
    Tree,

    #[command(about = "Check for outdated dependencies against Maven Central")]
    Outdated,

    #[command(about = "Audit dependencies for known security vulnerabilities via OSV.dev")]
    Audit,

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

    #[command(about = "Generate shell autocomplete scripts")]
    Completions {
        #[arg(help = "Shell type: bash, zsh, fish, powershell, elvish")]
        shell: clap_complete::Shell,
    },

    #[command(about = "Gradle build tool commands")]
    Gradle(GradleArgs),

    #[command(about = "Maven build tool commands")]
    Maven(MavenArgs),
}

#[derive(Args)]
struct CacheArgs {
    #[command(subcommand)]
    command: CacheSubcommands,
}

#[derive(Subcommand)]
enum CacheSubcommands {
    #[command(about = "Show cache size and item count")]
    Stats,

    #[command(about = "List all downloaded archive files in cache")]
    List,

    #[command(about = "Clear cached archive files and temporary extraction directories")]
    Clean,
}

#[derive(Args)]
struct GradleArgs {
    #[command(subcommand)]
    command: GradleSubcommands,
}

#[derive(Subcommand)]
enum GradleSubcommands {
    #[command(about = "Generate the Gradle wrapper (gradlew, gradlew.bat)")]
    Wrapper {
        #[arg(long, help = "Gradle version to generate (default: 8.10.2)")]
        version: Option<String>,
    },
}

#[derive(Args)]
struct MavenArgs {
    #[command(subcommand)]
    command: MavenSubcommands,
}

#[derive(Subcommand)]
enum MavenSubcommands {
    #[command(about = "Generate the Maven wrapper (mvnw, mvnw.cmd)")]
    Wrapper {
        #[arg(long, help = "Maven version to generate (default: 3.9.9)")]
        version: Option<String>,
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

        Commands::New {
            name,
            template,
            build_tool,
            group,
            java,
        } => {
            let dest = current_dir.join(&name);
            if dest.exists()
                && dest
                    .read_dir()
                    .map(|mut d| d.next().is_some())
                    .unwrap_or(false)
            {
                return Err(DjukeError::Generic(format!(
                    "Target directory '{}' already exists and is not empty",
                    dest.display()
                ))
                .into());
            }

            let t_type: djuke_templates::TemplateType = template.parse()?;
            let b_tool: djuke_templates::BuildToolChoice = build_tool.parse()?;

            println!(
                "{}",
                format!("Creating new Java {} project '{}'...", t_type, name)
                    .bold()
                    .cyan()
            );

            let opts = djuke_templates::NewProjectOptions {
                name: name.clone(),
                group_id: group,
                template: t_type,
                build_tool: b_tool,
                java_version: java,
                destination: dest.clone(),
            };

            djuke_templates::generate_project(&opts)?;

            println!("{}", "✔ Project initialized successfully!".green().bold());
            println!("\nNext steps:");
            println!("  cd {}", name.bold());
            println!("  djuke setup");
            println!("  djuke run\n");
        }

        Commands::Init {
            name,
            template,
            build_tool,
            group,
            java,
        } => {
            let project_name = name.unwrap_or_else(|| {
                current_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("app")
                    .to_string()
            });

            let t_type: djuke_templates::TemplateType = template.parse()?;
            let b_tool: djuke_templates::BuildToolChoice = build_tool.parse()?;

            println!(
                "{}",
                format!(
                    "Initializing Java {} project '{}' in current directory...",
                    t_type, project_name
                )
                .bold()
                .cyan()
            );

            let opts = djuke_templates::NewProjectOptions {
                name: project_name.clone(),
                group_id: group,
                template: t_type,
                build_tool: b_tool,
                java_version: java,
                destination: current_dir.clone(),
            };

            djuke_templates::generate_project(&opts)?;

            println!("{}", "✔ Project initialized successfully!".green().bold());
            println!("\nNext steps:");
            println!("  djuke setup");
            println!("  djuke run\n");
        }

        Commands::Add {
            dependency,
            scope,
            test,
        } => {
            let mut spec: djuke_pm::DependencySpec = dependency.parse()?;
            if test {
                spec.scope = djuke_pm::DependencyScope::Test;
            } else {
                spec.scope = scope.parse()?;
            }

            if spec.version.is_none() {
                if !cli.quiet {
                    println!(
                        "Resolving latest version for {}:{} from Maven Central...",
                        spec.group_id, spec.artifact_id
                    );
                }
                let central = djuke_pm::MavenCentralClient::new();
                match central
                    .resolve_latest_version(&spec.group_id, &spec.artifact_id)
                    .await
                {
                    Ok(v) => {
                        if !cli.quiet {
                            println!("Resolved latest version: {}", v.green().bold());
                        }
                        spec.version = Some(v);
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠ Could not resolve version from Maven Central: {}. Proceeding without explicit version.",
                            e
                        );
                    }
                }
            }

            let config = resolve_project_config(&current_dir, None, &paths);
            match config.build_tool {
                Some(BuildTool::Maven) => {
                    let pom_path = config.project_root.join("pom.xml");
                    if !pom_path.is_file() {
                        return Err(DjukeError::Generic(
                            "pom.xml not found in project root".into(),
                        )
                        .into());
                    }
                    djuke_pm::add_maven_dependency(&pom_path, &spec)?;
                    println!("✔ Added {} to pom.xml", spec.to_string().green().bold());
                }
                Some(BuildTool::Gradle) | None => {
                    let kts = config.project_root.join("build.gradle.kts");
                    let groovy = config.project_root.join("build.gradle");
                    let build_file = if kts.is_file() {
                        kts
                    } else if groovy.is_file() {
                        groovy
                    } else {
                        return Err(DjukeError::Generic(
                            "No build.gradle or build.gradle.kts found in project root".into(),
                        )
                        .into());
                    };
                    djuke_pm::add_gradle_dependency(&build_file, &spec)?;
                    println!(
                        "✔ Added {} to {}",
                        spec.to_string().green().bold(),
                        build_file.file_name().unwrap().to_string_lossy()
                    );
                }
            }
        }

        Commands::Remove { dependency } => {
            let parts: Vec<&str> = dependency.split(':').collect();
            if parts.len() < 2 {
                return Err(
                    DjukeError::Generic("Please provide 'groupId:artifactId'".into()).into(),
                );
            }
            let group = parts[0].trim();
            let artifact = parts[1].trim();

            let config = resolve_project_config(&current_dir, None, &paths);
            match config.build_tool {
                Some(BuildTool::Maven) => {
                    let pom_path = config.project_root.join("pom.xml");
                    djuke_pm::remove_maven_dependency(&pom_path, group, artifact)?;
                    println!("✔ Removed {}:{} from pom.xml", group, artifact);
                }
                Some(BuildTool::Gradle) | None => {
                    let kts = config.project_root.join("build.gradle.kts");
                    let groovy = config.project_root.join("build.gradle");
                    let build_file = if kts.is_file() { kts } else { groovy };
                    djuke_pm::remove_gradle_dependency(&build_file, group, artifact)?;
                    println!(
                        "✔ Removed {}:{} from {}",
                        group,
                        artifact,
                        build_file.file_name().unwrap().to_string_lossy()
                    );
                }
            }
        }

        Commands::Search { query, limit } => {
            let central = djuke_pm::MavenCentralClient::new();
            println!(
                "{}",
                format!("Searching Maven Central for '{}'...", query)
                    .bold()
                    .cyan()
            );
            let docs = central.search(&query, limit).await?;
            if docs.is_empty() {
                println!("No matching artifacts found.");
            } else {
                println!(
                    "{:<45} {:<15} {}",
                    "ARTIFACT".bold(),
                    "LATEST".bold(),
                    "PACKAGING".bold()
                );
                println!(
                    "{}",
                    "------------------------------------------------------------------------"
                        .cyan()
                );
                for doc in docs {
                    let coord = format!("{}:{}", doc.group_id, doc.artifact_id);
                    let ver = doc.latest_version.unwrap_or_else(|| "-".into());
                    let pkg = doc.packaging.unwrap_or_else(|| "jar".into());
                    println!("{:<45} {:<15} {}", coord.green(), ver.bold(), pkg);
                }
            }
        }

        Commands::Cache(cache_args) => match cache_args.command {
            CacheSubcommands::Stats => {
                let stats = djuke_cache::get_cache_stats(&paths)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&stats).unwrap());
                } else {
                    println!("{}", "djuke Cache Statistics:".bold().cyan());
                    println!("  • Cached Files: {}", stats.file_count);
                    println!("  • Total Disk Usage: {}", stats.human_size.bold().green());
                    println!("  • Cache Directory: {}", paths.cache_dir().display());
                }
            }
            CacheSubcommands::List => {
                let files = djuke_cache::list_cached_files(&paths)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&files).unwrap());
                } else if files.is_empty() {
                    println!("No cached archives in {}", paths.cache_dir().display());
                } else {
                    println!("{}", "Cached Archive Downloads:".bold().cyan());
                    for f in files {
                        println!("  • {:<50} ({})", f.name.green(), f.path.display());
                    }
                }
            }
            CacheSubcommands::Clean => {
                let (count, _total_bytes) = djuke_cache::clean_cache(&paths)?;
                println!("✔ Cleared {} files from cache.", count);
            }
        },

        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
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

            // 3. Auto-bootstrap wrapper if missing
            let mut wrapper_status = "present";
            if let Some(tool) = config.build_tool {
                let has_wrapper = config.has_gradle_wrapper || config.has_maven_wrapper;
                if !has_wrapper {
                    if !cli.quiet {
                        println!("No wrapper detected. Bootstrapping {} wrapper...", tool);
                    }
                    if let Err(e) = djuke_build::bootstrap_wrapper(
                        tool,
                        None,
                        &config.project_root,
                        &installed_jdk.java_home,
                        &paths,
                        cli.quiet,
                    )
                    .await
                    {
                        eprintln!("⚠ Wrapper generation failed: {}", e);
                        wrapper_status = "missing (generation failed)";
                    } else {
                        wrapper_status = "generated";
                    }
                }
            }

            println!("\n{}", "Project Environment Ready:".green().bold());
            println!(
                "  • Java:       {} ({})",
                installed_jdk.version.green(),
                installed_jdk.distribution
            );
            println!("  • JAVA_HOME:  {}", installed_jdk.java_home.display());
            println!(
                "  • Build Tool: {} (wrapper: {})",
                tool_str.cyan(),
                wrapper_status
            );
            println!("\nRun '{}' to build the project.", "djuke build".bold());
        }

        Commands::Gradle(gradle_args) => match gradle_args.command {
            GradleSubcommands::Wrapper { version } => {
                let config = resolve_project_config(&current_dir, None, &paths);
                let installed_jdk = resolve_jdk(&config.jdk_spec, &paths, true, cli.quiet).await?;
                djuke_build::bootstrap_wrapper(
                    BuildTool::Gradle,
                    version.as_deref(),
                    &config.project_root,
                    &installed_jdk.java_home,
                    &paths,
                    cli.quiet,
                )
                .await?;
            }
        },

        Commands::Maven(maven_args) => match maven_args.command {
            MavenSubcommands::Wrapper { version } => {
                let config = resolve_project_config(&current_dir, None, &paths);
                let installed_jdk = resolve_jdk(&config.jdk_spec, &paths, true, cli.quiet).await?;
                djuke_build::bootstrap_wrapper(
                    BuildTool::Maven,
                    version.as_deref(),
                    &config.project_root,
                    &installed_jdk.java_home,
                    &paths,
                    cli.quiet,
                )
                .await?;
            }
        },

        Commands::Build { args } => {
            run_task("build", args, &current_dir, &paths, cli.quiet).await?;
        }

        Commands::Test { args } => {
            run_task("test", args, &current_dir, &paths, cli.quiet).await?;
        }

        Commands::Run { args } => {
            // Check if first argument is a single .java source file (JEP 330 launcher)
            if let Some(first_arg) = args.first() {
                if first_arg.ends_with(".java") {
                    let file_path = PathBuf::from(first_arg);
                    if file_path.is_file() {
                        let config = resolve_project_config(&current_dir, None, &paths);
                        let installed_jdk =
                            resolve_jdk(&config.jdk_spec, &paths, true, cli.quiet).await?;
                        let java_bin = DjukePaths::java_executable(&installed_jdk.java_home);
                        let bin_str = java_bin.to_string_lossy();
                        let arg_slices: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                        let ctx = CommandContext {
                            java_home: installed_jdk.java_home,
                            working_dir: current_dir.clone(),
                            extra_env: config.env,
                        };
                        let status =
                            ProcessRunner::run_command(&bin_str, &arg_slices, &ctx).await?;
                        if !status.success() {
                            std::process::exit(status.code().unwrap_or(1));
                        }
                        return Ok(());
                    }
                }
            }

            run_task("run", args, &current_dir, &paths, cli.quiet).await?;
        }

        Commands::Dev { args } => {
            run_task("dev", args, &current_dir, &paths, cli.quiet).await?;
        }

        Commands::Tree => {
            let config = resolve_project_config(&current_dir, None, &paths);
            let deps = djuke_pm::extract_project_dependencies(&config.project_root)?;
            let proj_name = config
                .project_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("project");
            let rendered = djuke_pm::render_dependency_tree(proj_name, &deps);
            println!("{}", rendered);
        }

        Commands::Outdated => {
            let config = resolve_project_config(&current_dir, None, &paths);
            let deps = djuke_pm::extract_project_dependencies(&config.project_root)?;
            if deps.is_empty() {
                println!("No dependencies found in project.");
                return Ok(());
            }

            println!(
                "{}",
                format!(
                    "Checking {} dependencies against Maven Central...",
                    deps.len()
                )
                .bold()
                .cyan()
            );
            let central = djuke_pm::MavenCentralClient::new();
            let results = djuke_pm::check_outdated_dependencies(&central, &deps).await?;
            let table = djuke_pm::render_outdated_table(&results);
            println!("\n{}", table);
        }

        Commands::Audit => {
            let config = resolve_project_config(&current_dir, None, &paths);
            let deps = djuke_pm::extract_project_dependencies(&config.project_root)?;
            if deps.is_empty() {
                println!("No dependencies found in project to audit.");
                return Ok(());
            }

            println!(
                "{}",
                format!(
                    "Auditing {} dependencies against OSV.dev vulnerability database...",
                    deps.len()
                )
                .bold()
                .cyan()
            );
            let audit_client = djuke_pm::AuditClient::new();
            let reports = audit_client.audit_dependencies(&deps).await?;
            let report_str = djuke_pm::render_audit_report(&reports, deps.len());
            println!("{}", report_str);
            if !reports.is_empty() {
                std::process::exit(1);
            }
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
        paths: Some(paths),
    })
    .await?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
