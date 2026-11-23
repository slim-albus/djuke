use colored::Colorize;
use djuke_core::{DjukeError, Result};
use djuke_core::{Arch, Os, Platform};
use djuke_io::Downloader;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const REPO_OWNER: &str = "slim-albus";
const REPO_NAME: &str = "djuke";

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: Option<String>,
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Run self-upgrade checking GitHub releases.
pub async fn run_self_upgrade(check_only: bool, force: bool, quiet: bool) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");

    if !quiet {
        println!(
            "{} Checking for djuke updates (current: {})...",
            "⚡".cyan(),
            format!("v{}", current_version).bold()
        );
    }

    let client = reqwest::Client::builder()
        .user_agent("djuke-self-updater")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| DjukeError::Network(format!("Failed to build HTTP client: {}", e)))?;

    let release_url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        REPO_OWNER, REPO_NAME
    );

    let resp = client.get(&release_url).send().await.map_err(|e| {
        DjukeError::Network(format!(
            "Failed to connect to GitHub releases ({}): {}",
            release_url, e
        ))
    })?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        println!(
            "{} No published releases found on GitHub repository {}/{}.\nYou are running {} (latest local build).",
            "ℹ".yellow(),
            REPO_OWNER,
            REPO_NAME,
            format!("v{}", current_version).green()
        );
        return Ok(());
    }

    if !resp.status().is_success() {
        return Err(DjukeError::Network(format!(
            "GitHub releases API returned HTTP {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        )));
    }

    let release: GithubRelease = resp.json().await.map_err(|e| {
        DjukeError::Serialization(format!("Failed to parse GitHub release JSON: {}", e))
    })?;

    let latest_version = release.tag_name.trim_start_matches('v');
    let has_update = is_newer_version(latest_version, current_version);

    if !has_update && !force {
        println!(
            "{} djuke is already up to date! ({})",
            "✔".green().bold(),
            format!("v{}", current_version).cyan()
        );
        return Ok(());
    }

    if check_only {
        if has_update {
            println!(
                "{} An update is available: {} -> {}\nRun {} to upgrade.",
                "⭐".yellow(),
                format!("v{}", current_version).red(),
                format!("v{}", latest_version).green().bold(),
                "djuke upgrade".cyan().bold()
            );
        } else {
            println!(
                "{} djuke is up to date (v{}).",
                "✔".green(),
                current_version
            );
        }
        return Ok(());
    }

    println!(
        "{} Upgrading djuke {} -> {}...",
        "⬆".cyan().bold(),
        format!("v{}", current_version).yellow(),
        format!("v{}", latest_version).green().bold()
    );

    let platform = Platform::current();
    let asset = find_matching_asset(&release.assets, &platform).ok_or_else(|| {
        DjukeError::Generic(format!(
            "No compatible prebuilt binary found for {}/{} in release {}.\nAssets available: {}",
            platform.os,
            platform.arch,
            release.tag_name,
            release
                .assets
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;

    if !quiet {
        println!(
            "  {} Found release asset: {} ({:.1} MB)",
            "•".cyan(),
            asset.name.bold(),
            asset.size as f64 / (1024.0 * 1024.0)
        );
    }

    let temp_dir = tempfile::tempdir()?;
    let download_target = temp_dir.path().join(&asset.name);

    let downloader = Downloader::new(quiet);
    downloader
        .download_file(
            &asset.browser_download_url,
            &download_target,
            None,
            "Downloading djuke update",
        )
        .await?;

    let new_binary_path = if asset.name.ends_with(".zip")
        || asset.name.ends_with(".tar.gz")
        || asset.name.ends_with(".tgz")
    {
        let extract_dir = temp_dir.path().join("extracted");
        fs::create_dir_all(&extract_dir)?;
        djuke_io::extract_archive(&download_target, &extract_dir)?;
        find_executable_in_dir(&extract_dir)?
    } else {
        download_target
    };

    let current_exe = env::current_exe().map_err(|e| {
        DjukeError::Generic(format!(
            "Failed to determine current executable path: {}",
            e
        ))
    })?;

    replace_executable(&new_binary_path, &current_exe)?;

    println!(
        "\n{} Successfully upgraded djuke to {}!",
        "✔".green().bold(),
        format!("v{}", latest_version).green().bold()
    );
    println!("  Updated: {}", current_exe.display().to_string().cyan());

    Ok(())
}

/// Replace current executable with new binary.
fn replace_executable(new_binary: &Path, current_exe: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(new_binary)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(new_binary, perms)?;

        // On Unix, atomic rename replaces the directory entry even if running
        if fs::rename(new_binary, current_exe).is_err() {
            // Fallback across filesystems
            let temp_dest = current_exe.with_extension("djuke_new");
            fs::copy(new_binary, &temp_dest)?;
            fs::rename(&temp_dest, current_exe)?;
        }
    }

    #[cfg(windows)]
    {
        // On Windows, running exe cannot be directly overwritten, but it CAN be renamed!
        let old_exe = current_exe.with_extension("exe.old");
        if old_exe.exists() {
            let _ = fs::remove_file(&old_exe);
        }

        fs::rename(current_exe, &old_exe).map_err(|e| {
            DjukeError::Generic(format!(
                "Failed to move current executable '{}' to backup: {}",
                current_exe.display(),
                e
            ))
        })?;

        if let Err(e) = fs::copy(new_binary, current_exe) {
            // Restore original on failure
            let _ = fs::rename(&old_exe, current_exe);
            return Err(DjukeError::Generic(format!(
                "Failed to copy new binary to '{}': {}",
                current_exe.display(),
                e
            )));
        }

        // Clean up .old backup if unlocked
        let _ = fs::remove_file(&old_exe);
    }

    Ok(())
}

/// Find executable in extracted folder.
fn find_executable_in_dir(dir: &Path) -> Result<PathBuf> {
    let target_name = if cfg!(windows) { "djuke.exe" } else { "djuke" };

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if name.eq_ignore_ascii_case(target_name) {
                        return Ok(p);
                    }
                }
            } else if p.is_dir() {
                if let Ok(found) = find_executable_in_dir(&p) {
                    return Ok(found);
                }
            }
        }
    }

    Err(DjukeError::Generic(format!(
        "Extracted update archive does not contain '{}'",
        target_name
    )))
}

/// Find matching release asset for platform.
pub fn find_matching_asset<'a>(
    assets: &'a [GithubAsset],
    platform: &Platform,
) -> Option<&'a GithubAsset> {
    // First, look for exact os and arch match
    for asset in assets {
        let name_lower = asset.name.to_ascii_lowercase();
        let os_match = match platform.os {
            Os::Windows => {
                name_lower.contains("windows")
                    || name_lower.contains("pc-windows")
                    || name_lower.ends_with(".exe")
            }
            Os::MacOs => {
                name_lower.contains("darwin")
                    || name_lower.contains("apple")
                    || name_lower.contains("macos")
            }
            Os::Linux => name_lower.contains("linux"),
        };

        let arch_match = match platform.arch {
            Arch::X64 => {
                name_lower.contains("x86_64")
                    || name_lower.contains("x64")
                    || name_lower.contains("amd64")
            }
            Arch::Aarch64 => name_lower.contains("aarch64") || name_lower.contains("arm64"),
        };

        if os_match && arch_match {
            return Some(asset);
        }
    }

    // Fallback: match OS only (e.g. single windows binary)
    for asset in assets {
        let name_lower = asset.name.to_ascii_lowercase();
        let os_match = match platform.os {
            Os::Windows => name_lower.contains("windows") || name_lower.ends_with(".exe"),
            Os::MacOs => name_lower.contains("darwin") || name_lower.contains("macos"),
            Os::Linux => name_lower.contains("linux"),
        };
        if os_match {
            return Some(asset);
        }
    }

    None
}

/// Compare two semver-like version strings. Returns true if candidate is newer than current.
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    let parse_parts = |s: &str| -> Vec<u64> {
        s.split(['.', '-', '+'])
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    };

    let cand_parts = parse_parts(candidate);
    let curr_parts = parse_parts(current);

    for (c, cur) in cand_parts.iter().zip(curr_parts.iter()) {
        if c > cur {
            return true;
        } else if c < cur {
            return false;
        }
    }

    cand_parts.len() > curr_parts.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(is_newer_version("0.2.0", "0.1.9"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.1"));
        assert!(!is_newer_version("0.0.9", "0.1.0"));
    }

    #[test]
    fn test_find_matching_asset() {
        let assets = vec![
            GithubAsset {
                name: "djuke-x86_64-pc-windows-msvc.zip".into(),
                browser_download_url: "https://example.com/windows.zip".into(),
                size: 15_000_000,
            },
            GithubAsset {
                name: "djuke-x86_64-unknown-linux-gnu.tar.gz".into(),
                browser_download_url: "https://example.com/linux.tar.gz".into(),
                size: 15_000_000,
            },
            GithubAsset {
                name: "djuke-aarch64-apple-darwin.tar.gz".into(),
                browser_download_url: "https://example.com/macos.tar.gz".into(),
                size: 15_000_000,
            },
        ];

        let win_platform = Platform {
            os: Os::Windows,
            arch: Arch::X64,
        };
        let matched = find_matching_asset(&assets, &win_platform);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().name, "djuke-x86_64-pc-windows-msvc.zip");
    }
}
