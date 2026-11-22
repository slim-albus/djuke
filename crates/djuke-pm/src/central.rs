use serde::{Deserialize, Serialize};

use djuke_core::{DjukeError, Result};

const MAVEN_CENTRAL_SEARCH_URL: &str = "https://search.maven.org/solrsearch/select";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MavenArtifact {
    pub id: String,
    #[serde(rename = "g")]
    pub group_id: String,
    #[serde(rename = "a")]
    pub artifact_id: String,
    #[serde(rename = "latestVersion")]
    pub latest_version: Option<String>,
    #[serde(rename = "p")]
    pub packaging: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SolrResponse {
    response: SolrResponseBody,
}

#[derive(Debug, Clone, Deserialize)]
struct SolrResponseBody {
    docs: Vec<MavenArtifact>,
}

pub struct MavenCentralClient {
    client: reqwest::Client,
}

impl Default for MavenCentralClient {
    fn default() -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            ),
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl MavenCentralClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn search(&self, query: &str, rows: u32) -> Result<Vec<MavenArtifact>> {
        let res = self
            .client
            .get(MAVEN_CENTRAL_SEARCH_URL)
            .query(&[("q", query), ("rows", &rows.to_string()), ("wt", "json")])
            .send()
            .await
            .map_err(|e| DjukeError::Network(format!("Maven Central query failed: {}", e)))?;

        if !res.status().is_success() {
            return Err(DjukeError::Network(format!(
                "Maven Central returned status: {}",
                res.status()
            )));
        }

        let solr: SolrResponse = res
            .json()
            .await
            .map_err(|e| DjukeError::Serialization(e.to_string()))?;

        Ok(solr.response.docs)
    }

    pub async fn resolve_latest_version(&self, group: &str, artifact: &str) -> Result<String> {
        let q = format!("g:\"{}\" AND a:\"{}\"", group, artifact);
        let docs = self.search(&q, 1).await?;
        let doc = docs.into_iter().next().ok_or_else(|| {
            DjukeError::Generic(format!(
                "Artifact '{}:{}' not found on Maven Central",
                group, artifact
            ))
        })?;

        doc.latest_version.ok_or_else(|| {
            DjukeError::Generic(format!(
                "No release version found for '{}:{}'",
                group, artifact
            ))
        })
    }
}
