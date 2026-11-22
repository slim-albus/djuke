use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;

use djuke_config::resolve_project_config;
use djuke_core::Result;
use djuke_jdk::JdkStore;
use djuke_platform::{DjukePaths, Platform};

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

    // 3. Network check (Foojay Disco API ping)
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let net_res = client
        .get("https://api.foojay.io/disco/v3.0/major_versions")
        .send()
        .await;
    let net_status = net_res.map(|r| r.status().is_success()).unwrap_or(false);
    checks.push(HealthCheckResult {
        name: "Disco API Connectivity".into(),
        status: net_status,
        message: if net_status {
            "Foojay Disco API is reachable".into()
        } else {
            "Cannot connect to api.foojay.io".into()
        },
        hint: if !net_status {
            Some("Check your internet connection or proxy settings".into())
        } else {
            None
        },
    });

    // 4. Installed JDK inventory
    let store = JdkStore::new(paths.clone());
    let installed = store.list_installed().unwrap_or_default();
    let has_jdks = !installed.is_empty();
    let jdk_msg = if has_jdks {
        format!("{} JDK(s) managed locally", installed.len())
    } else {
        "No JDKs currently installed in djuke store".into()
    };
    checks.push(HealthCheckResult {
        name: "Installed JDKs".into(),
        status: has_jdks,
        message: jdk_msg,
        hint: if !has_jdks {
            Some(
                "Run 'djuke java install 21' or 'djuke java install latest' to install a JDK"
                    .into(),
            )
        } else {
            None
        },
    });

    // 5. Project context check
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
            hint: None,
        });

        // Java version configured for project
        let jdk_spec_str = format!("{}", resolved.jdk_spec);
        let jdk_present = store.find_installed(&resolved.jdk_spec).is_some();
        checks.push(HealthCheckResult {
            name: "Project Java Version".into(),
            status: jdk_present,
            message: if jdk_present {
                format!("Configured {} (installed)", jdk_spec_str)
            } else {
                format!("Configured {} (not installed)", jdk_spec_str)
            },
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

    let overall_status = checks.iter().all(|c| c.status);

    Ok(DoctorReport {
        platform: format!("{}", platform),
        djuke_home: format!("{}", paths.root.display()),
        checks,
        overall_status,
    })
}
