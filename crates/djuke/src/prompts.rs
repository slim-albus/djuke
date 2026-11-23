use miette::Result;
use std::io::IsTerminal;

use djuke_core::DjukePaths;
use djuke_core::{DjukeError, JdkDistribution};

use crate::cli::{CliProjectOptions, ResolvedProjectInitOptions};

pub async fn resolve_project_options(
    opts: CliProjectOptions,
) -> Result<ResolvedProjectInitOptions> {
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
                    "Gradle (Kotlin DSL - build.gradle.kts)   [Modern Gradle - Recommended]",
                    "Gradle (Groovy DSL - build.gradle)       [Classic Gradle syntax]",
                    "Maven (pom.xml)                          [Standard Apache Maven XML]",
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
                    "21 (LTS) [Installed locally ✓] - Recommended: Current production standard"
                        .to_string()
                } else {
                    "21 (LTS)                       - Recommended: Current production standard"
                        .to_string()
                };
                let v25_label = if is_downloaded("25") {
                    "25 (LTS) [Installed locally ✓] - Modern standard: Next-generation long-term support"
                        .to_string()
                } else {
                    "25 (LTS)                       - Modern standard: Next-generation long-term support"
                        .to_string()
                };
                let v23_label = if is_downloaded("23") {
                    "23 (GA)  [Installed locally ✓] - Recent feature release: Virtual threads & modern features"
                        .to_string()
                } else {
                    "23 (GA)                        - Recent feature release: Virtual threads & modern features"
                        .to_string()
                };
                let v17_label = if is_downloaded("17") {
                    "17 (LTS) [Installed locally ✓] - Enterprise LTS: Broad compatibility for established projects"
                        .to_string()
                } else {
                    "17 (LTS)                       - Enterprise LTS: Broad compatibility for established projects"
                        .to_string()
                };
                let v27_label = if is_downloaded("27") {
                    "27 (EA)  [Installed locally ✓] - Next-Gen / Bleeding edge: Early access language features"
                        .to_string()
                } else {
                    "27 (EA)                        - Next-Gen / Bleeding edge: Early access language features"
                        .to_string()
                };
                let v11_label = if is_downloaded("11") {
                    "11 (LTS) [Installed locally ✓] - Legacy enterprise: Older enterprise maintenance"
                        .to_string()
                } else {
                    "11 (LTS)                       - Legacy enterprise: Older enterprise maintenance"
                        .to_string()
                };
                let v8_label = if is_downloaded("8") {
                    "8  (LTS) [Installed locally ✓] - Legacy baseline: Java 1.8 compatibility"
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
