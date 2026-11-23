use serde::{Deserialize, Serialize};

use djuke_core::{DjukeError, JdkDistribution, Platform, Result};

const DISCO_BASE_URL: &str = "https://api.foojay.io/disco/v3.0";

#[derive(Debug, Clone, Deserialize)]
pub struct DiscoMajorVersion {
    pub major_version: u32,
    pub term_of_support: Option<String>,
    pub maintained: Option<bool>,
    pub early_access_only: Option<bool>,
    pub release_status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DiscoMajorVersionsResponse {
    result: Option<Vec<DiscoMajorVersion>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoLinks {
    pub pkg_info_uri: Option<String>,
    pub pkg_download_redirect: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoPackageInfo {
    pub filename: Option<String>,
    pub direct_download_uri: Option<String>,
    pub checksum: Option<String>,
    pub checksum_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DiscoPkgInfoResponse {
    result: Option<Vec<DiscoPackageInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoPackage {
    pub id: Option<String>,
    pub package_name: Option<String>,
    pub filename: Option<String>,
    pub distribution: Option<String>,
    pub major_version: Option<u32>,
    pub java_version: Option<String>,
    pub distribution_version: Option<String>,
    pub download_link: Option<String>,
    pub checksum: Option<String>,
    pub checksum_type: Option<String>,
    pub archive_type: Option<String>,
    pub links: Option<DiscoLinks>,
}

#[derive(Debug, Clone, Deserialize)]
struct DiscoResponse {
    result: Option<Vec<DiscoPackage>>,
}

pub struct DiscoClient {
    client: reqwest::Client,
}

impl Default for DiscoClient {
    fn default() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("djuke/0.1.0")
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl DiscoClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn fetch_major_versions(&self) -> Result<Vec<DiscoMajorVersion>> {
        let url = format!("{}/major_versions", DISCO_BASE_URL);
        let response =
            self.client.get(&url).send().await.map_err(|e| {
                DjukeError::Network(format!("Failed to fetch major versions: {}", e))
            })?;

        if !response.status().is_success() {
            return Err(DjukeError::Network(format!(
                "Disco API returned error status: {}",
                response.status()
            )));
        }

        let body: DiscoMajorVersionsResponse = response
            .json()
            .await
            .map_err(|e| DjukeError::Serialization(e.to_string()))?;

        Ok(body.result.unwrap_or_default())
    }

    pub async fn resolve_version_alias(&self, version: &str) -> String {
        let trimmed = version.trim();
        if trimmed.eq_ignore_ascii_case("latest") {
            if let Ok(versions) = self.fetch_major_versions().await {
                let max_ga = versions
                    .into_iter()
                    .filter(|v| {
                        v.release_status.as_deref() == Some("ga")
                            && !v.early_access_only.unwrap_or(false)
                    })
                    .map(|v| v.major_version)
                    .max();
                if let Some(m) = max_ga {
                    return m.to_string();
                }
            }
            "23".to_string()
        } else if trimmed.eq_ignore_ascii_case("lts") {
            if let Ok(versions) = self.fetch_major_versions().await {
                let max_lts = versions
                    .into_iter()
                    .filter(|v| {
                        v.release_status.as_deref() == Some("ga")
                            && !v.early_access_only.unwrap_or(false)
                            && v.term_of_support.as_deref() == Some("LTS")
                    })
                    .map(|v| v.major_version)
                    .max();
                if let Some(m) = max_lts {
                    return m.to_string();
                }
            }
            "21".to_string()
        } else {
            trimmed.to_string()
        }
    }

    pub async fn query_packages(
        &self,
        version: &str,
        distro: JdkDistribution,
        platform: Platform,
    ) -> Result<Vec<DiscoPackage>> {
        let resolved_version = self.resolve_version_alias(version).await;
        let url = format!("{}/packages", DISCO_BASE_URL);
        let ext = platform.os.default_archive_ext();

        let req = self.client.get(&url).query(&[
            ("version", resolved_version.as_str()),
            ("distro", distro.as_disco_name()),
            ("operating_system", platform.os.as_disco_name()),
            ("architecture", platform.arch.as_disco_name()),
            ("archive_type", ext),
            ("package_type", "jdk"),
            ("release_status", "ga"),
            ("latest", "available"),
        ]);

        let response = req
            .send()
            .await
            .map_err(|e| DjukeError::Network(format!("Foojay Disco API request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(DjukeError::Network(format!(
                "Disco API returned error status: {}",
                response.status()
            )));
        }

        let body: DiscoResponse = response
            .json()
            .await
            .map_err(|e| DjukeError::Serialization(e.to_string()))?;

        Ok(body.result.unwrap_or_default())
    }

    pub async fn fetch_package_info(&self, package_id: &str) -> Result<Option<DiscoPackageInfo>> {
        let url = format!("{}/ids/{}", DISCO_BASE_URL, package_id);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| DjukeError::Network(format!("Failed to fetch package info: {}", e)))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let body: DiscoPkgInfoResponse = response
            .json()
            .await
            .map_err(|e| DjukeError::Serialization(e.to_string()))?;

        Ok(body.result.and_then(|mut v| v.pop()))
    }

    pub async fn find_best_package(
        &self,
        version: &str,
        distro: JdkDistribution,
        platform: Platform,
    ) -> Result<DiscoPackage> {
        let is_latest = version.eq_ignore_ascii_case("latest");
        let is_lts = version.eq_ignore_ascii_case("lts");

        if is_latest || is_lts {
            if let Ok(versions) = self.fetch_major_versions().await {
                let mut candidates: Vec<u32> = versions
                    .into_iter()
                    .filter(|v| {
                        v.release_status.as_deref() == Some("ga")
                            && !v.early_access_only.unwrap_or(false)
                            && (!is_lts || v.term_of_support.as_deref() == Some("LTS"))
                    })
                    .map(|v| v.major_version)
                    .collect();
                candidates.sort_unstable_by(|a, b| b.cmp(a));

                for ver in candidates {
                    let ver_str = ver.to_string();
                    if let Ok(pkgs) = self.query_packages(&ver_str, distro, platform).await {
                        if let Some(mut pkg) = pkgs.into_iter().next() {
                            self.enrich_package(&mut pkg).await;
                            return Ok(pkg);
                        }
                    }
                }
            }
        }

        let resolved_version = self.resolve_version_alias(version).await;
        let packages = self
            .query_packages(&resolved_version, distro, platform)
            .await?;
        let mut pkg = packages
            .into_iter()
            .next()
            .ok_or_else(|| DjukeError::JdkNotFound {
                version: resolved_version.clone(),
                distro: distro.to_string(),
                os: platform.os.to_string(),
                arch: platform.arch.to_string(),
            })?;

        self.enrich_package(&mut pkg).await;
        Ok(pkg)
    }

    async fn enrich_package(&self, pkg: &mut DiscoPackage) {
        if let Some(id) = &pkg.id {
            if let Ok(Some(info)) = self.fetch_package_info(id).await {
                if let Some(uri) = info.direct_download_uri {
                    pkg.download_link = Some(uri);
                }
                if let Some(c) = info.checksum {
                    pkg.checksum = Some(c);
                }
                if let Some(ct) = info.checksum_type {
                    pkg.checksum_type = Some(ct);
                }
            }
        }

        if pkg.download_link.is_none() {
            if let Some(links) = &pkg.links {
                pkg.download_link = links.pkg_download_redirect.clone();
            }
        }
    }
}

impl djuke_core::JdkCatalogProvider for DiscoClient {
    fn name(&self) -> &'static str {
        "foojay-disco"
    }

    async fn find_package(
        &self,
        spec: &djuke_core::JdkSpec,
        os: &str,
        arch: &str,
    ) -> Result<djuke_core::DiscoveredJdkPackage> {
        let platform = Platform::current();
        let pkg = self
            .find_best_package(&spec.version, spec.distribution, platform)
            .await?;

        let download_url = pkg
            .download_link
            .ok_or_else(|| DjukeError::JdkNotFound {
                version: spec.version.clone(),
                distro: spec.distribution.to_string(),
                os: os.to_string(),
                arch: arch.to_string(),
            })?;

        let filename = pkg.filename.unwrap_or_else(|| "jdk_archive".to_string());
        let sha256 = if pkg
            .checksum_type
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("sha256"))
            .unwrap_or(false)
        {
            pkg.checksum
        } else {
            None
        };

        Ok(djuke_core::DiscoveredJdkPackage {
            download_url,
            filename,
            sha256_checksum: sha256,
            size_bytes: None,
        })
    }
}
