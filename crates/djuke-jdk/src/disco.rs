use serde::{Deserialize, Serialize};

use djuke_core::{DjukeError, JdkDistribution, Result};
use djuke_platform::Platform;

const DISCO_BASE_URL: &str = "https://api.foojay.io/disco/v3.0";

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
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl DiscoClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn query_packages(
        &self,
        version: &str,
        distro: JdkDistribution,
        platform: Platform,
    ) -> Result<Vec<DiscoPackage>> {
        let url = format!("{}/packages", DISCO_BASE_URL);
        let ext = platform.os.default_archive_ext();

        let req = self.client.get(&url).query(&[
            ("version", version),
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
        let packages = self.query_packages(version, distro, platform).await?;
        let mut pkg = packages
            .into_iter()
            .next()
            .ok_or_else(|| DjukeError::JdkNotFound {
                version: version.to_string(),
                distro: distro.to_string(),
                os: platform.os.to_string(),
                arch: platform.arch.to_string(),
            })?;

        // Enrich with direct download link and checksum from /ids/{id}
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

        // Fallback to pkg_download_redirect if direct_download_uri was not found
        if pkg.download_link.is_none() {
            if let Some(links) = &pkg.links {
                pkg.download_link = links.pkg_download_redirect.clone();
            }
        }

        Ok(pkg)
    }
}
