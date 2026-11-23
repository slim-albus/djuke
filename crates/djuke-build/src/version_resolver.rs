use std::fs;
use std::time::Duration;

use chrono::Utc;
use djuke_core::DjukePaths;
use serde::{Deserialize, Serialize};

use crate::bootstrap::{DEFAULT_GRADLE_VERSION, DEFAULT_MAVEN_VERSION};

const GRADLE_CURRENT_API: &str = "https://services.gradle.org/versions/current";
const GRADLE_ALL_API: &str = "https://services.gradle.org/versions/all";
const MAVEN_METADATA_API: &str =
    "https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/maven-metadata.xml";
const CACHE_TTL_SECS: i64 = 24 * 3600; // 24 hours

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedToolVersions {
    pub gradle_latest: String,
    pub gradle_legacy_latest: String,
    pub maven_latest: String,
    pub fetched_at: i64,
}

impl Default for CachedToolVersions {
    fn default() -> Self {
        Self {
            gradle_latest: DEFAULT_GRADLE_VERSION.to_string(),
            gradle_legacy_latest: "8.14.8".to_string(),
            maven_latest: DEFAULT_MAVEN_VERSION.to_string(),
            fetched_at: 0,
        }
    }
}

pub fn load_cached_tool_versions(paths: &DjukePaths) -> Option<CachedToolVersions> {
    let cache_file = paths.cache_dir().join("tool_versions.json");
    if cache_file.is_file() {
        if let Ok(content) = fs::read_to_string(&cache_file) {
            if let Ok(data) = serde_json::from_str::<CachedToolVersions>(&content) {
                return Some(data);
            }
        }
    }
    None
}

pub fn save_cached_tool_versions(paths: &DjukePaths, cached: &CachedToolVersions) {
    let cache_dir = paths.cache_dir();
    if fs::create_dir_all(&cache_dir).is_ok() {
        let cache_file = cache_dir.join("tool_versions.json");
        if let Ok(json) = serde_json::to_string_pretty(cached) {
            let _ = fs::write(cache_file, json);
        }
    }
}

#[derive(Deserialize)]
struct GradleVersionResponse {
    version: String,
}

pub async fn fetch_latest_gradle_online() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(2000))
        .user_agent("djuke/0.1.0")
        .build()
        .ok()?;

    let res = client.get(GRADLE_CURRENT_API).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }

    let parsed: GradleVersionResponse = res.json().await.ok()?;
    if !parsed.version.trim().is_empty() {
        Some(parsed.version.trim().to_string())
    } else {
        None
    }
}

pub async fn fetch_latest_legacy_gradle_online() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(2000))
        .user_agent("djuke/0.1.0")
        .build()
        .ok()?;

    let res = client.get(GRADLE_ALL_API).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }

    let list: Vec<GradleVersionResponse> = res.json().await.ok()?;
    let mut v8_versions: Vec<String> = list
        .into_iter()
        .map(|item| item.version)
        .filter(|v| v.starts_with("8.") && !v.contains('-'))
        .collect();

    v8_versions.sort_by(|a, b| version_compare(b, a));
    v8_versions.into_iter().next()
}

pub async fn fetch_latest_maven_online() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(2000))
        .user_agent("djuke/0.1.0")
        .build()
        .ok()?;

    let res = client.get(MAVEN_METADATA_API).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }

    let xml = res.text().await.ok()?;
    parse_latest_maven_from_metadata(&xml)
}

pub fn parse_latest_maven_from_metadata(xml: &str) -> Option<String> {
    let mut versions = Vec::new();
    for line in xml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<version>") && trimmed.ends_with("</version>") {
            let ver = trimmed
                .trim_start_matches("<version>")
                .trim_end_matches("</version>")
                .trim();
            // Focus on stable 3.x releases (e.g. 3.9.16), ignoring RCs/alphas
            if ver.starts_with("3.") && !ver.contains('-') {
                versions.push(ver.to_string());
            }
        }
    }

    versions.sort_by(|a, b| version_compare(b, a));
    versions.into_iter().next()
}

fn version_compare(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u64> {
        s.split(['.', '-'])
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    };
    let av = parse(a);
    let bv = parse(b);
    av.cmp(&bv)
}

/// Dynamically resolves the actual latest Gradle version at the time of execution.
/// Follows: Online Fetch -> Valid 24h Local Cache -> Baseline Embedded Constant.
pub async fn resolve_dynamic_gradle_version(
    target_java_version: &str,
    paths: Option<&DjukePaths>,
    allow_network: bool,
) -> String {
    let major = crate::bootstrap::extract_java_major(target_java_version).unwrap_or(21);
    let is_legacy = major <= 16;

    let now = Utc::now().timestamp();
    let cached = paths.and_then(load_cached_tool_versions);

    // 1. If cache is fresh (< 24h), use it immediately
    if let Some(ref c) = cached {
        if (now - c.fetched_at).abs() < CACHE_TTL_SECS {
            return if is_legacy {
                c.gradle_legacy_latest.clone()
            } else {
                c.gradle_latest.clone()
            };
        }
    }

    // 2. Query online if network is permitted
    if allow_network {
        if is_legacy {
            if let Some(v) = fetch_latest_legacy_gradle_online().await {
                if let Some(p) = paths {
                    let mut updated = cached.unwrap_or_default();
                    updated.gradle_legacy_latest = v.clone();
                    updated.fetched_at = now;
                    save_cached_tool_versions(p, &updated);
                }
                return v;
            }
        } else if let Some(v) = fetch_latest_gradle_online().await {
            if let Some(p) = paths {
                let mut updated = cached.unwrap_or_default();
                updated.gradle_latest = v.clone();
                updated.fetched_at = now;
                save_cached_tool_versions(p, &updated);
            }
            return v;
        }
    }

    // 3. Fall back to cached (even if older) or baseline default
    if let Some(c) = cached {
        if is_legacy && !c.gradle_legacy_latest.is_empty() {
            return c.gradle_legacy_latest;
        }
        if !is_legacy && !c.gradle_latest.is_empty() {
            return c.gradle_latest;
        }
    }

    crate::bootstrap::compatible_gradle_version_for_java(target_java_version).to_string()
}

/// Dynamically resolves the actual latest Maven version at the time of execution.
/// Follows: Online Fetch -> Valid 24h Local Cache -> Baseline Embedded Constant.
pub async fn resolve_dynamic_maven_version(
    paths: Option<&DjukePaths>,
    allow_network: bool,
) -> String {
    let now = Utc::now().timestamp();
    let cached = paths.and_then(load_cached_tool_versions);

    if let Some(ref c) = cached {
        if (now - c.fetched_at).abs() < CACHE_TTL_SECS && !c.maven_latest.is_empty() {
            return c.maven_latest.clone();
        }
    }

    if allow_network {
        if let Some(v) = fetch_latest_maven_online().await {
            if let Some(p) = paths {
                let mut updated = cached.unwrap_or_default();
                updated.maven_latest = v.clone();
                updated.fetched_at = now;
                save_cached_tool_versions(p, &updated);
            }
            return v;
        }
    }

    if let Some(c) = cached {
        if !c.maven_latest.is_empty() {
            return c.maven_latest;
        }
    }

    DEFAULT_MAVEN_VERSION.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_latest_maven_from_metadata() {
        let sample_xml = r#"
        <metadata>
          <versioning>
            <latest>4.0.0-rc-7</latest>
            <release>4.0.0-rc-7</release>
            <versions>
              <version>3.9.14</version>
              <version>3.9.15</version>
              <version>3.9.16</version>
              <version>4.0.0-rc-1</version>
              <version>4.0.0-rc-7</version>
            </versions>
          </versioning>
        </metadata>
        "#;

        let parsed = parse_latest_maven_from_metadata(sample_xml);
        assert_eq!(parsed, Some("3.9.16".to_string()));
    }

    #[test]
    fn test_version_compare() {
        assert_eq!(
            version_compare("9.9.2", "9.9.1"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            version_compare("9.10.0", "9.9.2"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            version_compare("3.9.16", "3.9.9"),
            std::cmp::Ordering::Greater
        );
    }
}
