use colored::Colorize;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use djuke_core::{DjukeError, Result};

use crate::dependency::DependencySpec;

#[derive(Debug, Serialize)]
struct OsvPackageQuery<'a> {
    name: &'a str,
    ecosystem: &'a str,
}

#[derive(Debug, Serialize)]
struct OsvQuery<'a> {
    version: &'a str,
    package: OsvPackageQuery<'a>,
}

#[derive(Debug, Deserialize)]
struct OsvResponse {
    vulns: Option<Vec<OsvVulnerability>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsvVulnerability {
    pub id: String,
    pub summary: Option<String>,
    pub aliases: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct VulnerabilityReport {
    pub dependency: DependencySpec,
    pub vulnerabilities: Vec<OsvVulnerability>,
}

pub struct AuditClient {
    client: Client,
}

impl Default for AuditClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("djuke/0.1.0 (https://github.com/djuke-dev/djuke)")
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    pub async fn audit_dependency(
        &self,
        dep: &DependencySpec,
    ) -> Result<Option<VulnerabilityReport>> {
        let version = match &dep.version {
            Some(v) => v,
            None => return Ok(None),
        };

        let coord = format!("{}:{}", dep.group_id, dep.artifact_id);
        let query = OsvQuery {
            version,
            package: OsvPackageQuery {
                name: &coord,
                ecosystem: "Maven",
            },
        };

        let res = self
            .client
            .post("https://api.osv.dev/v1/query")
            .json(&query)
            .send()
            .await;

        match res {
            Ok(resp) => {
                if !resp.status().is_success() {
                    return Ok(None);
                }
                let body: OsvResponse = resp.json().await.map_err(|e| {
                    DjukeError::Generic(format!("Failed to parse OSV response: {}", e))
                })?;

                if let Some(vulns) = body.vulns {
                    if !vulns.is_empty() {
                        return Ok(Some(VulnerabilityReport {
                            dependency: dep.clone(),
                            vulnerabilities: vulns,
                        }));
                    }
                }
                Ok(None)
            }
            Err(e) => {
                tracing::warn!("OSV audit request failed for {}: {}", coord, e);
                Ok(None)
            }
        }
    }

    pub async fn audit_dependencies(
        &self,
        deps: &[DependencySpec],
    ) -> Result<Vec<VulnerabilityReport>> {
        let mut reports = Vec::new();
        for dep in deps {
            if let Ok(Some(report)) = self.audit_dependency(dep).await {
                reports.push(report);
            }
        }
        Ok(reports)
    }
}

pub fn render_audit_report(reports: &[VulnerabilityReport], scanned_count: usize) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "\n🔒 {}\n",
        "djuke Dependency Security Audit (OSV.dev)".bold()
    ));
    out.push_str(&format!("{}\n", "=".repeat(60).dimmed()));
    out.push_str(&format!("Scanned {} dependencies.\n\n", scanned_count));

    if reports.is_empty() {
        out.push_str(&format!(
            "{}\n",
            "✓ 0 vulnerabilities detected. All scanned dependencies are clean!"
                .green()
                .bold()
        ));
        return out;
    }

    let mut total_vulns = 0;
    for rep in reports {
        total_vulns += rep.vulnerabilities.len();
        let coord = format!("{}:{}", rep.dependency.group_id, rep.dependency.artifact_id);
        let ver = rep.dependency.version.as_deref().unwrap_or("unknown");

        out.push_str(&format!(
            "✗ {} (version: {})\n",
            coord.red().bold(),
            ver.yellow()
        ));

        for vuln in &rep.vulnerabilities {
            let aliases = vuln
                .aliases
                .as_ref()
                .map(|a| format!(" ({})", a.join(", ")))
                .unwrap_or_default();

            out.push_str(&format!(
                "  - [{}] {}{}\n",
                vuln.id.red(),
                vuln.summary.as_deref().unwrap_or("No summary available"),
                aliases.dimmed()
            ));
            out.push_str(&format!(
                "    Advisory: {}\n",
                format!("https://osv.dev/vulnerability/{}", vuln.id).dimmed()
            ));
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "warning: {} vulnerabilities detected across {} dependencies.\n",
        total_vulns.to_string().red().bold(),
        reports.len().to_string().red().bold()
    ));

    out
}
