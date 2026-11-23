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
    let global_cfg =
        djuke_config::global::load_global_config(&paths.config_file()).unwrap_or_default();
    let default_distro = global_cfg.default_distribution();
    let default_version = global_cfg.default_java_version();

    let (chosen_spec, chosen_distro) = match spec {
        Some(s) => (s, distro),
        None => {
            if is_interactive {
                if installed.is_empty() {
                    println!("{}", "No JDKs currently installed locally.".yellow().bold());
                    let opt1 = format!("{} {} (default LTS)", default_distro, default_version);
                    let opt2 = format!("{} 25 (Next-gen LTS)", default_distro);
                    let opt3 = format!("{} 17 (Enterprise baseline)", default_distro);
                    let opt4 = "Eclipse Temurin 21 (Adoptium standard)".to_string();
                    let opt5 = "Amazon Corretto 21 (AWS runtime)".to_string();
                    let opt6 = "Custom version & distribution...".to_string();
                    let install_options = [
                        opt1.as_str(),
                        opt2.as_str(),
                        opt3.as_str(),
                        opt4.as_str(),
                        opt5.as_str(),
                        opt6.as_str(),
                    ];
                    let idx = dialoguer::Select::with_theme(&theme)
                        .with_prompt("Select a JDK to download, install, and set as default")
                        .items(&install_options)
                        .default(0)
                        .max_length(6)
                        .interact()
                        .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;

                    match idx {
                        0 => (default_version, Some(default_distro)),
                        1 => ("25".to_string(), Some(default_distro)),
                        2 => ("17".to_string(), Some(default_distro)),
                        3 => ("21".to_string(), Some(JdkDistribution::Temurin)),
                        4 => ("21".to_string(), Some(JdkDistribution::Corretto)),
                        5 => {
                            let ver: String = dialoguer::Input::with_theme(&theme)
                                .with_prompt("Target Java version (e.g. 21, 23, 25)")
                                .default(default_version)
                                .interact_text()
                                .map_err(|e| {
                                    DjukeError::Process(format!("Prompt failed: {}", e))
                                })?;
                            (ver, distro.or(Some(default_distro)))
                        }
                        _ => (default_version, Some(default_distro)),
                    }
                } else {
                    let mut items: Vec<String> = installed
                        .iter()
                        .map(|i| format!("{}@{} (v{})", i.distribution, i.version, i.major_version))
                        .collect();
                    items.push("+ Install new JDK runtime...".to_string());

                    let idx = dialoguer::Select::with_theme(&theme)
                        .with_prompt("Select JDK runtime to activate")
                        .items(&items)
                        .default(0)
                        .max_length(7)
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
                            .default(default_version)
                            .interact_text()
                            .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                        (ver, distro.or(Some(default_distro)))
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

    let mut parsed_spec: JdkSpec = if let Some((d_str, v_str)) = chosen_spec.split_once('@') {
        let d = d_str.parse()?;
        JdkSpec::new(d, v_str)
    } else {
        JdkSpec::new(chosen_distro.unwrap_or(default_distro), &chosen_spec)
    };
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
        JdkSubcommands::List {
            remote,
            lts,
            distro,
        } => {
            let store = JdkStore::new(paths.clone());
            let global_config =
                djuke_config::global::load_global_config(&paths.config_file()).unwrap_or_default();
            let default_d = global_config.default_distribution();
            let default_v = global_config.default_java_version();

            if remote {
                let disco = DiscoClient::new();
                let platform = Platform::current();
                let query_distro = distro.unwrap_or(default_d);

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

                // Auto-detect if current directory is actually a project folder
                let is_from_project = matches!(
                    config.jdk_source,
                    djuke_config::JdkResolutionSource::ProjectManifest(_)
                        | djuke_config::JdkResolutionSource::ProjectMarkerFile(_)
                );
                let project_active_id = if is_from_project {
                    store.find_installed(&config.jdk_spec).map(|j| j.id)
                } else {
                    None
                };

                let global_default_id = store
                    .find_installed(&djuke_core::JdkSpec::new(default_d, default_v.clone()))
                    .map(|j| j.id);

                if json {
                    let s = serde_json::to_string_pretty(&installed)
                        .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?;
                    println!("{}", s);
                } else if installed.is_empty() {
                    println!("No JDKs installed in {}.", paths.jdks_dir().display());
                    println!(
                        "Run '{}' to install a recommended JDK.",
                        format!("djuke jdk install {}", default_v).green()
                    );
                } else {
                    println!("{}", "Installed JDKs:".bold().cyan());
                    for jdk in installed {
                        let is_active = project_active_id.as_deref() == Some(&jdk.id);
                        let is_global = global_default_id.as_deref() == Some(&jdk.id);

                        let mut badges = Vec::new();
                        if is_active {
                            badges.push("* [project]".green().bold().to_string());
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
            let global_cfg =
                djuke_config::global::load_global_config(&paths.config_file()).unwrap_or_default();
            let default_distro = global_cfg.default_distribution();
            let default_version = global_cfg.default_java_version();

            let target_spec = match spec {
                Some(s) if !s.trim().is_empty() => s,
                _ => {
                    if std::io::stdin().is_terminal() {
                        let theme = dialoguer::theme::ColorfulTheme::default();
                        let opt1 = format!("{} {} (default LTS)", default_distro, default_version);
                        let opt2 = format!("{} 25 (Next-gen LTS)", default_distro);
                        let opt3 = format!("{} 17 (Enterprise baseline)", default_distro);
                        let opt4 = "Eclipse Temurin 21 (Adoptium standard)".to_string();
                        let opt5 = "Amazon Corretto 21 (AWS runtime)".to_string();
                        let opt6 = "Custom specification...".to_string();
                        let options = [
                            opt1.as_str(),
                            opt2.as_str(),
                            opt3.as_str(),
                            opt4.as_str(),
                            opt5.as_str(),
                            opt6.as_str(),
                        ];
                        let idx = dialoguer::Select::with_theme(&theme)
                            .with_prompt("Select a JDK to download and install")
                            .items(&options)
                            .default(0)
                            .max_length(6)
                            .interact()
                            .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;

                        match idx {
                            0 => default_version,
                            1 => "25".to_string(),
                            2 => "17".to_string(),
                            3 => "temurin@21".to_string(),
                            4 => "corretto@21".to_string(),
                            _ => {
                                let input: String = dialoguer::Input::with_theme(&theme)
                                    .with_prompt("Enter JDK specification (e.g. '21', 'zulu@17')")
                                    .interact_text()
                                    .map_err(|e| {
                                        DjukeError::Process(format!("Prompt failed: {}", e))
                                    })?;
                                input
                            }
                        }
                    } else {
                        default_version
                    }
                }
            };

            let mut parsed_spec: JdkSpec = if let Some((d_str, v_str)) = target_spec.split_once('@')
            {
                let d = d_str.parse()?;
                JdkSpec::new(d, v_str)
            } else {
                JdkSpec::new(distro.unwrap_or(default_distro), &target_spec)
            };
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
                    format!("{}@{}", installed.distribution, installed.version)
                        .green()
                        .bold(),
                    installed.java_home.display()
                );
            }
        }

        JdkSubcommands::Default { spec, distro } => {
            let mut cfg = djuke_config::global::load_global_config(&paths.config_file())?;
            let current_default_distro = cfg.default_distribution();
            let current_default_ver = cfg.default_java_version();

            let (target_version, target_distro) = match (&spec, distro) {
                (None, Some(d)) => (None, Some(d)),
                (None, None) => (None, None),
                (Some(s), explicit_distro) => {
                    let trimmed = s.trim();
                    if let Ok(d) = trimmed.parse::<djuke_core::JdkDistribution>() {
                        (None, Some(explicit_distro.unwrap_or(d)))
                    } else if let Ok(parsed_spec) = trimmed.parse::<djuke_core::JdkSpec>() {
                        let d = explicit_distro.unwrap_or(parsed_spec.distribution);
                        (Some(parsed_spec.version), Some(d))
                    } else {
                        let d = explicit_distro.unwrap_or(current_default_distro);
                        (Some(trimmed.to_string()), Some(d))
                    }
                }
            };

            if target_version.is_none() && target_distro.is_none() {
                let d = current_default_distro;
                let v = current_default_ver;
                let has_config = cfg.java.is_some() || cfg.defaults.is_some();
                let source = if has_config {
                    format!("Config file ({})", paths.config_file().display())
                } else {
                    "System fallback defaults".to_string()
                };
                let store = JdkStore::new(paths.clone());
                let global_spec = djuke_core::JdkSpec::new(d, v.clone());
                let installed = store.find_installed(&global_spec);

                if json {
                    let json_data = serde_json::json!({
                        "configured": has_config,
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
                    let label = if has_config {
                        format!("{}@{}", d, v).green().bold()
                    } else {
                        format!("{}@{} (default fallback)", d, v).yellow().bold()
                    };
                    println!("  • Runtime:      {}", label);
                    println!("  • Distribution: {}", d.to_string().cyan());
                    println!("  • Version:      {}", v.cyan());
                    println!("  • Source:       {}", source);
                    if let Some(i) = installed {
                        println!(
                            "  • Status:       {} ({})",
                            "Installed".green().bold(),
                            i.java_home.display()
                        );
                    } else {
                        println!(
                            "  • Status:       {}",
                            "Not installed locally".yellow().bold()
                        );
                        println!(
                            "  Run '{}' to install and activate.",
                            format!("djuke jdk install {}@{}", d, v).green()
                        );
                    }
                }
            } else if let Some(v) = target_version {
                let d = target_distro.unwrap_or(current_default_distro);
                let spec_to_install = djuke_core::JdkSpec::new(d, v.clone());
                let installed = resolve_jdk(&spec_to_install, paths, true, quiet).await?;
                let mut j = cfg.java.unwrap_or_default();
                j.default_version = Some(v);
                j.default_distribution = Some(d);
                cfg.java = Some(j);
                djuke_config::global::save_global_config(&paths.config_file(), &cfg)?;
                println!(
                    "✓ Set global default Java to {}",
                    format!("{}@{}", d, installed.version).green().bold()
                );
                println!("  JAVA_HOME: {}", installed.java_home.display());
            } else if let Some(d) = target_distro {
                let mut j = cfg.java.unwrap_or_default();
                j.default_distribution = Some(d);
                let current_v = current_default_ver;
                j.default_version = Some(current_v.clone());
                cfg.java = Some(j);
                djuke_config::global::save_global_config(&paths.config_file(), &cfg)?;
                println!(
                    "✓ Set global default JDK distribution to {}",
                    d.to_string().green().bold()
                );
                let store = JdkStore::new(paths.clone());
                let global_spec = djuke_core::JdkSpec::new(d, current_v.clone());
                if let Some(installed) = store.find_installed(&global_spec) {
                    println!(
                        "  Active global runtime: {} ({})",
                        format!("{}@{}", d, installed.version).green(),
                        installed.java_home.display()
                    );
                } else {
                    println!(
                        "  Note: {}@{} is not installed locally. Run '{}' to download it.",
                        d,
                        current_v,
                        format!("djuke jdk install {}@{}", d, current_v).green()
                    );
                }
            }
        }

        JdkSubcommands::Use {
            spec,
            distro,
            global,
        } => {
            handle_use_command(spec, distro, global, paths, current_dir, quiet).await?;
        }

        JdkSubcommands::Current => {
            let config = resolve_project_config(current_dir, None, paths);
            let store = JdkStore::new(paths.clone());
            let installed = store.find_installed(&config.jdk_spec);

            let is_project = matches!(
                config.jdk_source,
                djuke_config::JdkResolutionSource::ProjectManifest(_)
                    | djuke_config::JdkResolutionSource::ProjectMarkerFile(_)
            );

            if json {
                let json_data = serde_json::json!({
                    "spec": format!("{}", config.jdk_spec),
                    "distribution": config.jdk_spec.distribution.to_string(),
                    "version": config.jdk_spec.version,
                    "resolution_source": format!("{}", config.jdk_source),
                    "is_project": is_project,
                    "installed": installed.is_some(),
                    "java_home": installed.as_ref().map(|i| i.java_home.display().to_string())
                });
                let s = serde_json::to_string_pretty(&json_data)
                    .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?;
                println!("{}", s);
            } else if let Some(i) = installed {
                if !is_project {
                    println!(
                        "{}",
                        "(No project detected in current directory; using global default)".dimmed()
                    );
                }
                println!(
                    "Java runtime: {} -> {}",
                    format!("{}@{}", i.distribution, i.version).green().bold(),
                    i.java_home.display()
                );
                println!("Resolved from: {}", format!("{}", config.jdk_source).cyan());
            } else {
                if !is_project {
                    println!(
                        "{}",
                        "(No project detected in current directory; using global default)".dimmed()
                    );
                }
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
                                    "{}@{} (Java {})",
                                    j.distribution, j.version, j.major_version
                                )
                            })
                            .collect();
                        let idx = dialoguer::Select::with_theme(&theme)
                            .with_prompt("Select JDK to uninstall")
                            .items(&items)
                            .default(0)
                            .max_length(7)
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
