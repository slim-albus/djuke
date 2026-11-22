use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use djuke_core::{DjukeError, JdkDistribution, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub java: Option<GlobalJavaConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalJavaConfig {
    pub default_version: Option<String>,
    pub default_distribution: Option<JdkDistribution>,
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
