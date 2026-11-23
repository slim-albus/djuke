use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

use djuke_config::{load_global_config, resolve_project_config};
use djuke_core::{DjukePaths, Platform, Result};
use djuke_jdk::JdkStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub name: String,
    pub status: bool,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub platform: String,
    pub djuke_home: String,
    pub checks: Vec<HealthCheckResult>,
    pub overall_status: bool,
}

impl DoctorReport {
    pub fn print_human(&self) {
        println!("\n{}", "🩺 djuke Doctor System Diagnostic".bold().cyan());
        println!("{}\n", "================================".cyan());

        for check in &self.checks {
            if check.status {
                println!(
                    "{} {}: {}",
                    "✔".green().bold(),
                    check.name.bold(),
                    check.message
                );
            } else {
                println!(
                    "{} {}: {}",
                    "✘".red().bold(),
                    check.name.bold(),
                    check.message.red()
                );
                if let Some(hint) = &check.hint {
                    println!("    {} {}", "↳ Hint:".yellow(), hint);
                }
            }
        }

        println!();
        if self.overall_status {
            println!(
                "{}",
                "✔ Your environment is ready for Java development!"
                    .green()
                    .bold()
            );
        } else {
            println!(
                "{}",
                "⚠ Environment is not ready for Java development. Resolve the issues above."
                    .yellow()
                    .bold()
            );
            println!(
                "  {} Run '{}' or '{}' to install a managed JDK.\n",
                "Tip:".cyan().bold(),
                "djuke java install 21".green(),
                "djuke setup".green()
            );
        }
        println!();
    }
}

pub async fn run_doctor_checks(working_dir: &Path, paths: &DjukePaths) -> Result<DoctorReport> {
    let mut checks = Vec::new();
    let platform = Platform::current();

    // 1. Platform check
    checks.push(HealthCheckResult {
        name: "Platform".into(),
        status: true,
        message: format!("OS: {}, Architecture: {}", platform.os, platform.arch),
        hint: None,
    });

    // 2. DJUKE_HOME writable check
    let home_status = paths.ensure_dirs().is_ok();
    checks.push(HealthCheckResult {
        name: "djuke Home".into(),
        status: home_status,
        message: format!("{}", paths.root.display()),
        hint: if !home_status {
            Some("Ensure write permissions on user home directory or set DJUKE_HOME".into())
        } else {
            None
        },
    });

    // 3. Network checks in parallel
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("djuke-doctor")
        .build()
        .unwrap_or_default();

    let disco_fut = client
        .get("https://api.foojay.io/disco/v3.0/major_versions")
        .send();
    let central_fut = client.head("https://repo1.maven.org/maven2").send();
    let initializr_fut = client.head("https://start.spring.io").send();

    let (disco_res, central_res, initializr_res) =
        tokio::join!(disco_fut, central_fut, initializr_fut);

    let disco_ok = disco_res.map(|r| r.status().is_success()).unwrap_or(false);
    checks.push(HealthCheckResult {
        name: "Foojay Disco API".into(),
        status: disco_ok,
        message: if disco_ok {
            "Reachable (JDK metadata & download index)".into()
        } else {
            "Cannot connect to api.foojay.io".into()
        },
        hint: if !disco_ok {
            Some("Check internet connectivity or proxy settings".into())
        } else {
            None
        },
    });

    let central_ok = central_res
        .map(|r| r.status().is_success() || r.status().is_redirection())
        .unwrap_or(false);
    checks.push(HealthCheckResult {
        name: "Maven Central".into(),
        status: central_ok,
        message: if central_ok {
            "Reachable (repo1.maven.org)".into()
        } else {
            "Cannot connect to repo1.maven.org".into()
        },
        hint: if !central_ok {
            Some("Maven dependencies cannot be fetched without access to Maven Central".into())
        } else {
            None
        },
    });

    let initializr_ok = initializr_res
        .map(|r| r.status().is_success() || r.status().is_redirection())
        .unwrap_or(false);
    checks.push(HealthCheckResult {
        name: "Spring Initializr API".into(),
        status: initializr_ok,
        message: if initializr_ok {
            "Reachable (start.spring.io)".into()
        } else {
            "Offline / Unreachable (embedded offline templates will be used)".into()
        },
        hint: None,
    });

    // 4. Installed JDK inventory
    let store = JdkStore::new(paths.clone());
    let installed = store.list_installed().unwrap_or_default();
    let has_jdks = !installed.is_empty();
    let jdk_msg = if has_jdks {
        format!("{} JDK(s) managed locally in djuke store", installed.len())
    } else {
        "No JDKs currently installed in djuke store".into()
    };
    checks.push(HealthCheckResult {
        name: "Installed JDKs".into(),
        status: has_jdks,
        message: jdk_msg,
        hint: if !has_jdks {
            Some(
                "Run 'djuke java install 21' or 'djuke java default 21' to install a managed JDK"
                    .into(),
            )
        } else {
            None
        },
    });

    // 5. Global Default JDK
    let global_config = load_global_config(&paths.config_file()).unwrap_or_default();
    if let Some(j) = &global_config.java {
        let g_distro = j
            .default_distribution
            .unwrap_or_else(djuke_core::JdkDistribution::default_distro);
        let g_ver = j
            .default_version
            .clone()
            .unwrap_or_else(|| "21".to_string());
        let global_spec = djuke_core::JdkSpec::new(g_distro, g_ver);
        let global_installed = store.find_installed(&global_spec).is_some();
        checks.push(HealthCheckResult {
            name: "Global Default Java".into(),
            status: global_installed,
            message: if global_installed {
                format!("{} (installed locally)", global_spec)
            } else {
                format!("{} (not installed locally)", global_spec)
            },
            hint: if !global_installed {
                Some(format!(
                    "Run 'djuke use {} --global' to install and activate this JDK",
                    global_spec.version
                ))
            } else {
                None
            },
        });
    } else {
        checks.push(HealthCheckResult {
            name: "Global Default Java".into(),
            status: true,
            message: "Not configured (run 'djuke use' to set your preferred runtime)".into(),
            hint: Some(
                "Run 'djuke use' to interactively select or install your preferred default Java"
                    .into(),
            ),
        });
    }

    // 6. System PATH Java detection
    let system_java = detect_system_java();
    checks.push(HealthCheckResult {
        name: "System PATH Java".into(),
        status: true,
        message: match system_java {
            Some(info) => format!("Found: {}", info),
            None => "None found on PATH (djuke manages isolated runtimes)".into(),
        },
        hint: None,
    });

    // 7. Project context check
    let resolved = resolve_project_config(working_dir, None, paths);
    let has_project = resolved.build_tool.is_some() || resolved.manifest_path.is_some();
    if has_project {
        let tool_name = resolved
            .build_tool
            .map(|t| t.to_string())
            .unwrap_or_else(|| "unspecified".into());
        let wrapper_found = resolved.has_gradle_wrapper || resolved.has_maven_wrapper;
        checks.push(HealthCheckResult {
            name: "Project Build Tool".into(),
            status: true,
            message: format!("Detected {} (wrapper: {})", tool_name, wrapper_found),
            hint: if !wrapper_found {
                Some("Run 'djuke setup' to automatically bootstrap project wrapper".into())
            } else {
                None
            },
        });

        // Java version configured for project
        let jdk_spec_str = format!("{}", resolved.jdk_spec);
        let jdk_present = store.find_installed(&resolved.jdk_spec).is_some();
        checks.push(HealthCheckResult {
            name: "Project Java Version".into(),
            status: jdk_present,
            message: format!(
                "{} [{}] ({})",
                jdk_spec_str,
                resolved.jdk_source,
                if jdk_present {
                    "installed"
                } else {
                    "not installed"
                }
            ),
            hint: if !jdk_present {
                Some(format!("Run 'djuke setup' to install {}", jdk_spec_str))
            } else {
                None
            },
        });
    } else {
        checks.push(HealthCheckResult {
            name: "Project Detection".into(),
            status: true,
            message: "Running in non-project directory (global mode)".into(),
            hint: None,
        });
    }

    // 8. Disk cache stats
    if let Ok(stats) = djuke_io::get_cache_stats(paths) {
        checks.push(HealthCheckResult {
            name: "Cache Storage".into(),
            status: true,
            message: format!("{} files ({})", stats.file_count, stats.human_size),
            hint: None,
        });
    }

    let overall_status = checks.iter().all(|c| c.status);

    Ok(DoctorReport {
        platform: format!("{}", platform),
        djuke_home: format!("{}", paths.root.display()),
        checks,
        overall_status,
    })
}

fn detect_system_java() -> Option<String> {
    let output = Command::new("java").arg("-version").output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{}{}", stderr, stdout);
    let first_line = combined.lines().next()?;
    Some(first_line.trim().to_string())
}
