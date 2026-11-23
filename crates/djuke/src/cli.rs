use clap::{Args, Parser, Subcommand};
use djuke_core::{BuildTool, JdkDistribution};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "djuke",
    author = "djuke contributors",
    version,
    about = "Fast, seamless JDK manager and build-tool orchestrator for Java ecosystems",
    long_about = "A developer can clone any Java project, run djuke, and get the correct JDK and build environment immediately."
)]
pub struct Cli {
    #[arg(short, long, global = true, help = "Enable verbose debug logging")]
    pub verbose: bool,

    #[arg(short, long, global = true, help = "Suppress non-essential output")]
    pub quiet: bool,

    #[arg(
        long,
        global = true,
        help = "Output machine-readable JSON where applicable"
    )]
    pub json: bool,

    #[arg(long, global = true, help = "Disable ANSI color output")]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Check development environment health and diagnostic status")]
    Doctor,

    #[command(about = "JDK installation, listing, and switching", alias = "java")]
    Jdk(JdkArgs),

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

    #[command(about = "Inspect and update user preferences and default configuration")]
    Config(ConfigArgs),

    #[command(
        about = "Prepare project dependencies, JDK, and build tool wrappers",
        alias = "install"
    )]
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

        #[arg(
            help = "Optional JDK version or specification to configure and install (e.g. '21', 'temurin@17')"
        )]
        spec: Option<String>,
    },

    #[command(about = "Execute the project build task")]
    Build(BuildTaskArgs),

    #[command(about = "Execute the project test task")]
    Test(BuildTaskArgs),

    #[command(about = "Execute project application or single Java source file")]
    Run(RunTaskArgs),

    #[command(
        about = "Execute development task via the project's build tool",
        hide = true
    )]
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
        #[arg(help = "Shell type: bash, zsh, fish, powershell, nu")]
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
pub struct BuildTaskArgs {
    #[arg(
        short = 't',
        long = "tool",
        help = "Explicitly specify build tool: gradle or maven"
    )]
    pub tool: Option<BuildTool>,

    #[arg(
        long,
        help = "Run in offline mode without network dependency resolution"
    )]
    pub offline: bool,

    #[arg(
        short = 'U',
        long = "refresh",
        help = "Refresh dynamic dependencies and snapshot releases"
    )]
    pub refresh: bool,

    #[arg(
        short = 'P',
        help = "Pass project property (Gradle: -Pprop=val) or activate profile (Maven: -Pprofile)"
    )]
    pub profile: Vec<String>,

    #[arg(short = 'D', help = "Pass system property (e.g. -DskipTests)")]
    pub define: Vec<String>,

    #[arg(long, help = "Execute tasks in parallel")]
    pub parallel: bool,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

impl BuildTaskArgs {
    pub fn into_tool_args(self, tool: BuildTool) -> Vec<String> {
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
pub struct RunTaskArgs {
    #[arg(
        short = 't',
        long = "tool",
        help = "Explicitly specify build tool: gradle or maven"
    )]
    pub tool: Option<BuildTool>,

    #[arg(
        long,
        help = "Run in offline mode without network dependency resolution"
    )]
    pub offline: bool,

    #[arg(
        short = 'U',
        long = "refresh",
        help = "Refresh dynamic dependencies and snapshot releases"
    )]
    pub refresh: bool,

    #[arg(
        short = 'P',
        help = "Pass project property (Gradle: -Pprop=val) or activate profile (Maven: -Pprofile)"
    )]
    pub profile: Vec<String>,

    #[arg(short = 'D', help = "Pass system property (e.g. -Dserver.port=8080)")]
    pub define: Vec<String>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

impl RunTaskArgs {
    pub fn into_tool_args(self, tool: BuildTool) -> Vec<String> {
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

#[derive(Args, Debug, Clone)]
pub struct CacheArgs {
    #[command(subcommand)]
    pub command: CacheSubcommands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CacheSubcommands {
    #[command(about = "Show cache size and item count")]
    Stats,

    #[command(about = "List all downloaded archive files in cache")]
    List,

    #[command(about = "Clear cached archive files and temporary extraction directories")]
    Clean,
}

#[derive(Args, Debug, Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: Option<ConfigSubcommands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigSubcommands {
    #[command(about = "Get a configuration setting")]
    Get {
        #[arg(help = "Key name (e.g. 'distribution', 'version', 'build_tool', 'test_framework')")]
        key: String,
    },
    #[command(about = "Set a configuration setting")]
    Set {
        #[arg(help = "Key name (e.g. 'distribution', 'version', 'build_tool', 'test_framework')")]
        key: String,
        #[arg(help = "Value to set")]
        value: String,
    },
    #[command(about = "List all configuration settings")]
    List,
}

#[derive(Args, Debug, Clone)]
pub struct GradleArgs {
    #[command(subcommand)]
    pub command: Option<GradleSubcommands>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum GradleSubcommands {
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

#[derive(Args, Debug, Clone)]
pub struct MavenArgs {
    #[command(subcommand)]
    pub command: Option<MavenSubcommands>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum MavenSubcommands {
    #[command(about = "Generate the Maven wrapper (mvnw, mvnw.cmd)")]
    Wrapper {
        #[arg(
            long,
            help = "Maven version to generate (default: latest online release)"
        )]
        version: Option<String>,
    },
}

#[derive(Args, Debug, Clone)]
pub struct JdkArgs {
    #[command(subcommand)]
    pub command: Option<JdkSubcommands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum JdkSubcommands {
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
            help = "JDK specification e.g. '21', 'latest', 'lts', 'corretto@21' (interactive picker if omitted)"
        )]
        spec: Option<String>,

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
        #[arg(help = "JDK id, version, or folder name to remove (interactive picker if omitted)")]
        target: Option<String>,
    },
}

#[allow(dead_code)]
pub type JavaArgs = JdkArgs;
#[allow(dead_code)]
pub type JavaSubcommands = JdkSubcommands;

#[derive(Debug, Clone)]
pub struct CliProjectOptions {
    pub is_init: bool,
    pub current_dir: PathBuf,
    pub name: Option<String>,
    pub template: Option<String>,
    pub build_tool: Option<String>,
    pub group: Option<String>,
    pub java: Option<String>,
    pub distro: Option<JdkDistribution>,
    pub test_framework: Option<djuke_templates::TestFramework>,
    pub yes: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedProjectInitOptions {
    pub name: String,
    pub template: djuke_templates::TemplateType,
    pub build_tool: djuke_templates::BuildToolChoice,
    pub group_id: String,
    pub java_ver: String,
    pub distro_choice: Option<JdkDistribution>,
    pub initializr_deps: Option<Vec<String>>,
    pub test_framework: djuke_templates::TestFramework,
}
