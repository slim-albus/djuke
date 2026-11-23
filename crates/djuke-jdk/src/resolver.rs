use djuke_core::{DjukeError, DjukePaths, InstalledJdk, JdkSpec, Result};

use crate::installer::install_jdk;
use crate::store::JdkStore;

pub async fn resolve_jdk(
    spec: &JdkSpec,
    paths: &DjukePaths,
    auto_install: bool,
    quiet: bool,
) -> Result<InstalledJdk> {
    let store = JdkStore::new(paths.clone());

    if let Some(installed) = store.find_installed(spec) {
        return Ok(installed);
    }

    if auto_install {
        install_jdk(spec, paths, quiet).await
    } else {
        Err(DjukeError::JdkNotInstalled(format!(
            "{}@{}",
            spec.distribution, spec.version
        )))
    }
}
