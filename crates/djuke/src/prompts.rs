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
                    "CLI Application      - Minimal Java console app",
                    "Library              - Reusable library with JUnit 5",
                    "Spring Boot Web API  - Full-stack web application",
                    "Quarkus REST API     - Reactive cloud microservice",
                    "Micronaut API        - Modular cloud-native service",
                    "Javalin REST API     - Lightweight embedded REST API",
                    "gRPC Microservice    - High-performance gRPC service",
                    "Picocli CLI          - CLI app with flags and colors",
                    "Multi-Module Project - Modular library and service",
                ];
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select project template")
                    .items(&templates)
                    .default(0)
                    .max_length(7)
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
                    "Gradle (Kotlin DSL)  [Recommended - build.gradle.kts]",
                    "Gradle (Groovy DSL)  [Classic syntax - build.gradle]",
                    "Maven                [Apache XML - pom.xml]",
                ];
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select build tool")
                    .items(&tools)
                    .default(0)
                    .max_length(5)
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
                    "OpenJDK (Oracle)      [Default official build]",
                    "Eclipse Temurin       [Adoptium production standard]",
                    "Amazon Corretto       [AWS production distribution]",
                    "Azul Zulu             [Certified TCK builds]",
                    "Microsoft OpenJDK     [Cloud-optimized OpenJDK]",
                    "GraalVM Community     [Native-image high performance]",
                    "BellSoft Liberica     [Full desktop & cloud builds]",
                    "Oracle JDK            [Official Oracle distribution]",
                    "SAP SapMachine        [Enterprise SAP OpenJDK]",
                    "IBM Semeru            [Low footprint Eclipse OpenJ9]",
                ];
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select JDK distribution (flavour)")
                    .items(&distros)
                    .default(0)
                    .max_length(7)
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
                    "21 (LTS) [Installed ✓] - Production standard".to_string()
                } else {
                    "21 (LTS)               - Production standard".to_string()
                };
                let v25_label = if is_downloaded("25") {
                    "25 (LTS) [Installed ✓] - Next-gen LTS standard".to_string()
                } else {
                    "25 (LTS)               - Next-gen LTS standard".to_string()
                };
                let v23_label = if is_downloaded("23") {
                    "23 (GA)  [Installed ✓] - Feature release".to_string()
                } else {
                    "23 (GA)                - Feature release".to_string()
                };
                let v17_label = if is_downloaded("17") {
                    "17 (LTS) [Installed ✓] - Enterprise baseline".to_string()
                } else {
                    "17 (LTS)               - Enterprise baseline".to_string()
                };
                let v27_label = if is_downloaded("27") {
                    "27 (EA)  [Installed ✓] - Bleeding edge preview".to_string()
                } else {
                    "27 (EA)                - Bleeding edge preview".to_string()
                };
                let v11_label = if is_downloaded("11") {
                    "11 (LTS) [Installed ✓] - Legacy enterprise LTS".to_string()
                } else {
                    "11 (LTS)               - Legacy enterprise LTS".to_string()
                };
                let v8_label = if is_downloaded("8") {
                    "8  (LTS) [Installed ✓] - Legacy Java 1.8".to_string()
                } else {
                    "8  (LTS)               - Legacy Java 1.8".to_string()
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
                    .max_length(7)
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
                    "JUnit 5 (Jupiter) - Modern standard [Recommended]",
                    "AssertJ + JUnit 5 - Fluent assertion library",
                    "Spock Framework   - BDD testing with Groovy",
                    "TestNG            - Enterprise test framework",
                    "None              - Do not generate test files",
                ];
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select testing framework")
                    .items(&frameworks)
                    .default(0)
                    .max_length(5)
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
                    let name = &d.name;
                    let short_name = if name.len() > 22 { &name[..20] } else { name };
                    format!("{:<12} - {}", d.id, short_name)
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
                .max_length(7)
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
                            let name = &d.name;
                            let short_name = if name.len() > 22 { &name[..20] } else { name };
                            format!("{:<12} - {}", d.id, short_name)
                        })
                        .collect();
                    let default_rem_flags = vec![false; remaining.len()];
                    let rem_chosen = dialoguer::MultiSelect::with_theme(&theme)
                        .with_prompt(
                            "Select additional dependencies (Space to toggle, Enter to confirm)",
                        )
                        .items(&rem_labels)
                        .defaults(&default_rem_flags)
                        .max_length(7)
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
