use clap::{Args, CommandFactory, Parser, Subcommand};
use colored::Colorize;
use miette::Result;
use std::env;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use djuke_build::{execute_task, TaskExecution};
use djuke_config::{load_manifest, resolve_project_config, resolve_project_config_with_tool};
use djuke_core::{BuildTool, DjukeError, JdkDistribution, JdkSpec, ProjectManifest};
use djuke_doctor::run_doctor_checks;
use djuke_jdk::{disco::DiscoMajorVersion, install_jdk, resolve_jdk, DiscoClient, JdkStore};
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
        name: Option<String>,

        #[arg(
            short,
            long,
            help = "Template type: cli, lib, spring-boot, quarkus, micronaut"
        )]
        template: Option<String>,

        #[arg(
            short = 'b',
            long = "build-tool",
            help = "Build tool: gradle-kts, gradle-groovy, maven"
        )]
        build_tool: Option<String>,

        #[arg(long, help = "Root package / group ID (default: com.example)")]
        group: Option<String>,

        #[arg(long, help = "Target Java version (number, 'latest', or 'lts')")]
        java: Option<String>,

        #[arg(
            short = 'd',
            long = "distro",
            help = "JDK distribution: temurin, openjdk, corretto, zulu, microsoft, semeru, graalvm, liberica, oracle, sapmachine"
        )]
        distro: Option<JdkDistribution>,

        #[arg(
            short = 'y',
            long = "yes",
            help = "Skip interactive prompts and use defaults"
        )]
        yes: bool,
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
            help = "Template type: cli, lib, spring-boot, quarkus, micronaut"
        )]
        template: Option<String>,

        #[arg(
            short = 'b',
            long = "build-tool",
            help = "Build tool: gradle-kts, gradle-groovy, maven"
        )]
        build_tool: Option<String>,

        #[arg(long, help = "Root package / group ID (default: com.example)")]
        group: Option<String>,

        #[arg(long, help = "Target Java version (number, 'latest', or 'lts')")]
        java: Option<String>,

        #[arg(
            short = 'd',
            long = "distro",
            help = "JDK distribution: temurin, openjdk, corretto, zulu, microsoft, semeru, graalvm, liberica, oracle, sapmachine"
        )]
        distro: Option<JdkDistribution>,

        #[arg(
            short = 'y',
            long = "yes",
            help = "Skip interactive prompts and use defaults"
        )]
        yes: bool,
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
    Setup {
        #[arg(
            short = 't',
            long = "tool",
            help = "Explicitly specify build tool: gradle or maven"
        )]
        tool: Option<BuildTool>,

        #[arg(
            short = 'd',
            long = "distro",
            help = "JDK distribution: temurin, openjdk, corretto, zulu, microsoft, semeru, graalvm, liberica, oracle, sapmachine"
        )]
        distro: Option<JdkDistribution>,
    },

    #[command(about = "Execute the project build task")]
    Build(BuildTaskArgs),

    #[command(about = "Execute the project test task")]
    Test(BuildTaskArgs),

    #[command(about = "Execute project application or single Java source file")]
    Run(RunTaskArgs),

    #[command(about = "Start the project application in development/live-reload mode")]
    Dev(BuildTaskArgs),

    #[command(about = "Display a visual tree of project dependencies")]
    Tree,

    #[command(about = "Check for outdated dependencies against Maven Central")]
    Outdated,

    #[command(about = "Audit dependencies for known security vulnerabilities via OSV.dev")]
    Audit,

    #[command(about = "Execute the project clean task")]
    Clean(BuildTaskArgs),

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

    #[command(about = "Gradle build tool commands and direct execution")]
    Gradle(GradleArgs),

    #[command(about = "Maven build tool commands and direct execution")]
    Maven(MavenArgs),

    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Args, Debug, Clone)]
struct BuildTaskArgs {
    #[arg(
        short = 't',
        long = "tool",
        help = "Explicitly specify build tool: gradle or maven"
    )]
    tool: Option<BuildTool>,

    #[arg(
        long,
        help = "Run in offline mode without network dependency resolution"
    )]
    offline: bool,

    #[arg(
        short = 'U',
        long = "refresh",
        help = "Refresh dynamic dependencies and snapshot releases"
    )]
    refresh: bool,

    #[arg(
        short = 'P',
        help = "Pass project property (Gradle: -Pprop=val) or activate profile (Maven: -Pprofile)"
    )]
    profile: Vec<String>,

    #[arg(short = 'D', help = "Pass system property (e.g. -DskipTests)")]
    define: Vec<String>,

    #[arg(long, help = "Execute tasks in parallel")]
    parallel: bool,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

impl BuildTaskArgs {
    fn into_tool_args(self, tool: BuildTool) -> Vec<String> {
        let mut res = Vec::new();
        if self.offline {
            match tool {
                BuildTool::Gradle => res.push("--offline".into()),
                BuildTool::Maven => res.push("-o".into()),
            }
        }
        if self.refresh {
            match tool {
                BuildTool::Gradle => res.push("--refresh-dependencies".into()),
                BuildTool::Maven => res.push("-U".into()),
            }
        }
        if self.parallel {
            match tool {
                BuildTool::Gradle => res.push("--parallel".into()),
                BuildTool::Maven => {
                    res.push("-T".into());
                    res.push("1C".into());
                }
            }
        }
        for p in self.profile {
            res.push(format!("-P{}", p));
        }
        for d in self.define {
            res.push(format!("-D{}", d));
        }
        res.extend(self.args);
        res
    }
}

#[derive(Args, Debug, Clone)]
struct RunTaskArgs {
    #[arg(
        short = 't',
        long = "tool",
        help = "Explicitly specify build tool: gradle or maven"
    )]
    tool: Option<BuildTool>,

    #[arg(
        long,
        help = "Run in offline mode without network dependency resolution"
    )]
    offline: bool,

    #[arg(
        short = 'U',
        long = "refresh",
        help = "Refresh dynamic dependencies and snapshot releases"
    )]
    refresh: bool,

    #[arg(
        short = 'P',
        help = "Pass project property (Gradle: -Pprop=val) or activate profile (Maven: -Pprofile)"
    )]
    profile: Vec<String>,

    #[arg(short = 'D', help = "Pass system property (e.g. -Dserver.port=8080)")]
    define: Vec<String>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

impl RunTaskArgs {
    fn into_tool_args(self, tool: BuildTool) -> Vec<String> {
        let mut res = Vec::new();
        if self.offline {
            match tool {
                BuildTool::Gradle => res.push("--offline".into()),
                BuildTool::Maven => res.push("-o".into()),
            }
        }
        if self.refresh {
            match tool {
                BuildTool::Gradle => res.push("--refresh-dependencies".into()),
                BuildTool::Maven => res.push("-U".into()),
            }
        }
        for p in self.profile {
            res.push(format!("-P{}", p));
        }
        for d in self.define {
            res.push(format!("-D{}", d));
        }
        res.extend(self.args);
        res
    }
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
    command: Option<GradleSubcommands>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

#[derive(Subcommand)]
enum GradleSubcommands {
    #[command(about = "Generate the Gradle wrapper (gradlew, gradlew.bat)")]
    Wrapper {
        #[arg(long, help = "Gradle version to generate (default: 8.10.2)")]
        version: Option<String>,
    },

    #[command(about = "Stop running Gradle daemons")]
    Stop,

    #[command(about = "Show status of running Gradle daemons")]
    Status,
}

#[derive(Args)]
struct MavenArgs {
    #[command(subcommand)]
    command: Option<MavenSubcommands>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
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

        #[arg(
            short,
            long,
            help = "JDK distribution: temurin, openjdk, corretto, zulu, microsoft, semeru, graalvm, liberica, oracle, sapmachine"
        )]
        distro: Option<JdkDistribution>,
    },

    #[command(about = "Install a JDK (e.g. '21', 'latest', 'temurin@21', 'corretto@latest')")]
    Install {
        #[arg(
            default_value = "latest",
            help = "JDK specification e.g. '21', 'latest', 'lts', 'corretto@21'"
        )]
        spec: String,

        #[arg(
            short,
            long,
            help = "JDK distribution: temurin, openjdk, corretto, zulu, microsoft, semeru, graalvm, liberica, oracle, sapmachine"
        )]
        distro: Option<JdkDistribution>,
    },

    #[command(about = "Set the active Java version for project or globally")]
    Use {
        #[arg(help = "JDK specification e.g. '21', 'latest', 'temurin@21'")]
        spec: String,

        #[arg(
            short,
            long,
            help = "JDK distribution: temurin, openjdk, corretto, zulu, microsoft, semeru, graalvm, liberica, oracle, sapmachine"
        )]
        distro: Option<JdkDistribution>,

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

struct CliProjectOptions {
    is_init: bool,
    current_dir: PathBuf,
    name: Option<String>,
    template: Option<String>,
    build_tool: Option<String>,
    group: Option<String>,
    java: Option<String>,
    distro: Option<JdkDistribution>,
    yes: bool,
}

fn resolve_project_options(
    opts: CliProjectOptions,
) -> Result<(
    String,
    djuke_templates::TemplateType,
    djuke_templates::BuildToolChoice,
    String,
    String,
    Option<JdkDistribution>,
)> {
    let is_interactive = !opts.yes && std::io::stdin().is_terminal();
    let theme = dialoguer::theme::ColorfulTheme::default();

    // 1. Resolve Project Name
    let name = match opts.name {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => {
            if is_interactive {
                let default_name = if opts.is_init {
                    opts.current_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("app")
                        .to_string()
                } else {
                    "my-app".to_string()
                };
                dialoguer::Input::with_theme(&theme)
                    .with_prompt("Project name")
                    .default(default_name)
                    .interact_text()
                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?
            } else if opts.is_init {
                opts.current_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("app")
                    .to_string()
            } else {
                "my-app".to_string()
            }
        }
    };

    // 2. Resolve Template
    let template = match opts.template {
        Some(t) => t.parse()?,
        None => {
            if is_interactive {
                let templates = [
                    "CLI Application        - Minimal Java console app with Main class (Java 8 - 27+)",
                    "Library                - Reusable Java library with JUnit 5 test suite (Java 8 - 27+)",
                    "Spring Boot Web API    - Full-stack web application with Spring Boot 3.4 & Actuator (Java 17 - 25)",
                    "Quarkus REST API       - Supersonic Subatomic reactive microservice (Java 17 - 23)",
                    "Micronaut Microservice - Cloud-native modular Java application (Java 17 - 23)",
                ];
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select project template")
                    .items(&templates)
                    .default(0)
                    .interact()
                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                match idx {
                    0 => djuke_templates::TemplateType::Cli,
                    1 => djuke_templates::TemplateType::Lib,
                    2 => djuke_templates::TemplateType::SpringBoot,
                    3 => djuke_templates::TemplateType::Quarkus,
                    4 => djuke_templates::TemplateType::Micronaut,
                    _ => djuke_templates::TemplateType::Cli,
                }
            } else {
                djuke_templates::TemplateType::Cli
            }
        }
    };

    // 3. Resolve Build Tool
    let build_tool = match opts.build_tool {
        Some(b) => b.parse()?,
        None => {
            if is_interactive {
                let tools = [
                    "Gradle (Kotlin DSL - build.gradle.kts)   [Latest Gradle 9.9 - Recommended]",
                    "Gradle (Groovy DSL - build.gradle)       [Latest Gradle 9.9 - Classic syntax]",
                    "Maven (pom.xml)                          [Latest Maven 3.9 - Standard XML]",
                ];
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select build tool")
                    .items(&tools)
                    .default(0)
                    .interact()
                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                match idx {
                    0 => djuke_templates::BuildToolChoice::GradleKts,
                    1 => djuke_templates::BuildToolChoice::GradleGroovy,
                    2 => djuke_templates::BuildToolChoice::Maven,
                    _ => djuke_templates::BuildToolChoice::GradleKts,
                }
            } else {
                djuke_templates::BuildToolChoice::GradleKts
            }
        }
    };

    // 4. Resolve Java Version (Richer, clearer, more accessible options)
    let java = match opts.java {
        Some(j) => j,
        None => {
            if is_interactive {
                let versions = [
                    "21 (LTS) - Recommended: Current production standard, maximum library compatibility",
                    "25 (LTS) - Modern standard: Next-generation long-term support",
                    "23 (GA)  - Recent feature release: Virtual threads & modern language features",
                    "17 (LTS) - Enterprise LTS: Broad compatibility for established projects",
                    "27 (EA)  - Next-Gen / Bleeding edge: Early access language features",
                    "11 (LTS) - Legacy enterprise: Older enterprise maintenance",
                    "8  (LTS) - Legacy baseline: Java 1.8 compatibility",
                    "Custom version... (type any version, e.g. 24, 22, 26)",
                ];
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select target Java version")
                    .items(&versions)
                    .default(0)
                    .interact()
                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                match idx {
                    0 => "21".to_string(),
                    1 => "25".to_string(),
                    2 => "23".to_string(),
                    3 => "17".to_string(),
                    4 => "27".to_string(),
                    5 => "11".to_string(),
                    6 => "8".to_string(),
                    7 => dialoguer::Input::with_theme(&theme)
                        .with_prompt("Enter target Java version")
                        .default("21".to_string())
                        .interact_text()
                        .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?,
                    _ => "21".to_string(),
                }
            } else {
                "21".to_string()
            }
        }
    };

    // 5. Resolve Group ID / Package
    let group = match opts.group {
        Some(g) if !g.trim().is_empty() => g.trim().to_string(),
        _ => {
            let sanitized_name: String = name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '_'
                    }
                })
                .collect();
            let default_group = format!("com.example.{}", sanitized_name);
            if is_interactive {
                dialoguer::Input::with_theme(&theme)
                    .with_prompt("Root package / Group ID")
                    .default(default_group)
                    .interact_text()
                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?
            } else {
                "com.example".to_string()
            }
        }
    };

    // 6. Resolve JDK Distribution / Flavour
    let distro = match opts.distro {
        Some(d) => Some(d),
        None => {
            if is_interactive {
                let distros = [
                    "Eclipse Temurin       [Default - Adoptium open-source build, recommended]",
                    "OpenJDK (Oracle)      [Official reference openjdk from jdk.java.net]",
                    "Amazon Corretto       [Production-ready multiplatform distribution from AWS]",
                    "Azul Zulu             [Certified TCK-tested OpenJDK builds]",
                    "Microsoft OpenJDK     [Optimized for Azure and general cloud deployments]",
                    "GraalVM Community     [High-performance runtime with native-image compilation]",
                    "BellSoft Liberica     [Recommended by Spring Boot, supports full desktop/cloud]",
                    "Oracle JDK            [Official Oracle commercial/personal distribution]",
                    "SAP SapMachine        [Enterprise OpenJDK maintained by SAP]",
                    "IBM Semeru (OpenJ9)   [Low memory footprint JVM powered by Eclipse OpenJ9]",
                ];
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select JDK distribution (flavour)")
                    .items(&distros)
                    .default(0)
                    .interact()
                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                match idx {
                    0 => Some(JdkDistribution::Temurin),
                    1 => Some(JdkDistribution::OpenJdk),
                    2 => Some(JdkDistribution::Corretto),
                    3 => Some(JdkDistribution::Zulu),
                    4 => Some(JdkDistribution::Microsoft),
                    5 => Some(JdkDistribution::Graalvm),
                    6 => Some(JdkDistribution::Liberica),
                    7 => Some(JdkDistribution::Oracle),
                    8 => Some(JdkDistribution::Sapmachine),
                    9 => Some(JdkDistribution::Semeru),
                    _ => Some(JdkDistribution::Temurin),
                }
            } else {
                Some(JdkDistribution::Temurin)
            }
        }
    };

    Ok((name, template, build_tool, group, java, distro))
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
            distro,
            yes,
        } => {
            let (proj_name, t_type, b_tool, group_id, java_ver, distro_choice) =
                resolve_project_options(CliProjectOptions {
                    is_init: false,
                    current_dir: current_dir.clone(),
                    name,
                    template,
                    build_tool,
                    group,
                    java,
                    distro,
                    yes,
                })?;

            let dest = current_dir.join(&proj_name);
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

            println!(
                "{}",
                format!("Creating new Java {} project '{}'...", t_type, proj_name)
                    .bold()
                    .cyan()
            );

            let opts = djuke_templates::NewProjectOptions {
                name: proj_name.clone(),
                group_id,
                template: t_type,
                build_tool: b_tool,
                java_version: java_ver,
                distribution: distro_choice,
                destination: dest.clone(),
            };

            let java_str = opts.java_version.clone();
            djuke_templates::generate_project(&opts)?;

            match b_tool {
                djuke_templates::BuildToolChoice::GradleKts
                | djuke_templates::BuildToolChoice::GradleGroovy => {
                    let g_ver = djuke_build::compatible_gradle_version_for_java(&java_str);
                    let _ = djuke_build::create_gradle_wrapper(&dest, Some(g_ver));
                }
                djuke_templates::BuildToolChoice::Maven => {
                    let _ = djuke_build::create_maven_wrapper(&dest, None);
                }
            }

            println!("{}", "✔ Project initialized successfully!".green().bold());
            println!("\nNext steps:");
            println!("  cd {}", proj_name.bold());
            println!("  djuke setup");
            println!("  djuke run\n");
        }

        Commands::Init {
            name,
            template,
            build_tool,
            group,
            java,
            distro,
            yes,
        } => {
            let (proj_name, t_type, b_tool, group_id, java_ver, distro_choice) =
                resolve_project_options(CliProjectOptions {
                    is_init: true,
                    current_dir: current_dir.clone(),
                    name,
                    template,
                    build_tool,
                    group,
                    java,
                    distro,
                    yes,
                })?;

            println!(
                "{}",
                format!(
                    "Initializing Java {} project '{}' in current directory...",
                    t_type, proj_name
                )
                .bold()
                .cyan()
            );

            let opts = djuke_templates::NewProjectOptions {
                name: proj_name.clone(),
                group_id,
                template: t_type,
                build_tool: b_tool,
                java_version: java_ver,
                distribution: distro_choice,
                destination: current_dir.clone(),
            };

            let java_str = opts.java_version.clone();
            djuke_templates::generate_project(&opts)?;

            match b_tool {
                djuke_templates::BuildToolChoice::GradleKts
                | djuke_templates::BuildToolChoice::GradleGroovy => {
                    let g_ver = djuke_build::compatible_gradle_version_for_java(&java_str);
                    let _ = djuke_build::create_gradle_wrapper(&current_dir, Some(g_ver));
                }
                djuke_templates::BuildToolChoice::Maven => {
                    let _ = djuke_build::create_maven_wrapper(&current_dir, None);
                }
            }

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
            JavaSubcommands::List {
                remote,
                lts,
                distro,
            } => {
                let store = JdkStore::new(paths.clone());
                if remote {
                    let disco = DiscoClient::new();
                    let platform = Platform::current();
                    let query_distro = distro.unwrap_or(JdkDistribution::Temurin);

                    println!(
                        "{}",
                        format!(
                            "Available remote JDKs for {} ({}) via Foojay Disco API:",
                            platform, query_distro
                        )
                        .bold()
                        .cyan()
                    );
                    println!(
                        "{}",
                        "--------------------------------------------------------".cyan()
                    );

                    let major_versions = disco.fetch_major_versions().await.unwrap_or_default();
                    let mut candidates: Vec<DiscoMajorVersion> = major_versions
                        .into_iter()
                        .filter(|v| {
                            v.release_status.as_deref() == Some("ga")
                                && !v.early_access_only.unwrap_or(false)
                                && (!lts || v.term_of_support.as_deref() == Some("LTS"))
                        })
                        .collect();
                    candidates.sort_by_key(|a| std::cmp::Reverse(a.major_version));

                    for mv in candidates.into_iter().take(10) {
                        let ver_str = mv.major_version.to_string();
                        if let Ok(packages) =
                            disco.query_packages(&ver_str, query_distro, platform).await
                        {
                            if let Some(pkg) = packages.into_iter().next() {
                                let java_ver = pkg.java_version.unwrap_or_else(|| ver_str.clone());
                                let support = mv.term_of_support.as_deref().unwrap_or("STS");
                                println!(
                                    "  {}@{} ({})",
                                    query_distro.to_string().green(),
                                    java_ver.bold(),
                                    support.yellow()
                                );
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

            JavaSubcommands::Install { spec, distro } => {
                let mut parsed_spec: JdkSpec = spec.parse()?;
                if let Some(d) = distro {
                    parsed_spec.distribution = d;
                }
                let installed = install_jdk(&parsed_spec, &paths, cli.quiet).await?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&installed)
                            .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?
                    );
                }
            }

            JavaSubcommands::Use {
                spec,
                distro,
                global,
            } => {
                let mut parsed_spec: JdkSpec = spec.parse()?;
                if let Some(d) = distro {
                    parsed_spec.distribution = d;
                }
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

        Commands::Setup { tool, distro } => {
            let mut config = resolve_project_config_with_tool(&current_dir, None, tool, &paths);
            if let Some(d) = distro {
                config.jdk_spec.distribution = d;
            }
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
            if let Some(t) = config.build_tool {
                let has_wrapper = config.has_gradle_wrapper || config.has_maven_wrapper;
                if !has_wrapper {
                    if !cli.quiet {
                        println!("No wrapper detected. Bootstrapping {} wrapper...", t);
                    }
                    let g_ver =
                        djuke_build::compatible_gradle_version_for_java(&config.jdk_spec.version);
                    if let Err(e) = djuke_build::bootstrap_wrapper(
                        t,
                        Some(g_ver),
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
            Some(GradleSubcommands::Wrapper { version }) => {
                let config = resolve_project_config_with_tool(
                    &current_dir,
                    None,
                    Some(BuildTool::Gradle),
                    &paths,
                );
                let v = version.as_deref().unwrap_or_else(|| {
                    djuke_build::compatible_gradle_version_for_java(&config.jdk_spec.version)
                });
                djuke_build::create_gradle_wrapper(&config.project_root, Some(v))?;
                if !cli.quiet {
                    println!(
                        "{}",
                        "✔ Gradle wrapper created successfully.".green().bold()
                    );
                }
            }
            Some(GradleSubcommands::Stop) => {
                run_raw_tool(
                    BuildTool::Gradle,
                    &["--stop".into()],
                    &current_dir,
                    &paths,
                    cli.quiet,
                )
                .await?;
            }
            Some(GradleSubcommands::Status) => {
                run_raw_tool(
                    BuildTool::Gradle,
                    &["--status".into()],
                    &current_dir,
                    &paths,
                    cli.quiet,
                )
                .await?;
            }
            None => {
                if gradle_args.args.is_empty() {
                    run_raw_tool(
                        BuildTool::Gradle,
                        &["--help".into()],
                        &current_dir,
                        &paths,
                        cli.quiet,
                    )
                    .await?;
                } else {
                    run_raw_tool(
                        BuildTool::Gradle,
                        &gradle_args.args,
                        &current_dir,
                        &paths,
                        cli.quiet,
                    )
                    .await?;
                }
            }
        },

        Commands::Maven(maven_args) => match maven_args.command {
            Some(MavenSubcommands::Wrapper { version }) => {
                let config = resolve_project_config_with_tool(
                    &current_dir,
                    None,
                    Some(BuildTool::Maven),
                    &paths,
                );
                djuke_build::create_maven_wrapper(&config.project_root, version.as_deref())?;
                if !cli.quiet {
                    println!("{}", "✔ Maven wrapper created successfully.".green().bold());
                }
            }
            None => {
                if maven_args.args.is_empty() {
                    run_raw_tool(
                        BuildTool::Maven,
                        &["--version".into()],
                        &current_dir,
                        &paths,
                        cli.quiet,
                    )
                    .await?;
                } else {
                    run_raw_tool(
                        BuildTool::Maven,
                        &maven_args.args,
                        &current_dir,
                        &paths,
                        cli.quiet,
                    )
                    .await?;
                }
            }
        },

        Commands::Build(build_args) => {
            let tool =
                resolve_project_config_with_tool(&current_dir, None, build_args.tool, &paths)
                    .build_tool
                    .unwrap_or(BuildTool::Gradle);
            let tool_args = build_args.clone().into_tool_args(tool);
            run_task(
                "build",
                tool_args,
                &current_dir,
                &paths,
                cli.quiet,
                build_args.tool,
            )
            .await?;
        }

        Commands::Test(test_args) => {
            let tool = resolve_project_config_with_tool(&current_dir, None, test_args.tool, &paths)
                .build_tool
                .unwrap_or(BuildTool::Gradle);
            let tool_args = test_args.clone().into_tool_args(tool);
            run_task(
                "test",
                tool_args,
                &current_dir,
                &paths,
                cli.quiet,
                test_args.tool,
            )
            .await?;
        }

        Commands::Run(run_args) => {
            // Check if first argument is a single .java source file (JEP 330 launcher)
            if let Some(first_arg) = run_args.args.first() {
                if first_arg.ends_with(".java") {
                    let file_path = PathBuf::from(first_arg);
                    if file_path.is_file() {
                        let config = resolve_project_config_with_tool(
                            &current_dir,
                            None,
                            run_args.tool,
                            &paths,
                        );
                        let installed_jdk =
                            resolve_jdk(&config.jdk_spec, &paths, true, cli.quiet).await?;
                        let java_bin = DjukePaths::java_executable(&installed_jdk.java_home);
                        let bin_str = java_bin.to_string_lossy();
                        let arg_slices: Vec<&str> =
                            run_args.args.iter().map(|s| s.as_str()).collect();
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

            let tool = resolve_project_config_with_tool(&current_dir, None, run_args.tool, &paths)
                .build_tool
                .unwrap_or(BuildTool::Gradle);
            let tool_args = run_args.clone().into_tool_args(tool);
            run_task(
                "run",
                tool_args,
                &current_dir,
                &paths,
                cli.quiet,
                run_args.tool,
            )
            .await?;
        }

        Commands::Dev(dev_args) => {
            let tool = resolve_project_config_with_tool(&current_dir, None, dev_args.tool, &paths)
                .build_tool
                .unwrap_or(BuildTool::Gradle);
            let tool_args = dev_args.clone().into_tool_args(tool);
            run_task(
                "dev",
                tool_args,
                &current_dir,
                &paths,
                cli.quiet,
                dev_args.tool,
            )
            .await?;
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

        Commands::Clean(clean_args) => {
            let tool =
                resolve_project_config_with_tool(&current_dir, None, clean_args.tool, &paths)
                    .build_tool
                    .unwrap_or(BuildTool::Gradle);
            let tool_args = clean_args.clone().into_tool_args(tool);
            run_task(
                "clean",
                tool_args,
                &current_dir,
                &paths,
                cli.quiet,
                clean_args.tool,
            )
            .await?;
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

        Commands::External(args) => {
            if args.is_empty() {
                Cli::command().print_help().ok();
                println!();
                return Ok(());
            }
            let task_name = &args[0];
            let task_args = args[1..].to_vec();
            run_task(task_name, task_args, &current_dir, &paths, cli.quiet, None).await?;
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

async fn run_raw_tool(
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
