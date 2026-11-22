use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use djuke_core::{DjukeError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyScope {
    Implementation,
    CompileOnly,
    RuntimeOnly,
    Test,
}

impl DependencyScope {
    pub fn as_gradle_scope(&self) -> &'static str {
        match self {
            Self::Implementation => "implementation",
            Self::CompileOnly => "compileOnly",
            Self::RuntimeOnly => "runtimeOnly",
            Self::Test => "testImplementation",
        }
    }

    pub fn as_maven_scope(&self) -> Option<&'static str> {
        match self {
            Self::Implementation => None,
            Self::CompileOnly => Some("provided"),
            Self::RuntimeOnly => Some("runtime"),
            Self::Test => Some("test"),
        }
    }
}

impl fmt::Display for DependencyScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Implementation => write!(f, "implementation"),
            Self::CompileOnly => write!(f, "compileOnly"),
            Self::RuntimeOnly => write!(f, "runtimeOnly"),
            Self::Test => write!(f, "test"),
        }
    }
}

impl FromStr for DependencyScope {
    type Err = DjukeError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "implementation" | "compile" | "default" => Ok(Self::Implementation),
            "compileonly" | "provided" => Ok(Self::CompileOnly),
            "runtimeonly" | "runtime" => Ok(Self::RuntimeOnly),
            "test" | "testimplementation" => Ok(Self::Test),
            other => Err(DjukeError::Generic(format!(
                "Unknown dependency scope '{}'. Supported: implementation, test, compileOnly, runtimeOnly",
                other
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencySpec {
    pub group_id: String,
    pub artifact_id: String,
    pub version: Option<String>,
    pub scope: DependencyScope,
}

impl fmt::Display for DependencySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(v) = &self.version {
            write!(f, "{}:{}:{}", self.group_id, self.artifact_id, v)
        } else {
            write!(f, "{}:{}", self.group_id, self.artifact_id)
        }
    }
}

impl FromStr for DependencySpec {
    type Err = DjukeError;

    fn from_str(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.len() {
            2 => Ok(Self {
                group_id: parts[0].trim().to_string(),
                artifact_id: parts[1].trim().to_string(),
                version: None,
                scope: DependencyScope::Implementation,
            }),
            3 => Ok(Self {
                group_id: parts[0].trim().to_string(),
                artifact_id: parts[1].trim().to_string(),
                version: Some(parts[2].trim().to_string()),
                scope: DependencyScope::Implementation,
            }),
            1 => {
                let single = parts[0].trim();
                // Common shortcuts
                match single {
                    "guava" => Ok(Self {
                        group_id: "com.google.guava".into(),
                        artifact_id: "guava".into(),
                        version: None,
                        scope: DependencyScope::Implementation,
                    }),
                    "lombok" => Ok(Self {
                        group_id: "org.projectlombok".into(),
                        artifact_id: "lombok".into(),
                        version: None,
                        scope: DependencyScope::CompileOnly,
                    }),
                    "junit" | "junit5" => Ok(Self {
                        group_id: "org.junit.jupiter".into(),
                        artifact_id: "junit-jupiter".into(),
                        version: None,
                        scope: DependencyScope::Test,
                    }),
                    "jackson" => Ok(Self {
                        group_id: "com.fasterxml.jackson.core".into(),
                        artifact_id: "jackson-databind".into(),
                        version: None,
                        scope: DependencyScope::Implementation,
                    }),
                    "postgres" | "postgresql" => Ok(Self {
                        group_id: "org.postgresql".into(),
                        artifact_id: "postgresql".into(),
                        version: None,
                        scope: DependencyScope::Implementation,
                    }),
                    _ => Err(DjukeError::Generic(format!(
                        "Invalid dependency coordinate '{}'. Expected 'groupId:artifactId' or 'groupId:artifactId:version'",
                        single
                    ))),
                }
            }
            _ => Err(DjukeError::Generic(format!(
                "Invalid dependency coordinate '{}'. Expected 'groupId:artifactId[:version]'",
                s
            ))),
        }
    }
}
