use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::os::Os;
use djuke_core::Result;

#[derive(Debug, Clone)]
pub struct DjukePaths {
    pub root: PathBuf,
}

impl DjukePaths {
    pub fn discover() -> Self {
        let root = if let Ok(custom) = env::var("DJUKE_HOME") {
            PathBuf::from(custom)
        } else if let Some(home) = dirs::home_dir() {
            home.join(".djuke")
        } else {
            PathBuf::from(".djuke")
        };

        Self { root }
    }

    pub fn jdks_dir(&self) -> PathBuf {
        self.root.join("jdks")
    }

    pub fn tools_dir(&self) -> PathBuf {
        self.root.join("tools")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(self.jdks_dir())?;
        fs::create_dir_all(self.tools_dir())?;
        fs::create_dir_all(self.cache_dir())?;
        fs::create_dir_all(self.tmp_dir())?;
        Ok(())
    }

    pub fn effective_java_home(installed_dir: &Path) -> PathBuf {
        let mac_home = installed_dir.join("Contents").join("Home");
        if mac_home.is_dir() {
            mac_home
        } else {
            installed_dir.to_path_buf()
        }
    }

    pub fn java_executable(java_home: &Path) -> PathBuf {
        let home = Self::effective_java_home(java_home);
        if Os::current() == Os::Windows {
            home.join("bin").join("java.exe")
        } else {
            home.join("bin").join("java")
        }
    }
}
