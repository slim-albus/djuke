use std::collections::HashMap;
use std::path::{Path, PathBuf};

use djuke_core::DjukePaths;
use djuke_core::{BuildTool, JdkSpec, TaskMap};

use crate::detect::detect_project_markers;
use crate::global::load_global_config;
use crate::manifest::find_manifest_file;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum JdkResolutionSource {
    CliFlag,
    EnvironmentVariable,
    ProjectManifest(PathBuf),
    ProjectMarkerFile(String),
    GlobalConfig(PathBuf),
    FallbackDefault,
}

impl std::fmt::Display for JdkResolutionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CliFlag => write!(f, "CLI flag (--java)"),
            Self::EnvironmentVariable => write!(f, "Environment variable (DJUKE_JAVA)"),
            Self::ProjectManifest(p) => write!(f, "Project manifest ({})", p.display()),
            Self::ProjectMarkerFile(s) => write!(f, "Project version file ({})", s),
            Self::GlobalConfig(p) => write!(f, "Global config ({})", p.display()),
            Self::FallbackDefault => write!(f, "Default fallback"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedProjectConfig {
    pub project_root: PathBuf,
    pub manifest_path: Option<PathBuf>,
    pub jdk_spec: JdkSpec,
    pub jdk_source: JdkResolutionSource,
    pub build_tool: Option<BuildTool>,
    pub has_gradle_wrapper: bool,
    pub has_maven_wrapper: bool,
    pub tasks: TaskMap,
    pub env: HashMap<String, String>,
}

pub fn resolve_project_config(
    start_dir: &Path,
    cli_java_override: Option<&str>,
    paths: &DjukePaths,
) -> ResolvedProjectConfig {
    resolve_project_config_with_tool(start_dir, cli_java_override, None, paths)
}

pub fn resolve_project_config_with_tool(
    start_dir: &Path,
    cli_java_override: Option<&str>,
    cli_tool_override: Option<BuildTool>,
    paths: &DjukePaths,
) -> ResolvedProjectConfig {
    let global_config = load_global_config(&paths.config_file()).unwrap_or_default();
    let default_distro = global_config.default_distribution();
    let default_version = global_config.default_java_version();
    let markers = detect_project_markers(start_dir);
    let manifest_path = find_manifest_file(&markers.root);

    let manifest = manifest_path
        .as_ref()
        .and_then(|p| crate::manifest::load_manifest(p).ok())
        .unwrap_or_default();

    // 1. Resolve JDK version & distribution & source
    let (distro, version, jdk_source) = if let Some(cli_val) = cli_java_override {
        let (d, v) = if let Ok(spec) = cli_val.parse::<JdkSpec>() {
            (spec.distribution, spec.version)
        } else {
            (default_distro, cli_val.to_string())
        };
        (d, v, JdkResolutionSource::CliFlag)
    } else if let Ok(env_val) = std::env::var("DJUKE_JAVA") {
        let (d, v) = if let Ok(spec) = env_val.parse::<JdkSpec>() {
            (spec.distribution, spec.version)
        } else {
            (default_distro, env_val)
        };
        (d, v, JdkResolutionSource::EnvironmentVariable)
    } else if let Some(java_sec) = &manifest.java {
        let d = java_sec.distribution.unwrap_or(default_distro);
        let path = manifest_path
            .clone()
            .unwrap_or_else(|| markers.root.join("djuke.toml"));
        (
            d,
            java_sec.version.clone(),
            JdkResolutionSource::ProjectManifest(path),
        )
    } else if let Some(inferred_ver) = &markers.inferred_java_version {
        (
            default_distro,
            inferred_ver.clone(),
            JdkResolutionSource::ProjectMarkerFile(".java-version / .sdkmanrc".into()),
        )
    } else if global_config.java.is_some() || global_config.defaults.is_some() {
        (
            default_distro,
            default_version,
            JdkResolutionSource::GlobalConfig(paths.config_file()),
        )
    } else {
        (
            default_distro,
            default_version,
            JdkResolutionSource::FallbackDefault,
        )
    };

    let jdk_spec = JdkSpec::new(distro, version);

    // 2. Resolve Build Tool
    let build_tool = cli_tool_override
        .or_else(|| manifest.build.as_ref().and_then(|b| b.tool))
        .or(markers.build_tool);

    // 3. Resolve Tasks
    let mut tasks = TaskMap::new();
    if let Some(tool) = build_tool {
        match tool {
            BuildTool::Gradle => {
                let (gradle_run, gradle_dev) = {
                    let build_gradle = markers.root.join("build.gradle");
                    let build_kts = markers.root.join("build.gradle.kts");
                    let content = std::fs::read_to_string(&build_gradle)
                        .or_else(|_| std::fs::read_to_string(&build_kts))
                        .unwrap_or_default();
                    if content.contains("spring") {
                        ("bootRun", "bootRun")
                    } else if content.contains("quarkus") {
                        ("quarkusDev", "quarkusDev")
                    } else if content.contains("micronaut") {
                        ("run", "run --continuous")
                    } else {
                        ("run", "run")
                    }
                };

                tasks.insert("build".into(), "build".into());
                tasks.insert("test".into(), "test".into());
                tasks.insert("run".into(), gradle_run.into());
                tasks.insert("dev".into(), gradle_dev.into());
                tasks.insert("clean".into(), "clean".into());
            }
            BuildTool::Maven => {
                let (maven_run, maven_dev) = {
                    let pom = markers.root.join("pom.xml");
                    let content = std::fs::read_to_string(&pom).unwrap_or_default();
                    if content.contains("spring-boot") {
                        ("spring-boot:run", "spring-boot:run")
                    } else if content.contains("quarkus") {
                        ("quarkus:dev", "quarkus:dev")
                    } else if content.contains("micronaut") {
                        ("exec:java", "mn:run")
                    } else {
                        ("compile exec:java", "compile exec:java")
                    }
                };

                tasks.insert("build".into(), "clean package".into());
                tasks.insert("test".into(), "test".into());
                tasks.insert("run".into(), maven_run.into());
                tasks.insert("dev".into(), maven_dev.into());
                tasks.insert("clean".into(), "clean".into());
            }
        }
    }
    if let Some(custom_tasks) = manifest.tasks {
        for (k, v) in custom_tasks {
            tasks.insert(k, v);
        }
    }

    // 4. Resolve Env
    let env = manifest.env.unwrap_or_default();

    ResolvedProjectConfig {
        project_root: markers.root,
        manifest_path,
        jdk_spec,
        jdk_source,
        build_tool,
        has_gradle_wrapper: markers.has_gradle_wrapper,
        has_maven_wrapper: markers.has_maven_wrapper,
        tasks,
        env,
    }
}
