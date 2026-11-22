use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use djuke_core::{DjukeError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemplateType {
    Cli,
    Lib,
    SpringBoot,
    Quarkus,
    Micronaut,
}

impl fmt::Display for TemplateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cli => write!(f, "cli"),
            Self::Lib => write!(f, "lib"),
            Self::SpringBoot => write!(f, "spring-boot"),
            Self::Quarkus => write!(f, "quarkus"),
            Self::Micronaut => write!(f, "micronaut"),
        }
    }
}

impl FromStr for TemplateType {
    type Err = DjukeError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "cli" | "app" | "console" => Ok(Self::Cli),
            "lib" | "library" => Ok(Self::Lib),
            "spring" | "spring-boot" | "springboot" => Ok(Self::SpringBoot),
            "quarkus" => Ok(Self::Quarkus),
            "micronaut" => Ok(Self::Micronaut),
            other => Err(DjukeError::Generic(format!(
                "Unknown template '{}'. Available: cli, lib, spring-boot, quarkus, micronaut",
                other
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildToolChoice {
    GradleKts,
    GradleGroovy,
    Maven,
}

impl fmt::Display for BuildToolChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GradleKts => write!(f, "gradle-kts"),
            Self::GradleGroovy => write!(f, "gradle-groovy"),
            Self::Maven => write!(f, "maven"),
        }
    }
}

impl FromStr for BuildToolChoice {
    type Err = DjukeError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "gradle" | "gradle-kts" | "kts" => Ok(Self::GradleKts),
            "gradle-groovy" | "groovy" => Ok(Self::GradleGroovy),
            "maven" | "mvn" => Ok(Self::Maven),
            other => Err(DjukeError::Generic(format!(
                "Unknown build tool choice '{}'. Available: gradle-kts, gradle-groovy, maven",
                other
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewProjectOptions {
    pub name: String,
    pub group_id: String,
    pub template: TemplateType,
    pub build_tool: BuildToolChoice,
    pub java_version: String,
    pub destination: PathBuf,
}
