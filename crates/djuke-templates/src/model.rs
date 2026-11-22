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
    Javalin,
    Grpc,
    Picocli,
    MultiModule,
}

impl fmt::Display for TemplateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cli => write!(f, "cli"),
            Self::Lib => write!(f, "lib"),
            Self::SpringBoot => write!(f, "spring-boot"),
            Self::Quarkus => write!(f, "quarkus"),
            Self::Micronaut => write!(f, "micronaut"),
            Self::Javalin => write!(f, "javalin"),
            Self::Grpc => write!(f, "grpc"),
            Self::Picocli => write!(f, "picocli"),
            Self::MultiModule => write!(f, "multi-module"),
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
            "javalin" | "rest" | "embedded" => Ok(Self::Javalin),
            "grpc" | "proto" | "protobuf" => Ok(Self::Grpc),
            "picocli" | "cli-rich" | "cli-app" => Ok(Self::Picocli),
            "multi-module" | "multimodule" | "modular" => Ok(Self::MultiModule),
            other => Err(DjukeError::Generic(format!(
                "Unknown template '{}'. Available: cli, lib, spring-boot, quarkus, micronaut, javalin, grpc, picocli, multi-module",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TestFramework {
    #[default]
    JUnit5,
    AssertJ,
    Spock,
    TestNG,
    None,
}

impl fmt::Display for TestFramework {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JUnit5 => write!(f, "junit5"),
            Self::AssertJ => write!(f, "assertj"),
            Self::Spock => write!(f, "spock"),
            Self::TestNG => write!(f, "testng"),
            Self::None => write!(f, "none"),
        }
    }
}

impl FromStr for TestFramework {
    type Err = DjukeError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "junit" | "junit5" | "jupiter" => Ok(Self::JUnit5),
            "assertj" | "assert-j" => Ok(Self::AssertJ),
            "spock" => Ok(Self::Spock),
            "testng" | "test-ng" => Ok(Self::TestNG),
            "none" | "skip" | "no" | "false" => Ok(Self::None),
            other => Err(DjukeError::Generic(format!(
                "Unknown test framework '{}'. Available: junit5, assertj, spock, testng, none",
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
    pub distribution: Option<djuke_core::JdkDistribution>,
    pub destination: PathBuf,
    pub framework_version: Option<String>,
    pub initializr_dependencies: Option<Vec<String>>,
    pub skip_initializr: bool,
    pub test_framework: TestFramework,
}

impl NewProjectOptions {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        group_id: impl Into<String>,
        template: TemplateType,
        build_tool: BuildToolChoice,
        java_version: impl Into<String>,
        destination: PathBuf,
    ) -> Self {
        Self {
            name: name.into(),
            group_id: group_id.into(),
            template,
            build_tool,
            java_version: java_version.into(),
            distribution: None,
            destination,
            framework_version: None,
            initializr_dependencies: None,
            skip_initializr: false,
            test_framework: TestFramework::JUnit5,
        }
    }
}
