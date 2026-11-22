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

mod updater;

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
            help = "Template type: cli, lib, spring-boot, quarkus, micronaut, javalin, grpc, picocli, multi-module"
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
            long = "test-framework",
            help = "Testing framework: junit5, assertj, spock, testng, none"
        )]
        test_framework: Option<djuke_templates::TestFramework>,

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
            help = "Template type: cli, lib, spring-boot, quarkus, micronaut, javalin, grpc, picocli, multi-module"
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
            long = "test-framework",
            help = "Testing framework: junit5, assertj, spock, testng, none"
        )]
        test_framework: Option<djuke_templates::TestFramework>,

        #[arg(
            short = 'y',
            long = "yes",
            help = "Skip interactive prompts and use defaults"
        )]
        yes: bool,
    },

    #[command(about = "Set the active Java version for project or globally")]
    Use {
        #[arg(help = "JDK specification e.g. '21', 'latest', 'temurin@21', 'openjdk@21'")]
        spec: Option<String>,

        #[arg(
            short = 'd',
            long = "distro",
            help = "JDK distribution: temurin, openjdk, corretto, zulu, microsoft, semeru, graalvm, liberica, oracle, sapmachine"
        )]
        distro: Option<JdkDistribution>,

        #[arg(
            long,
            help = "Set as global default instead of modifying local project"
        )]
        global: bool,
    },

    #[command(about = "Add a dependency to the current project (Maven or Gradle)")]
    Add {
        #[arg(
            help = "Dependency coordinate (e.g. 'org.postgresql:postgresql', 'guava', 'lombok')"
        )]
        dependency: Option<String>,

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
        dependency: Option<String>,
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

    #[command(about = "Upgrade djuke to the latest release directly from GitHub")]
    Upgrade {
        #[arg(long, help = "Check for updates without downloading or installing")]
        check: bool,

        #[arg(short, long, help = "Force re-download even if already up to date")]
        force: bool,
    },

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
        #[arg(
            long,
            help = "Gradle version to generate (default: latest online release)"
        )]
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
        #[arg(
            long,
            help = "Maven version to generate (default: latest online release)"
        )]
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
        spec: Option<String>,

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

    #[command(about = "View or configure the global default Java version and distribution")]
    Default {
        #[arg(
            help = "JDK specification to set as default, e.g. '21', '25', 'temurin@21', 'corretto@latest'"
        )]
        spec: Option<String>,

        #[arg(
            short = 'd',
            long = "distro",
            help = "JDK distribution: temurin, openjdk, corretto, zulu, microsoft, semeru, graalvm, liberica, oracle, sapmachine"
        )]
        distro: Option<JdkDistribution>,
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
    test_framework: Option<djuke_templates::TestFramework>,
    yes: bool,
}

struct ResolvedProjectInitOptions {
    name: String,
    template: djuke_templates::TemplateType,
    build_tool: djuke_templates::BuildToolChoice,
    group_id: String,
    java_ver: String,
    distro_choice: Option<JdkDistribution>,
    initializr_deps: Option<Vec<String>>,
    test_framework: djuke_templates::TestFramework,
}

async fn resolve_project_options(opts: CliProjectOptions) -> Result<ResolvedProjectInitOptions> {
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
                    "Spring Boot Web API    - Full-stack web app with Spring Boot (Spring Initializr / Java 17 - 27)",
                    "Quarkus REST API       - Supersonic Subatomic reactive microservice (Java 17 - 23)",
                    "Micronaut Microservice - Cloud-native modular Java application (Java 17 - 23)",
                    "Javalin REST API       - Lightweight, ultra-fast embedded REST API with Jetty (Java 11 - 27+)",
                    "gRPC Microservice      - High-performance Protocol Buffers & gRPC service (Java 11 - 27+)",
                    "Picocli CLI            - Production command-line app with annotations & colors (Java 8 - 27+)",
                    "Multi-Module Project   - Enterprise modular architecture (core library + api service) (Java 8 - 27+)",
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
                    5 => djuke_templates::TemplateType::Javalin,
                    6 => djuke_templates::TemplateType::Grpc,
                    7 => djuke_templates::TemplateType::Picocli,
                    8 => djuke_templates::TemplateType::MultiModule,
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

    // 4. Resolve JDK Distribution / Flavour early
    let distro = match opts.distro {
        Some(d) => Some(d),
        None => {
            if is_interactive {
                let distros = [
                    "OpenJDK (Oracle)      [Default - Official reference openjdk from jdk.java.net]",
                    "Eclipse Temurin       [Adoptium open-source build, recommended for enterprise]",
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
                    0 => Some(JdkDistribution::OpenJdk),
                    1 => Some(JdkDistribution::Temurin),
                    2 => Some(JdkDistribution::Corretto),
                    3 => Some(JdkDistribution::Zulu),
                    4 => Some(JdkDistribution::Microsoft),
                    5 => Some(JdkDistribution::Graalvm),
                    6 => Some(JdkDistribution::Liberica),
                    7 => Some(JdkDistribution::Oracle),
                    8 => Some(JdkDistribution::Sapmachine),
                    9 => Some(JdkDistribution::Semeru),
                    _ => Some(JdkDistribution::OpenJdk),
                }
            } else {
                Some(JdkDistribution::default_distro())
            }
        }
    };

    let selected_distro = distro.unwrap_or_else(JdkDistribution::default_distro);

    // 5. Resolve Java Version (with installed-status indicator under chosen flavour)
    let java = match opts.java {
        Some(j) => j,
        None => {
            if is_interactive {
                let store = djuke_jdk::JdkStore::new(DjukePaths::discover());
                let installed_jdks = store.list_installed().unwrap_or_default();
                let is_downloaded = |ver: &str| -> bool {
                    installed_jdks.iter().any(|i| {
                        i.distribution == selected_distro
                            && (i.version == ver
                                || i.version.starts_with(ver)
                                || (ver == "8" && i.version.starts_with("1.8")))
                    })
                };

                let v21_label = if is_downloaded("21") {
                    "21 (LTS) [Installed locally ✔] - Recommended: Current production standard"
                        .to_string()
                } else {
                    "21 (LTS)                       - Recommended: Current production standard"
                        .to_string()
                };
                let v25_label = if is_downloaded("25") {
                    "25 (LTS) [Installed locally ✔] - Modern standard: Next-generation long-term support"
                        .to_string()
                } else {
                    "25 (LTS)                       - Modern standard: Next-generation long-term support"
                        .to_string()
                };
                let v23_label = if is_downloaded("23") {
                    "23 (GA)  [Installed locally ✔] - Recent feature release: Virtual threads & modern features"
                        .to_string()
                } else {
                    "23 (GA)                        - Recent feature release: Virtual threads & modern features"
                        .to_string()
                };
                let v17_label = if is_downloaded("17") {
                    "17 (LTS) [Installed locally ✔] - Enterprise LTS: Broad compatibility for established projects"
                        .to_string()
                } else {
                    "17 (LTS)                       - Enterprise LTS: Broad compatibility for established projects"
                        .to_string()
                };
                let v27_label = if is_downloaded("27") {
                    "27 (EA)  [Installed locally ✔] - Next-Gen / Bleeding edge: Early access language features"
                        .to_string()
                } else {
                    "27 (EA)                        - Next-Gen / Bleeding edge: Early access language features"
                        .to_string()
                };
                let v11_label = if is_downloaded("11") {
                    "11 (LTS) [Installed locally ✔] - Legacy enterprise: Older enterprise maintenance"
                        .to_string()
                } else {
                    "11 (LTS)                       - Legacy enterprise: Older enterprise maintenance"
                        .to_string()
                };
                let v8_label = if is_downloaded("8") {
                    "8  (LTS) [Installed locally ✔] - Legacy baseline: Java 1.8 compatibility"
                        .to_string()
                } else {
                    "8  (LTS)                       - Legacy baseline: Java 1.8 compatibility"
                        .to_string()
                };

                let versions = [
                    v21_label.as_str(),
                    v25_label.as_str(),
                    v23_label.as_str(),
                    v17_label.as_str(),
                    v27_label.as_str(),
                    v11_label.as_str(),
                    v8_label.as_str(),
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

    // 6. Resolve Testing Framework (let users choose / skip)
    let test_framework = match opts.test_framework {
        Some(tf) => tf,
        None => {
            if is_interactive {
                let frameworks = [
                    "JUnit 5 (Jupiter) - Modern standard testing framework for Java [Recommended]",
                    "AssertJ + JUnit 5 - Fluent, rich assertions with expressive test diagnostics",
                    "Spock Framework   - Expressive BDD-style testing with Groovy syntax",
                    "TestNG            - Feature-rich enterprise test framework with data providers",
                    "None              - Skip test library and do not generate test files",
                ];
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select testing framework")
                    .items(&frameworks)
                    .default(0)
                    .interact()
                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                match idx {
                    0 => djuke_templates::TestFramework::JUnit5,
                    1 => djuke_templates::TestFramework::AssertJ,
                    2 => djuke_templates::TestFramework::Spock,
                    3 => djuke_templates::TestFramework::TestNG,
                    4 => djuke_templates::TestFramework::None,
                    _ => djuke_templates::TestFramework::JUnit5,
                }
            } else {
                djuke_templates::TestFramework::JUnit5
            }
        }
    };

    // 7. Resolve Group ID / Package
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

    // 8. Resolve Spring Boot dependencies if applicable
    let initializr_deps = if template == djuke_templates::TemplateType::SpringBoot {
        if is_interactive {
            let popular = djuke_templates::SpringInitializrClient::popular_dependencies();
            let labels: Vec<String> = popular
                .iter()
                .map(|d| {
                    let desc = d.description.as_deref().unwrap_or("");
                    format!("{:<15} - {:<24} [{}]", d.id, format!("({})", d.name), desc)
                })
                .collect();
            let default_flags: Vec<bool> = popular
                .iter()
                .map(|d| d.id == "web" || d.id == "devtools" || d.id == "actuator")
                .collect();
            let chosen_indices = dialoguer::MultiSelect::with_theme(&theme)
                .with_prompt("Select Spring Boot dependencies (Space to toggle, Enter to confirm)")
                .items(&labels)
                .defaults(&default_flags)
                .interact()
                .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;

            let mut selected_ids: Vec<String> = if chosen_indices.is_empty() {
                vec!["web".into(), "devtools".into(), "actuator".into()]
            } else {
                chosen_indices
                    .into_iter()
                    .map(|i| popular[i].id.clone())
                    .collect()
            };

            let expand = dialoguer::Confirm::with_theme(&theme)
                .with_prompt("Browse more dependencies from full Initializr catalog?")
                .default(false)
                .interact()
                .unwrap_or(false);

            if expand {
                let client = djuke_templates::SpringInitializrClient::new();
                let all_deps = client.fetch_dependencies().await;
                let remaining: Vec<_> = all_deps
                    .into_iter()
                    .filter(|d| !popular.iter().any(|p| p.id == d.id))
                    .collect();
                if !remaining.is_empty() {
                    let rem_labels: Vec<String> = remaining
                        .iter()
                        .map(|d| {
                            let desc = d.description.as_deref().unwrap_or("");
                            let short_desc = if desc.len() > 40 {
                                format!("{}...", &desc[..37])
                            } else {
                                desc.to_string()
                            };
                            format!(
                                "{:<15} - {:<24} [{}]",
                                d.id,
                                format!("({})", d.name),
                                short_desc
                            )
                        })
                        .collect();
                    let default_rem_flags = vec![false; remaining.len()];
                    let rem_chosen = dialoguer::MultiSelect::with_theme(&theme)
                        .with_prompt(
                            "Select additional dependencies (Space to toggle, Enter to confirm)",
                        )
                        .items(&rem_labels)
                        .defaults(&default_rem_flags)
                        .interact()
                        .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                    for idx in rem_chosen {
                        selected_ids.push(remaining[idx].id.clone());
                    }
                }
            }

            Some(selected_ids)
        } else {
            Some(vec!["web".into(), "devtools".into(), "actuator".into()])
        }
    } else {
        None
    };

    Ok(ResolvedProjectInitOptions {
        name,
        template,
        build_tool,
        group_id: group,
        java_ver: java,
        distro_choice: distro,
        initializr_deps,
        test_framework,
    })
}

async fn handle_use_command(
    spec: Option<String>,
    distro: Option<JdkDistribution>,
    global: bool,
    paths: &DjukePaths,
    current_dir: &Path,
    quiet: bool,
) -> Result<()> {
    let manifest_file = current_dir.join("djuke.toml");
    let has_project_indicator = manifest_file.is_file()
        || current_dir.join("pom.xml").is_file()
        || current_dir.join("build.gradle").is_file()
        || current_dir.join("build.gradle.kts").is_file();

    let target_global = global || !has_project_indicator;

    let theme = dialoguer::theme::ColorfulTheme::default();
    let is_interactive = std::io::stdin().is_terminal();

    let store = JdkStore::new(paths.clone());
    let installed = store.list_installed()?;

    let (chosen_spec, chosen_distro) = match spec {
        Some(s) => (s, distro),
        None => {
            if is_interactive {
                if installed.is_empty() {
                    println!("{}", "No JDKs currently installed locally.".yellow().bold());
                    let install_options = [
                        "Download & install OpenJDK (Oracle) 21 [Default - Official reference openjdk build]",
                        "Download & install Eclipse Temurin 21 (LTS) [Adoptium production standard]",
                        "Download & install Eclipse Temurin 25 (LTS) [Next-gen long-term support]",
                        "Download & install Eclipse Temurin 17 (LTS) [Enterprise legacy LTS]",
                        "Custom version & distribution...",
                    ];
                    let idx = dialoguer::Select::with_theme(&theme)
                        .with_prompt("Select a JDK to download, install, and set as default")
                        .items(&install_options)
                        .default(0)
                        .interact()
                        .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;

                    match idx {
                        0 => ("21".to_string(), Some(JdkDistribution::OpenJdk)),
                        1 => ("21".to_string(), Some(JdkDistribution::Temurin)),
                        2 => ("25".to_string(), Some(JdkDistribution::Temurin)),
                        3 => ("17".to_string(), Some(JdkDistribution::Temurin)),
                        4 => {
                            let ver: String = dialoguer::Input::with_theme(&theme)
                                .with_prompt("Target Java version (e.g. 21, 23, 25)")
                                .default("21".to_string())
                                .interact_text()
                                .map_err(|e| {
                                    DjukeError::Process(format!("Prompt failed: {}", e))
                                })?;
                            (ver, distro.or(Some(JdkDistribution::default_distro())))
                        }
                        _ => ("21".to_string(), Some(JdkDistribution::default_distro())),
                    }
                } else {
                    let mut items: Vec<String> = installed
                        .iter()
                        .map(|i| {
                            format!(
                                "{:<20} ({}) -> {}",
                                format!("{}@{}", i.distribution, i.version).green(),
                                i.major_version,
                                i.java_home.display()
                            )
                        })
                        .collect();
                    items.push(
                        "+ Download & install a new JDK runtime..."
                            .cyan()
                            .to_string(),
                    );

                    let idx = dialoguer::Select::with_theme(&theme)
                        .with_prompt("Select JDK runtime to activate")
                        .items(&items)
                        .default(0)
                        .interact()
                        .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;

                    if idx < installed.len() {
                        let chosen = &installed[idx];
                        (chosen.version.clone(), Some(chosen.distribution))
                    } else {
                        let ver: String = dialoguer::Input::with_theme(&theme)
                            .with_prompt(
                                "Enter Java version to download & install (e.g. 21, 23, 25)",
                            )
                            .default("21".to_string())
                            .interact_text()
                            .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                        (ver, distro.or(Some(JdkDistribution::default_distro())))
                    }
                }
            } else if !installed.is_empty() {
                let chosen = &installed[0];
                (chosen.version.clone(), Some(chosen.distribution))
            } else {
                return Err(DjukeError::Generic(
                    "No JDKs installed locally. Run 'djuke use 21' or 'djuke java install 21'."
                        .into(),
                )
                .into());
            }
        }
    };

    let mut parsed_spec: JdkSpec = chosen_spec.parse()?;
    if let Some(d) = chosen_distro {
        parsed_spec.distribution = d;
    }

    let installed_jdk = resolve_jdk(&parsed_spec, paths, true, quiet).await?;

    if target_global {
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
    println!("  JAVA_HOME: {}", installed_jdk.java_home.display());

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    djuke_process::init_terminal_safety();

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
            test_framework,
            yes,
        } => {
            let init_opts = resolve_project_options(CliProjectOptions {
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
            })
            .await?;

            let dest = current_dir.join(&init_opts.name);
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
                format!(
                    "Creating new Java {} project '{}'...",
                    init_opts.template, init_opts.name
                )
                .bold()
                .cyan()
            );

            let resolved_java = if init_opts.java_ver.eq_ignore_ascii_case("latest")
                || init_opts.java_ver.eq_ignore_ascii_case("lts")
            {
                let disco = djuke_jdk::DiscoClient::new();
                disco.resolve_version_alias(&init_opts.java_ver).await
            } else {
                init_opts.java_ver
            };

            let framework_version = match init_opts.template {
                djuke_templates::TemplateType::SpringBoot => {
                    let central = djuke_pm::MavenCentralClient::new();
                    central
                        .resolve_latest_version(
                            "org.springframework.boot",
                            "spring-boot-starter-parent",
                        )
                        .await
                        .ok()
                }
                djuke_templates::TemplateType::Quarkus => {
                    let central = djuke_pm::MavenCentralClient::new();
                    central
                        .resolve_latest_version("io.quarkus.platform", "quarkus-bom")
                        .await
                        .ok()
                }
                djuke_templates::TemplateType::Micronaut => {
                    let central = djuke_pm::MavenCentralClient::new();
                    central
                        .resolve_latest_version("io.micronaut.platform", "micronaut-parent")
                        .await
                        .ok()
                }
                _ => None,
            };

            let opts = djuke_templates::NewProjectOptions {
                name: init_opts.name.clone(),
                group_id: init_opts.group_id,
                template: init_opts.template,
                build_tool: init_opts.build_tool,
                java_version: resolved_java,
                distribution: init_opts.distro_choice,
                destination: dest.clone(),
                framework_version,
                initializr_dependencies: init_opts.initializr_deps,
                skip_initializr: false,
                test_framework: init_opts.test_framework,
            };

            let java_str = opts.java_version.clone();
            djuke_templates::generate_project(&opts).await?;

            let tool_enum = match init_opts.build_tool {
                djuke_templates::BuildToolChoice::GradleKts
                | djuke_templates::BuildToolChoice::GradleGroovy => djuke_core::BuildTool::Gradle,
                djuke_templates::BuildToolChoice::Maven => djuke_core::BuildTool::Maven,
            };

            // 1. Resolve actual latest wrapper version dynamically
            let resolved_v = match tool_enum {
                djuke_core::BuildTool::Gradle => {
                    djuke_build::resolve_dynamic_gradle_version(&java_str, Some(&paths), true).await
                }
                djuke_core::BuildTool::Maven => {
                    djuke_build::resolve_dynamic_maven_version(Some(&paths), true).await
                }
            };

            match tool_enum {
                djuke_core::BuildTool::Gradle => {
                    let _ = djuke_build::create_gradle_wrapper(&dest, Some(&resolved_v));
                }
                djuke_core::BuildTool::Maven => {
                    let _ = djuke_build::create_maven_wrapper(&dest, Some(&resolved_v));
                }
            }

            // 2. Resolve JDK and immediately prime/cache the wrapper distribution
            let jdk_spec = djuke_core::JdkSpec {
                distribution: init_opts
                    .distro_choice
                    .unwrap_or_else(djuke_core::JdkDistribution::default_distro),
                version: java_str,
            };
            if let Ok(installed_jdk) = resolve_jdk(&jdk_spec, &paths, true, cli.quiet).await {
                let _ = djuke_build::prime_wrapper(
                    tool_enum,
                    &dest,
                    &installed_jdk.java_home,
                    &paths,
                    cli.quiet,
                )
                .await;
            }

            println!(
                "{}",
                "✔ Project created and toolchain primed successfully!"
                    .green()
                    .bold()
            );
            println!("\nNext steps:");
            println!("  cd {}", init_opts.name.bold());
            println!("  djuke run\n");
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
            let init_opts = resolve_project_options(CliProjectOptions {
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
            })
            .await?;

            println!(
                "{}",
                format!(
                    "Initializing Java {} project '{}' in current directory...",
                    init_opts.template, init_opts.name
                )
                .bold()
                .cyan()
            );

            let resolved_java = if init_opts.java_ver.eq_ignore_ascii_case("latest")
                || init_opts.java_ver.eq_ignore_ascii_case("lts")
            {
                let disco = djuke_jdk::DiscoClient::new();
                disco.resolve_version_alias(&init_opts.java_ver).await
            } else {
                init_opts.java_ver
            };

            let framework_version = match init_opts.template {
                djuke_templates::TemplateType::SpringBoot => {
                    let central = djuke_pm::MavenCentralClient::new();
                    central
                        .resolve_latest_version(
                            "org.springframework.boot",
                            "spring-boot-starter-parent",
                        )
                        .await
                        .ok()
                }
                djuke_templates::TemplateType::Quarkus => {
                    let central = djuke_pm::MavenCentralClient::new();
                    central
                        .resolve_latest_version("io.quarkus.platform", "quarkus-bom")
                        .await
                        .ok()
                }
                djuke_templates::TemplateType::Micronaut => {
                    let central = djuke_pm::MavenCentralClient::new();
                    central
                        .resolve_latest_version("io.micronaut.platform", "micronaut-parent")
                        .await
                        .ok()
                }
                _ => None,
            };

            let opts = djuke_templates::NewProjectOptions {
                name: init_opts.name.clone(),
                group_id: init_opts.group_id,
                template: init_opts.template,
                build_tool: init_opts.build_tool,
                java_version: resolved_java,
                distribution: init_opts.distro_choice,
                destination: current_dir.clone(),
                framework_version,
                initializr_dependencies: init_opts.initializr_deps,
                skip_initializr: false,
                test_framework: init_opts.test_framework,
            };

            let java_str = opts.java_version.clone();
            djuke_templates::generate_project(&opts).await?;

            let tool_enum = match init_opts.build_tool {
                djuke_templates::BuildToolChoice::GradleKts
                | djuke_templates::BuildToolChoice::GradleGroovy => djuke_core::BuildTool::Gradle,
                djuke_templates::BuildToolChoice::Maven => djuke_core::BuildTool::Maven,
            };

            // 1. Resolve actual latest wrapper version dynamically
            let resolved_v = match tool_enum {
                djuke_core::BuildTool::Gradle => {
                    djuke_build::resolve_dynamic_gradle_version(&java_str, Some(&paths), true).await
                }
                djuke_core::BuildTool::Maven => {
                    djuke_build::resolve_dynamic_maven_version(Some(&paths), true).await
                }
            };

            match tool_enum {
                djuke_core::BuildTool::Gradle => {
                    let _ = djuke_build::create_gradle_wrapper(&current_dir, Some(&resolved_v));
                }
                djuke_core::BuildTool::Maven => {
                    let _ = djuke_build::create_maven_wrapper(&current_dir, Some(&resolved_v));
                }
            }

            // 2. Resolve JDK and immediately prime/cache the wrapper distribution
            let jdk_spec = djuke_core::JdkSpec {
                distribution: init_opts
                    .distro_choice
                    .unwrap_or_else(djuke_core::JdkDistribution::default_distro),
                version: java_str,
            };
            if let Ok(installed_jdk) = resolve_jdk(&jdk_spec, &paths, true, cli.quiet).await {
                let _ = djuke_build::prime_wrapper(
                    tool_enum,
                    &current_dir,
                    &installed_jdk.java_home,
                    &paths,
                    cli.quiet,
                )
                .await;
            }

            println!(
                "{}",
                "✔ Project initialized and toolchain primed successfully!"
                    .green()
                    .bold()
            );
            println!("\nNext steps:");
            println!("  djuke run\n");
        }

        Commands::Use {
            spec,
            distro,
            global,
        } => {
            handle_use_command(spec, distro, global, &paths, &current_dir, cli.quiet).await?;
        }

        Commands::Add {
            dependency,
            scope,
            test,
        } => {
            let dep_input = match dependency {
                Some(d) if !d.trim().is_empty() => d.trim().to_string(),
                _ => {
                    let is_interactive = std::io::stdin().is_terminal();
                    if is_interactive {
                        let theme = dialoguer::theme::ColorfulTheme::default();
                        let search_query: String = dialoguer::Input::with_theme(&theme)
                            .with_prompt("Search Maven Central for library to add")
                            .interact_text()
                            .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                        let central = djuke_pm::MavenCentralClient::new();
                        let docs = central.search(&search_query, 5).await?;
                        if docs.is_empty() {
                            return Err(DjukeError::Generic(format!(
                                "No artifacts found matching '{}'",
                                search_query
                            ))
                            .into());
                        }
                        let items: Vec<String> = docs
                            .iter()
                            .map(|d| {
                                format!(
                                    "{}:{} ({})",
                                    d.group_id,
                                    d.artifact_id,
                                    d.latest_version.as_deref().unwrap_or("-")
                                )
                            })
                            .collect();
                        let idx = dialoguer::Select::with_theme(&theme)
                            .with_prompt("Select dependency to add")
                            .items(&items)
                            .default(0)
                            .interact()
                            .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                        format!("{}:{}", docs[idx].group_id, docs[idx].artifact_id)
                    } else {
                        return Err(DjukeError::Generic(
                            "Please provide a dependency coordinate (e.g. 'guava' or 'org.postgresql:postgresql')".into(),
                        )
                        .into());
                    }
                }
            };

            let canonical_dep = resolve_dependency_alias(&dep_input);
            let mut spec: djuke_pm::DependencySpec = canonical_dep.parse()?;
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
            let config = resolve_project_config(&current_dir, None, &paths);
            let dep_str = match dependency {
                Some(d) if !d.trim().is_empty() => d.trim().to_string(),
                _ => {
                    let deps = djuke_pm::extract_project_dependencies(&config.project_root)?;
                    if deps.is_empty() {
                        return Err(DjukeError::Generic(
                            "No dependencies found in project to remove.".into(),
                        )
                        .into());
                    }
                    let is_interactive = std::io::stdin().is_terminal();
                    if is_interactive {
                        let theme = dialoguer::theme::ColorfulTheme::default();
                        let items: Vec<String> = deps
                            .iter()
                            .map(|d| format!("{}:{}", d.group_id, d.artifact_id))
                            .collect();
                        let idx = dialoguer::Select::with_theme(&theme)
                            .with_prompt("Select dependency to remove")
                            .items(&items)
                            .default(0)
                            .interact()
                            .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                        items[idx].clone()
                    } else {
                        return Err(DjukeError::Generic(
                            "Please provide dependency to remove".into(),
                        )
                        .into());
                    }
                }
            };

            let (group, artifact) = if dep_str.contains(':') {
                let parts: Vec<&str> = dep_str.split(':').collect();
                (parts[0].trim().to_string(), parts[1].trim().to_string())
            } else {
                let deps = djuke_pm::extract_project_dependencies(&config.project_root)?;
                let matched = deps
                    .iter()
                    .find(|d| d.artifact_id.eq_ignore_ascii_case(&dep_str));
                if let Some(m) = matched {
                    (m.group_id.clone(), m.artifact_id.clone())
                } else {
                    return Err(DjukeError::Generic(format!(
                        "Could not find dependency matching '{}' in project. Provide 'groupId:artifactId'.",
                        dep_str
                    )).into());
                }
            };

            match config.build_tool {
                Some(BuildTool::Maven) => {
                    let pom_path = config.project_root.join("pom.xml");
                    djuke_pm::remove_maven_dependency(&pom_path, &group, &artifact)?;
                    println!("✔ Removed {}:{} from pom.xml", group, artifact);
                }
                Some(BuildTool::Gradle) | None => {
                    let kts = config.project_root.join("build.gradle.kts");
                    let groovy = config.project_root.join("build.gradle");
                    let build_file = if kts.is_file() { kts } else { groovy };
                    djuke_pm::remove_gradle_dependency(&build_file, &group, &artifact)?;
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
                for doc in &docs {
                    let coord = format!("{}:{}", doc.group_id, doc.artifact_id);
                    let ver = doc.latest_version.as_deref().unwrap_or("-");
                    let pkg = doc.packaging.as_deref().unwrap_or("jar");
                    println!("{:<45} {:<15} {}", coord.green(), ver.bold(), pkg);
                }

                if let Some(top) = docs.first() {
                    let ver = top.latest_version.as_deref().unwrap_or("RELEASE");
                    println!("\n{}", "Quick Snippets:".bold().cyan());
                    println!(
                        "  {} implementation(\"{}:{}:{}\")",
                        "Gradle (Kotlin):".yellow(),
                        top.group_id,
                        top.artifact_id,
                        ver
                    );
                    println!(
                        "  {} implementation '{}:{}:{}'",
                        "Gradle (Groovy):".yellow(),
                        top.group_id,
                        top.artifact_id,
                        ver
                    );
                    println!("  {} <dependency><groupId>{}</groupId><artifactId>{}</artifactId><version>{}</version></dependency>", "Maven (pom.xml):".yellow(), top.group_id, top.artifact_id, ver);
                    println!(
                        "  {} djuke add {}:{}",
                        "djuke add:".yellow(),
                        top.group_id,
                        top.artifact_id
                    );
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
                    let query_distro = distro.unwrap_or_else(JdkDistribution::default_distro);

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

                    // Ensure key releases (27, 25, 23, 21, 17, 11, 8) are always included if applicable
                    let known_majors: &[u32] = if lts {
                        &[25, 21, 17, 11, 8]
                    } else {
                        &[27, 25, 23, 21, 17, 11, 8]
                    };
                    for &km in known_majors {
                        if !candidates.iter().any(|c| c.major_version == km) {
                            candidates.push(DiscoMajorVersion {
                                major_version: km,
                                term_of_support: if [25, 21, 17, 11, 8].contains(&km) {
                                    Some("LTS".into())
                                } else {
                                    Some("STS".into())
                                },
                                maintained: Some(true),
                                release_status: Some("ga".into()),
                                early_access_only: Some(false),
                            });
                        }
                    }
                    candidates.sort_by_key(|a| std::cmp::Reverse(a.major_version));

                    for mv in candidates.into_iter().take(12) {
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
                    let config = resolve_project_config(&current_dir, None, &paths);
                    let global_config =
                        djuke_config::global::load_global_config(&paths.config_file())
                            .unwrap_or_default();
                    let g_pref = global_config.java.as_ref().map(|j| {
                        (
                            j.default_distribution
                                .unwrap_or_else(djuke_core::JdkDistribution::default_distro),
                            j.default_version
                                .clone()
                                .unwrap_or_else(|| "21".to_string()),
                        )
                    });

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
                            let is_active = (jdk.distribution == config.jdk_spec.distribution
                                && jdk.version == config.jdk_spec.version)
                                || jdk.version.starts_with(&config.jdk_spec.version);
                            let is_global = if let Some((g_distro, g_ver)) = &g_pref {
                                (jdk.distribution == *g_distro && jdk.version == *g_ver)
                                    || jdk.version.starts_with(g_ver)
                            } else {
                                false
                            };

                            let mut badges = Vec::new();
                            if is_active {
                                badges.push("* [active: project]".green().bold().to_string());
                            }
                            if is_global {
                                badges.push("+ [default: global]".cyan().bold().to_string());
                            }
                            let badge_str = if badges.is_empty() {
                                String::new()
                            } else {
                                format!(" {}", badges.join(" "))
                            };

                            println!(
                                "  {} {} ({}){} -> {}",
                                "•".cyan(),
                                format!("{}@{}", jdk.distribution, jdk.version)
                                    .green()
                                    .bold(),
                                jdk.major_version,
                                badge_str,
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

            JavaSubcommands::Default { spec, distro } => {
                let mut cfg = djuke_config::global::load_global_config(&paths.config_file())?;
                match spec {
                    None => match &cfg.java {
                        Some(j) => {
                            let d = j
                                .default_distribution
                                .unwrap_or_else(djuke_core::JdkDistribution::default_distro);
                            let v = j
                                .default_version
                                .clone()
                                .unwrap_or_else(|| "21".to_string());
                            let source = format!("Config file ({})", paths.config_file().display());
                            let store = JdkStore::new(paths.clone());
                            let global_spec = djuke_core::JdkSpec::new(d, v.clone());
                            let installed = store.find_installed(&global_spec);

                            if cli.json {
                                let json_data = serde_json::json!({
                                    "configured": true,
                                    "distribution": d.to_string(),
                                    "version": v,
                                    "source": source,
                                    "installed": installed.is_some(),
                                    "java_home": installed.as_ref().map(|i| i.java_home.display().to_string()),
                                });
                                println!("{}", serde_json::to_string_pretty(&json_data).unwrap());
                            } else {
                                println!("{}", "Global Default Java Configuration:".bold().cyan());
                                println!(
                                    "  • Runtime:    {}",
                                    format!("{}@{}", d, v).green().bold()
                                );
                                println!("  • Source:     {}", source);
                                if let Some(i) = installed {
                                    println!(
                                        "  • Status:     {} ({})",
                                        "Installed".green().bold(),
                                        i.java_home.display()
                                    );
                                } else {
                                    println!(
                                        "  • Status:     {}",
                                        "Not installed locally".yellow().bold()
                                    );
                                    println!(
                                        "  Run '{}' to install and activate.",
                                        format!("djuke use --global {}", v).green()
                                    );
                                }
                            }
                        }
                        None => {
                            if cli.json {
                                let json_data = serde_json::json!({
                                    "configured": false,
                                    "message": "No global default Java configured. Run 'djuke use --global' to set one.",
                                });
                                println!("{}", serde_json::to_string_pretty(&json_data).unwrap());
                            } else {
                                println!("{}", "Global Default Java Configuration:".bold().cyan());
                                println!("  • Runtime:    {}", "Not configured".yellow().bold());
                                println!(
                                    "  Run '{}' to set your preferred global Java runtime.",
                                    "djuke use --global".green()
                                );
                            }
                        }
                    },
                    Some(s) => {
                        let mut parsed_spec: JdkSpec = s.parse()?;
                        if let Some(d) = distro {
                            parsed_spec.distribution = d;
                        }
                        let installed = resolve_jdk(&parsed_spec, &paths, true, cli.quiet).await?;
                        let mut j = cfg.java.unwrap_or_default();
                        j.default_version = Some(parsed_spec.version.clone());
                        j.default_distribution = Some(parsed_spec.distribution);
                        cfg.java = Some(j);
                        djuke_config::global::save_global_config(&paths.config_file(), &cfg)?;
                        println!(
                            "✔ Set global default Java to {}@{}",
                            parsed_spec.distribution, parsed_spec.version
                        );
                        println!("  JAVA_HOME: {}", installed.java_home.display());
                    }
                }
            }

            JavaSubcommands::Use {
                spec,
                distro,
                global,
            } => {
                handle_use_command(spec, distro, global, &paths, &current_dir, cli.quiet).await?;
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
                        "resolution_source": format!("{}", config.jdk_source),
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
                    println!("Resolved from: {}", format!("{}", config.jdk_source).cyan());
                } else {
                    println!(
                        "Configured Java: {} (not installed locally)",
                        format!("{}", config.jdk_spec).yellow()
                    );
                    println!("Resolved from: {}", format!("{}", config.jdk_source).cyan());
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
                    let resolved_v = match t {
                        BuildTool::Gradle => {
                            djuke_build::resolve_dynamic_gradle_version(
                                &config.jdk_spec.version,
                                Some(&paths),
                                true,
                            )
                            .await
                        }
                        BuildTool::Maven => {
                            djuke_build::resolve_dynamic_maven_version(Some(&paths), true).await
                        }
                    };
                    if let Err(e) = djuke_build::bootstrap_wrapper(
                        t,
                        Some(&resolved_v),
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
                        wrapper_status = "generated & primed";
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
                let v = match version {
                    Some(ver) => ver,
                    None => {
                        djuke_build::resolve_dynamic_gradle_version(
                            &config.jdk_spec.version,
                            Some(&paths),
                            true,
                        )
                        .await
                    }
                };
                djuke_build::create_gradle_wrapper(&config.project_root, Some(&v))?;
                if let Ok(installed_jdk) =
                    resolve_jdk(&config.jdk_spec, &paths, true, cli.quiet).await
                {
                    let _ = djuke_build::prime_wrapper(
                        BuildTool::Gradle,
                        &config.project_root,
                        &installed_jdk.java_home,
                        &paths,
                        cli.quiet,
                    )
                    .await;
                }
                if !cli.quiet {
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
                let v = match version {
                    Some(ver) => ver,
                    None => djuke_build::resolve_dynamic_maven_version(Some(&paths), true).await,
                };
                djuke_build::create_maven_wrapper(&config.project_root, Some(&v))?;
                if let Ok(installed_jdk) =
                    resolve_jdk(&config.jdk_spec, &paths, true, cli.quiet).await
                {
                    let _ = djuke_build::prime_wrapper(
                        BuildTool::Maven,
                        &config.project_root,
                        &installed_jdk.java_home,
                        &paths,
                        cli.quiet,
                    )
                    .await;
                }
                if !cli.quiet {
                    println!(
                        "{}",
                        "✔ Maven wrapper created and primed successfully."
                            .green()
                            .bold()
                    );
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
            // Check if first argument is a single .java source file (JEP 330 launcher / inline script runner)
            if let Some(first_arg) = run_args.args.first() {
                if first_arg.ends_with(".java") {
                    let file_path = PathBuf::from(first_arg);
                    if !file_path.is_file() {
                        return Err(DjukeError::Generic(format!(
                            "Java script file '{}' not found",
                            first_arg
                        ))
                        .into());
                    }
                    let remaining_args = if run_args.args.len() > 1 {
                        &run_args.args[1..]
                    } else {
                        &[]
                    };
                    return run_single_file_script(
                        &file_path,
                        remaining_args,
                        run_args.tool,
                        &paths,
                        cli.quiet,
                    )
                    .await;
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
                        println!("set -gx JAVA_HOME \"{}\"", java_home);
                        println!("set -gx PATH \"{}\" $PATH", bin_dir);
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

        Commands::Upgrade { check, force } => {
            updater::run_self_upgrade(check, force, cli.quiet).await?;
        }

        Commands::External(args) => {
            if args.is_empty() {
                Cli::command().print_help().ok();
                println!();
                return Ok(());
            }
            if args[0].ends_with(".java") {
                let file_path = PathBuf::from(&args[0]);
                if !file_path.is_file() {
                    return Err(DjukeError::Generic(format!(
                        "Java script file '{}' not found",
                        args[0]
                    ))
                    .into());
                }
                let remaining_args = if args.len() > 1 { &args[1..] } else { &[] };
                return run_single_file_script(&file_path, remaining_args, None, &paths, cli.quiet)
                    .await;
            }
            let task_name = &args[0];
            let task_args = args[1..].to_vec();
            run_task(task_name, task_args, &current_dir, &paths, cli.quiet, None).await?;
        }
    }

    Ok(())
}

async fn run_single_file_script(
    file_path: &Path,
    script_args: &[String],
    tool_override: Option<BuildTool>,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let script_content = std::fs::read_to_string(file_path).map_err(|e| {
        DjukeError::Generic(format!(
            "Failed to read script '{}': {}",
            file_path.display(),
            e
        ))
    })?;
    let directives = djuke_pm::ScriptDirectives::parse(&script_content);

    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut config = resolve_project_config_with_tool(&current_dir, None, tool_override, paths);

    // If script specifies //JAVA <ver>, override config's jdk_spec
    if let Some(ref jv) = directives.java_version {
        config.jdk_spec.version = jv.clone();
    }

    let installed_jdk = resolve_jdk(&config.jdk_spec, paths, true, quiet).await?;
    let java_bin = DjukePaths::java_executable(&installed_jdk.java_home);
    let bin_str = java_bin.to_string_lossy();

    // Resolve dependencies if any
    let mut resolved_jars = Vec::new();
    if !directives.dependencies.is_empty() {
        if !quiet {
            println!(
                "{} Resolving {} script dependencies for {}...",
                "⚡".cyan(),
                directives.dependencies.len(),
                file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .green()
            );
        }
        resolved_jars = djuke_pm::resolve_script_dependencies(
            &directives.dependencies,
            &paths.cache_dir(),
            quiet,
        )
        .await?;
    }

    // Build final java invocation arguments:
    let mut java_args: Vec<String> = Vec::new();

    let has_preview = directives
        .jvm_options
        .iter()
        .any(|o| o == "--enable-preview");
    let has_source = directives
        .jvm_options
        .iter()
        .any(|o| o == "--source" || o.starts_with("--source="));

    for opt in &directives.jvm_options {
        java_args.push(opt.clone());
    }

    // Java source launcher requires --source <version> when --enable-preview is present
    if has_preview && !has_source {
        java_args.push("--source".to_string());
        java_args.push(config.jdk_spec.version.clone());
    }

    if !resolved_jars.is_empty() {
        java_args.push("-cp".to_string());
        java_args.push(djuke_pm::build_classpath(&resolved_jars));
    }

    for src in &directives.sources {
        java_args.push(src.clone());
    }

    java_args.push(file_path.to_string_lossy().to_string());

    for arg in script_args {
        java_args.push(arg.clone());
    }

    let arg_slices: Vec<&str> = java_args.iter().map(|s| s.as_str()).collect();
    let ctx = CommandContext {
        java_home: installed_jdk.java_home,
        working_dir: current_dir,
        extra_env: config.env,
    };

    let status = ProcessRunner::run_command(&bin_str, &arg_slices, &ctx).await?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
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

fn resolve_dependency_alias(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    match lower.as_str() {
        "lombok" => "org.projectlombok:lombok".into(),
        "guava" => "com.google.guava:guava".into(),
        "jackson" | "jackson-databind" => "com.fasterxml.jackson.core:jackson-databind".into(),
        "postgres" | "postgresql" => "org.postgresql:postgresql".into(),
        "mysql" => "com.mysql:mysql-connector-j".into(),
        "h2" => "com.h2database:h2".into(),
        "sqlite" => "org.xerial:sqlite-jdbc".into(),
        "junit" | "junit5" => "org.junit.jupiter:junit-jupiter".into(),
        "mockito" => "org.mockito:mockito-core".into(),
        "assertj" => "org.assertj:assertj-core".into(),
        "slf4j" => "org.slf4j:slf4j-api".into(),
        "logback" => "ch.qos.logback:logback-classic".into(),
        "gson" => "com.google.code.gson:gson".into(),
        "commons-lang" | "commons-lang3" => "org.apache.commons:commons-lang3".into(),
        "dotenv" => "io.github.cdimascio:dotenv-java".into(),
        "javalin" => "io.javalin:javalin".into(),
        "picocli" => "info.picocli:picocli".into(),
        _ => input.to_string(),
    }
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
