use chrono::Utc;
use djuke_archive::extract_archive;
use djuke_core::{DjukeError, InstalledJdk, JdkSpec, Result};
use djuke_download::Downloader;
use djuke_platform::{DjukePaths, Platform};

use crate::disco::DiscoClient;
use crate::store::JdkStore;

pub async fn install_jdk(spec: &JdkSpec, paths: &DjukePaths, quiet: bool) -> Result<InstalledJdk> {
    paths.ensure_dirs()?;
    let store = JdkStore::new(paths.clone());

    // If already installed, return immediately
    if let Some(existing) = store.find_installed(spec) {
        return Ok(existing);
    }

    let disco = DiscoClient::new();
    let platform = Platform::current();

    if !quiet {
        eprintln!(
            "Resolving JDK {} ({}) for {}...",
            spec.version, spec.distribution, platform
        );
    }

    let package = disco
        .find_best_package(&spec.version, spec.distribution, platform)
        .await?;

    let download_url = package
        .download_link
        .ok_or_else(|| DjukeError::JdkNotFound {
            version: spec.version.clone(),
            distro: spec.distribution.to_string(),
            os: platform.os.to_string(),
            arch: platform.arch.to_string(),
        })?;

    let filename = package
        .filename
        .unwrap_or_else(|| "jdk_archive".to_string());
    let temp_archive_path = paths.cache_dir().join(&filename);

    let checksum = if package
        .checksum_type
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case("sha256"))
        .unwrap_or(true)
    {
        package.checksum.as_deref().filter(|c| !c.trim().is_empty())
    } else {
        None
    };

    let downloader = Downloader::new(quiet);
    downloader
        .download_file(
            &download_url,
            &temp_archive_path,
            checksum,
            &format!("JDK {} ({})", spec.version, spec.distribution),
        )
        .await?;

    // Determine target installation folder name
    let ver_str = package
        .java_version
        .or(package.distribution_version)
        .unwrap_or_else(|| spec.version.clone());
    let major = package.major_version.unwrap_or_else(|| {
        ver_str
            .split('.')
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or(21)
    });

    let folder_name = format!("{}-{}", spec.distribution, ver_str);
    let target_dir = paths.jdks_dir().join(&folder_name);

    if !quiet {
        eprintln!("Extracting to {}...", target_dir.display());
    }

    extract_archive(&temp_archive_path, &target_dir)?;

    let effective_home = DjukePaths::effective_java_home(&target_dir);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bin_dir = effective_home.join("bin");
        if let Ok(entries) = std::fs::read_dir(bin_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        let mut perms = meta.permissions();
                        perms.set_mode(0o755);
                        let _ = std::fs::set_permissions(entry.path(), perms);
                    }
                }
            }
        }
    }

    let installed = InstalledJdk {
        id: folder_name,
        distribution: spec.distribution,
        version: ver_str,
        major_version: major,
        os: platform.os.to_string(),
        arch: platform.arch.to_string(),
        java_home: effective_home,
        installed_at: Utc::now(),
    };

    store.save_jdk_metadata(&target_dir, &installed)?;

    if !quiet {
        eprintln!(
            "✔ Successfully installed {} to {}",
            installed.id,
            target_dir.display()
        );
    }

    Ok(installed)
}
