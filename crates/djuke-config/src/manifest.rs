use std::fs;
use std::path::{Path, PathBuf};

use djuke_core::{DjukeError, ProjectManifest, Result};

pub fn find_manifest_file(dir: &Path) -> Option<PathBuf> {
    let djuke_toml = dir.join("djuke.toml");
    if djuke_toml.is_file() {
        return Some(djuke_toml);
    }
    let juke_toml = dir.join("juke.toml");
    if juke_toml.is_file() {
        return Some(juke_toml);
    }
    None
}

pub fn load_manifest(path: &Path) -> Result<ProjectManifest> {
    let content = fs::read_to_string(path)?;
    let manifest: ProjectManifest = toml::from_str(&content).map_err(|e| {
        DjukeError::Config(format!(
            "Failed to parse manifest at '{}': {}",
            path.display(),
            e
        ))
    })?;
    Ok(manifest)
}

pub fn save_manifest(path: &Path, manifest: &ProjectManifest) -> Result<()> {
    let toml_str =
        toml::to_string_pretty(manifest).map_err(|e| DjukeError::Serialization(e.to_string()))?;
    fs::write(path, toml_str)?;
    Ok(())
}
