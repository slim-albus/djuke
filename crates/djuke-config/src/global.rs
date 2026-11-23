use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use djuke_core::{DjukeError, JdkDistribution, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub java: Option<GlobalJavaConfig>,
    pub defaults: Option<GlobalDefaultsConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalJavaConfig {
    pub default_version: Option<String>,
    pub default_distribution: Option<JdkDistribution>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalDefaultsConfig {
    pub distribution: Option<JdkDistribution>,
    pub java_version: Option<String>,
    pub build_tool: Option<String>,
    pub test_framework: Option<String>,
}

impl GlobalConfig {
    pub fn default_distribution(&self) -> JdkDistribution {
        self.defaults
            .as_ref()
            .and_then(|d| d.distribution)
            .or_else(|| self.java.as_ref().and_then(|j| j.default_distribution))
            .unwrap_or_else(JdkDistribution::default_distro)
    }

    pub fn default_java_version(&self) -> String {
        self.defaults
            .as_ref()
            .and_then(|d| d.java_version.clone())
            .or_else(|| self.java.as_ref().and_then(|j| j.default_version.clone()))
            .unwrap_or_else(|| "21".to_string())
    }

    pub fn default_build_tool(&self) -> Option<String> {
        self.defaults.as_ref().and_then(|d| d.build_tool.clone())
    }

    pub fn default_test_framework(&self) -> Option<String> {
        self.defaults
            .as_ref()
            .and_then(|d| d.test_framework.clone())
    }

    pub fn set_property(&mut self, key: &str, value: &str) -> Result<()> {
        let trimmed_key = key.trim().to_ascii_lowercase();
        let trimmed_val = value.trim();
        match trimmed_key.as_str() {
            "distribution"
            | "distro"
            | "default_distribution"
            | "java.default_distribution"
            | "java.distribution"
            | "defaults.distribution" => {
                let d: JdkDistribution = trimmed_val.parse()?;
                let mut j = self.java.clone().unwrap_or_default();
                j.default_distribution = Some(d);
                self.java = Some(j);
                let mut def = self.defaults.clone().unwrap_or_default();
                def.distribution = Some(d);
                self.defaults = Some(def);
            }
            "version"
            | "java_version"
            | "default_version"
            | "java.default_version"
            | "java.version"
            | "defaults.java_version" => {
                let mut j = self.java.clone().unwrap_or_default();
                j.default_version = Some(trimmed_val.to_string());
                self.java = Some(j);
                let mut def = self.defaults.clone().unwrap_or_default();
                def.java_version = Some(trimmed_val.to_string());
                self.defaults = Some(def);
            }
            "build_tool" | "tool" | "defaults.build_tool" => {
                let mut d = self.defaults.clone().unwrap_or_default();
                d.build_tool = Some(trimmed_val.to_string());
                self.defaults = Some(d);
            }
            "test_framework" | "test" | "defaults.test_framework" => {
                let mut d = self.defaults.clone().unwrap_or_default();
                d.test_framework = Some(trimmed_val.to_string());
                self.defaults = Some(d);
            }
            other => {
                return Err(DjukeError::Config(format!(
                    "Unknown configuration setting '{}'. Available settings: distribution, version, build_tool, test_framework",
                    other
                )));
            }
        }
        Ok(())
    }

    pub fn get_property(&self, key: &str) -> Option<String> {
        let trimmed_key = key.trim().to_ascii_lowercase();
        match trimmed_key.as_str() {
            "distribution"
            | "distro"
            | "default_distribution"
            | "java.default_distribution"
            | "java.distribution" => Some(self.default_distribution().to_string()),
            "version"
            | "java_version"
            | "default_version"
            | "java.default_version"
            | "java.version" => Some(self.default_java_version()),
            "build_tool" | "tool" | "defaults.build_tool" => self.default_build_tool(),
            "test_framework" | "test" | "defaults.test_framework" => self.default_test_framework(),
            _ => None,
        }
    }
}

pub fn load_global_config(path: &Path) -> Result<GlobalConfig> {
    if !path.is_file() {
        return Ok(GlobalConfig::default());
    }
    let content = fs::read_to_string(path)?;
    let cfg: GlobalConfig = toml::from_str(&content).map_err(|e| {
        DjukeError::Config(format!(
            "Failed to parse global config at '{}': {}",
            path.display(),
            e
        ))
    })?;
    Ok(cfg)
}

pub fn save_global_config(path: &Path, config: &GlobalConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let toml_str =
        toml::to_string_pretty(config).map_err(|e| DjukeError::Serialization(e.to_string()))?;
    fs::write(path, toml_str)?;
    Ok(())
}
