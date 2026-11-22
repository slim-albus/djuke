use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use crate::error::{DjukeError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JdkDistribution {
    Temurin,
    OpenJdk,
    Corretto,
    Zulu,
    Microsoft,
    Semeru,
    Graalvm,
    Liberica,
    Oracle,
    Sapmachine,
}

impl JdkDistribution {
    pub fn as_disco_name(&self) -> &'static str {
        match self {
            Self::Temurin => "temurin",
            Self::OpenJdk => "oracle_open_jdk",
            Self::Corretto => "corretto",
            Self::Zulu => "zulu",
            Self::Microsoft => "microsoft",
            Self::Semeru => "semeru",
            Self::Graalvm => "graalvm_ce",
            Self::Liberica => "liberica",
            Self::Oracle => "oracle",
            Self::Sapmachine => "sap_machine",
        }
    }

    pub fn default_distro() -> Self {
        Self::OpenJdk
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Temurin,
            Self::OpenJdk,
            Self::Corretto,
            Self::Zulu,
            Self::Microsoft,
            Self::Semeru,
            Self::Graalvm,
            Self::Liberica,
            Self::Oracle,
            Self::Sapmachine,
        ]
    }
}

impl fmt::Display for JdkDistribution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temurin => write!(f, "temurin"),
            Self::OpenJdk => write!(f, "openjdk"),
            Self::Corretto => write!(f, "corretto"),
            Self::Zulu => write!(f, "zulu"),
            Self::Microsoft => write!(f, "microsoft"),
            Self::Semeru => write!(f, "semeru"),
            Self::Graalvm => write!(f, "graalvm"),
            Self::Liberica => write!(f, "liberica"),
            Self::Oracle => write!(f, "oracle"),
            Self::Sapmachine => write!(f, "sapmachine"),
        }
    }
}

impl FromStr for JdkDistribution {
    type Err = DjukeError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "temurin" | "adoptium" => Ok(Self::Temurin),
            "openjdk" | "open_jdk" | "open-jdk" | "oracle_open_jdk" | "oracle-openjdk" => {
                Ok(Self::OpenJdk)
            }
            "corretto" | "amazon" => Ok(Self::Corretto),
            "zulu" | "azul" => Ok(Self::Zulu),
            "microsoft" | "ms" => Ok(Self::Microsoft),
            "semeru" | "ibm" => Ok(Self::Semeru),
            "graalvm" | "graal" => Ok(Self::Graalvm),
            "liberica" | "bellsoft" => Ok(Self::Liberica),
            "oracle" => Ok(Self::Oracle),
            "sapmachine" | "sap" | "sap_machine" => Ok(Self::Sapmachine),
            other => Err(DjukeError::InvalidJdkSpec(format!(
                "Unknown distribution '{}'. Supported: temurin, openjdk, corretto, zulu, microsoft, semeru, graalvm, liberica, oracle, sapmachine",
                other
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JdkSpec {
    pub distribution: JdkDistribution,
    pub version: String,
}

impl JdkSpec {
    pub fn new(distribution: JdkDistribution, version: impl Into<String>) -> Self {
        Self {
            distribution,
            version: version.into(),
        }
    }
}

impl fmt::Display for JdkSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.distribution, self.version)
    }
}

impl FromStr for JdkSpec {
    type Err = DjukeError;

    fn from_str(s: &str) -> Result<Self> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(DjukeError::InvalidJdkSpec(
                "JDK spec cannot be empty".into(),
            ));
        }

        if let Some((distro_str, ver_str)) = trimmed.split_once('@') {
            let distro = JdkDistribution::from_str(distro_str)?;
            let ver = ver_str.trim().to_string();
            if ver.is_empty() {
                return Err(DjukeError::InvalidJdkSpec(
                    "Missing version after '@'".into(),
                ));
            }
            Ok(Self {
                distribution: distro,
                version: ver,
            })
        } else {
            // Defaults to Temurin if no distro is prefixed
            Ok(Self {
                distribution: JdkDistribution::default_distro(),
                version: trimmed.to_string(),
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledJdk {
    pub id: String,
    pub distribution: JdkDistribution,
    pub version: String,
    pub major_version: u32,
    pub os: String,
    pub arch: String,
    pub java_home: PathBuf,
    pub installed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildTool {
    Gradle,
    Maven,
}

impl fmt::Display for BuildTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gradle => write!(f, "gradle"),
            Self::Maven => write!(f, "maven"),
        }
    }
}

impl FromStr for BuildTool {
    type Err = DjukeError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "gradle" | "gradle-kts" | "gradlew" => Ok(Self::Gradle),
            "maven" | "mvn" | "mvnw" => Ok(Self::Maven),
            other => Err(DjukeError::Generic(format!(
                "Unknown build tool '{}'. Supported: gradle, maven",
                other
            ))),
        }
    }
}

pub type TaskMap = HashMap<String, String>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub project: Option<ProjectSection>,
    pub java: Option<JavaSection>,
    pub build: Option<BuildSection>,
    pub tasks: Option<TaskMap>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSection {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaSection {
    pub version: String,
    pub distribution: Option<JdkDistribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSection {
    pub tool: Option<BuildTool>,
    pub version: Option<String>,
    pub wrapper: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jdk_spec_parsing() {
        let spec1: JdkSpec = "21".parse().unwrap();
        assert_eq!(spec1.distribution, JdkDistribution::OpenJdk);
        assert_eq!(spec1.version, "21");

        let spec2: JdkSpec = "corretto@17".parse().unwrap();
        assert_eq!(spec2.distribution, JdkDistribution::Corretto);
        assert_eq!(spec2.version, "17");

        let spec3: JdkSpec = "zulu@21.0.4".parse().unwrap();
        assert_eq!(spec3.distribution, JdkDistribution::Zulu);
        assert_eq!(spec3.version, "21.0.4");

        assert!("invalid@17".parse::<JdkSpec>().is_err());
        assert!("@17".parse::<JdkSpec>().is_err());
    }
}
