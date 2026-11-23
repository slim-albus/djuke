use colored::Colorize;
use miette::Result;
use std::io::IsTerminal;
use std::path::Path;

use djuke_config::{load_manifest, resolve_project_config};
use djuke_core::{DjukeError, DjukePaths, JdkDistribution, JdkSpec, Platform, ProjectManifest};
use djuke_jdk::{disco::DiscoMajorVersion, install_jdk, resolve_jdk, DiscoClient, JdkStore};

use crate::cli::{JdkArgs, JdkSubcommands};

pub async fn handle_use_command(
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
            "✓ Set global default Java to {}@{}",
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
            "✓ Configured project in '{}' to use {}@{}",
            manifest_file.display(),
            parsed_spec.distribution,
            parsed_spec.version
        );
    }
    println!("  JAVA_HOME: {}", installed_jdk.java_home.display());

    Ok(())
}

pub async fn handle_java(
    args: JdkArgs,
    current_dir: &Path,
    paths: &DjukePaths,
    json: bool,
    quiet: bool,
) -> Result<()> {
    let sub = args.command.unwrap_or(JdkSubcommands::List {
        remote: false,
        lts: false,
        distro: None,
    });
    match sub {
        JdkSubcommands::List { remote, lts, distro } => {
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
                let config = resolve_project_config(current_dir, None, paths);
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

                if json {
                    let s = serde_json::to_string_pretty(&installed)
                        .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?;
                    println!("{}", s);
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

        JdkSubcommands::Install { spec, distro } => {
            let target_spec = match spec {
                Some(s) if !s.trim().is_empty() => s,
                _ => {
                    if std::io::stdin().is_terminal() {
                        let theme = dialoguer::theme::ColorfulTheme::default();
                        let options = [
                            "Eclipse Temurin 21 (LTS) [Adoptium standard]",
                            "Eclipse Temurin 25 (LTS) [Next-gen long-term support]",
                            "Eclipse Temurin 17 (LTS) [Enterprise legacy LTS]",
                            "Oracle OpenJDK 21 [Official reference build]",
                            "Amazon Corretto 21 [AWS production runtime]",
                            "Custom specification...",
                        ];
                        let idx = dialoguer::Select::with_theme(&theme)
                            .with_prompt("Select a JDK to download and install")
                            .items(&options)
                            .default(0)
                            .interact()
                            .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;

                        match idx {
                            0 => "21".to_string(),
                            1 => "25".to_string(),
                            2 => "17".to_string(),
                            3 => "openjdk@21".to_string(),
                            4 => "corretto@21".to_string(),
                            _ => {
                                let input: String = dialoguer::Input::with_theme(&theme)
                                    .with_prompt("Enter JDK specification (e.g. '21', 'zulu@17')")
                                    .interact_text()
                                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                                input
                            }
                        }
                    } else {
                        "21".to_string()
                    }
                }
            };

            let mut parsed_spec: JdkSpec = target_spec.parse()?;
            if let Some(d) = distro {
                parsed_spec.distribution = d;
            }
            let installed = install_jdk(&parsed_spec, paths, quiet).await?;
            if json {
                let s = serde_json::to_string_pretty(&installed)
                    .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?;
                println!("{}", s);
            } else if !quiet {
                println!(
                    "✓ Installed {} to {}",
                    format!("{}@{}", installed.distribution, installed.version).green().bold(),
                    installed.java_home.display()
                );
            }
        }

        JdkSubcommands::Default { spec, distro } => {
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

                        if json {
                            let json_data = serde_json::json!({
                                "configured": true,
                                "distribution": d.to_string(),
                                "version": v,
                                "source": source,
                                "installed": installed.is_some(),
                                "java_home": installed.as_ref().map(|i| i.java_home.display().to_string()),
                            });
                            let s = serde_json::to_string_pretty(&json_data)
                                .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?;
                            println!("{}", s);
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
                        if json {
                            let json_data = serde_json::json!({
                                "configured": false,
                                "message": "No global default Java configured. Run 'djuke use --global' to set one.",
                            });
                            let s = serde_json::to_string_pretty(&json_data)
                                .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?;
                            println!("{}", s);
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
                    let installed = resolve_jdk(&parsed_spec, paths, true, quiet).await?;
                    let mut j = cfg.java.unwrap_or_default();
                    j.default_version = Some(parsed_spec.version.clone());
                    j.default_distribution = Some(parsed_spec.distribution);
                    cfg.java = Some(j);
                    djuke_config::global::save_global_config(&paths.config_file(), &cfg)?;
                    println!(
                        "✓ Set global default Java to {}@{}",
                        parsed_spec.distribution, parsed_spec.version
                    );
                    println!("  JAVA_HOME: {}", installed.java_home.display());
                }
            }
        }

        JdkSubcommands::Use { spec, distro, global } => {
            handle_use_command(spec, distro, global, paths, current_dir, quiet).await?;
        }

        JdkSubcommands::Current => {
            let config = resolve_project_config(current_dir, None, paths);
            let store = JdkStore::new(paths.clone());
            let installed = store.find_installed(&config.jdk_spec);

            if json {
                let json_data = serde_json::json!({
                    "spec": format!("{}", config.jdk_spec),
                    "distribution": config.jdk_spec.distribution.to_string(),
                    "version": config.jdk_spec.version,
                    "resolution_source": format!("{}", config.jdk_source),
                    "installed": installed.is_some(),
                    "java_home": installed.as_ref().map(|i| i.java_home.display().to_string())
                });
                let s = serde_json::to_string_pretty(&json_data)
                    .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?;
                println!("{}", s);
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

        JdkSubcommands::Uninstall { target } => {
            let store = JdkStore::new(paths.clone());
            let target_id = match target {
                Some(t) if !t.trim().is_empty() => t,
                _ => {
                    let installed = store.list_installed()?;
                    if installed.is_empty() {
                        println!("No JDKs currently installed locally.");
                        return Ok(());
                    }
                    if std::io::stdin().is_terminal() {
                        let theme = dialoguer::theme::ColorfulTheme::default();
                        let items: Vec<String> = installed
                            .iter()
                            .map(|j| {
                                format!(
                                    "{}@{} ({})",
                                    j.distribution,
                                    j.version,
                                    j.java_home.display()
                                )
                            })
                            .collect();
                        let idx = dialoguer::Select::with_theme(&theme)
                            .with_prompt("Select JDK to uninstall")
                            .items(&items)
                            .default(0)
                            .interact()
                            .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                        installed[idx].id.clone()
                    } else {
                        return Err(DjukeError::Generic(
                            "Please provide the JDK version or id to uninstall (e.g. 'djuke jdk uninstall 21')".into(),
                        )
                        .into());
                    }
                }
            };
            let removed_path = store.uninstall(&target_id)?;
            println!("✓ Removed JDK at {}", removed_path.display());
        }
    }
    Ok(())
}
