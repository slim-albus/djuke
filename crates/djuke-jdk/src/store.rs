use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

use djuke_core::{
    DjukeError, DjukePaths, InstalledJdk, JdkDistribution, JdkSpec, Platform, Result,
};

pub struct JdkStore {
    paths: DjukePaths,
}

impl JdkStore {
    pub fn new(paths: DjukePaths) -> Self {
        Self { paths }
    }

    pub fn list_installed(&self) -> Result<Vec<InstalledJdk>> {
        let jdks_dir = self.paths.jdks_dir();
        if !jdks_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut list = Vec::new();
        for entry in fs::read_dir(jdks_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(jdk) = self.read_jdk_metadata(&path) {
                    list.push(jdk);
                }
            }
        }

        list.sort_by_key(|a| std::cmp::Reverse(a.major_version));
        Ok(list)
    }

    pub fn find_installed(&self, spec: &JdkSpec) -> Option<InstalledJdk> {
        let installed = self.list_installed().ok()?;
        if spec.version.eq_ignore_ascii_case("latest") {
            return installed
                .into_iter()
                .filter(|jdk| jdk.distribution == spec.distribution)
                .max_by_key(|jdk| jdk.major_version);
        }
        if spec.version.eq_ignore_ascii_case("lts") {
            return installed
                .into_iter()
                .filter(|jdk| {
                    jdk.distribution == spec.distribution
                        && matches!(jdk.major_version, 25 | 21 | 17 | 11 | 8)
                })
                .max_by_key(|jdk| jdk.major_version);
        }
        installed.into_iter().find(|jdk| {
            let distro_match = jdk.distribution == spec.distribution;
            let ver_match = jdk.major_version.to_string() == spec.version
                || jdk.version == spec.version
                || jdk.version.starts_with(&format!("{}.", spec.version));
            distro_match && ver_match
        })
    }

    pub fn save_jdk_metadata(&self, dir: &Path, jdk: &InstalledJdk) -> Result<()> {
        let metadata_path = dir.join(".djuke-jdk.json");
        let json_str = serde_json::to_string_pretty(jdk)
            .map_err(|e| DjukeError::Serialization(e.to_string()))?;
        fs::write(metadata_path, json_str)?;
        Ok(())
    }

    pub fn read_jdk_metadata(&self, dir: &Path) -> Option<InstalledJdk> {
        let metadata_path = dir.join(".djuke-jdk.json");
        if metadata_path.is_file() {
            if let Ok(content) = fs::read_to_string(&metadata_path) {
                if let Ok(jdk) = serde_json::from_str::<InstalledJdk>(&content) {
                    return Some(jdk);
                }
            }
        }

        // Fallback: infer from folder name and contents
        let folder_name = dir.file_name()?.to_string_lossy();
        let java_bin = DjukePaths::java_executable(dir);
        if java_bin.is_file() {
            let (distro, ver) = if let Some((d, v)) = folder_name.split_once('-') {
                (
                    d.parse()
                        .unwrap_or_else(|_| JdkDistribution::default_distro()),
                    v.to_string(),
                )
            } else {
                (JdkDistribution::default_distro(), folder_name.to_string())
            };
            let major: u32 = ver
                .split('.')
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(21);

            let platform = Platform::current();
            let effective_home = DjukePaths::effective_java_home(dir);
            Some(InstalledJdk {
                id: folder_name.to_string(),
                distribution: distro,
                version: ver,
                major_version: major,
                os: platform.os.to_string(),
                arch: platform.arch.to_string(),
                java_home: effective_home,
                installed_at: Utc::now(),
            })
        } else {
            None
        }
    }

    pub fn uninstall(&self, target_id_or_version: &str) -> Result<PathBuf> {
        let installed = self.list_installed()?;
        let target = installed
            .into_iter()
            .find(|jdk| {
                jdk.id == target_id_or_version
                    || jdk.version == target_id_or_version
                    || jdk.major_version.to_string() == target_id_or_version
            })
            .ok_or_else(|| {
                DjukeError::JdkNotInstalled(format!("No JDK matching '{}'", target_id_or_version))
            })?;

        let install_dir = self.paths.jdks_dir().join(&target.id);
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir)?;
        } else if target.java_home.exists() {
            fs::remove_dir_all(&target.java_home)?;
        }
        Ok(target.java_home)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_jdk_store_save_and_find() {
        let dir = tempdir().unwrap();
        let paths = DjukePaths {
            root: dir.path().to_path_buf(),
        };
        paths.ensure_dirs().unwrap();

        let store = JdkStore::new(paths.clone());
        let target_dir = paths.jdks_dir().join("temurin-21.0.4");
        fs::create_dir_all(&target_dir).unwrap();

        let jdk = InstalledJdk {
            id: "temurin-21.0.4".into(),
            distribution: JdkDistribution::Temurin,
            version: "21.0.4".into(),
            major_version: 21,
            os: "windows".into(),
            arch: "x64".into(),
            java_home: target_dir.clone(),
            installed_at: Utc::now(),
        };

        store.save_jdk_metadata(&target_dir, &jdk).unwrap();

        let found = store.find_installed(&JdkSpec::new(JdkDistribution::Temurin, "21"));
        assert!(found.is_some());
        let found_jdk = found.unwrap();
        assert_eq!(found_jdk.version, "21.0.4");
        assert_eq!(found_jdk.distribution, JdkDistribution::Temurin);
    }
}
