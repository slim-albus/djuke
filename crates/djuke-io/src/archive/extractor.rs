use std::fs;
use std::path::Path;

use djuke_core::{DjukeError, Result};

use super::tarball::extract_tar_gz;
use super::zip_archive::extract_zip;

pub fn extract_archive(archive_path: &Path, destination_dir: &Path) -> Result<()> {
    let parent = destination_dir
        .parent()
        .ok_or_else(|| DjukeError::Archive("Destination has no parent directory".into()))?;
    fs::create_dir_all(parent)?;

    // Staging temp directory in the same parent for atomic rename
    let staging_dir = tempfile::Builder::new()
        .prefix(".djuke-staging-")
        .tempdir_in(parent)
        .map_err(|e| DjukeError::Archive(format!("Failed to create staging directory: {}", e)))?;

    let path_str = archive_path.to_string_lossy().to_lowercase();
    if path_str.ends_with(".tar.gz") || path_str.ends_with(".tgz") {
        extract_tar_gz(archive_path, staging_dir.path())?;
    } else if path_str.ends_with(".zip") {
        extract_zip(archive_path, staging_dir.path())?;
    } else {
        return Err(DjukeError::Archive(format!(
            "Unsupported archive format: {}",
            archive_path.display()
        )));
    }

    // JDK archives usually contain an enclosing root folder (e.g. `jdk-21.0.4+7`).
    // If staging_dir contains exactly one directory and no files, flatten it.
    let entries: Vec<_> = fs::read_dir(staging_dir.path())?
        .filter_map(|e| e.ok())
        .collect();

    let final_content_dir = if entries.len() == 1 && entries[0].path().is_dir() {
        entries[0].path()
    } else {
        staging_dir.path().to_path_buf()
    };

    // If destination already exists, remove it first with forced attribute cleanup
    if destination_dir.exists() {
        remove_dir_all_force(destination_dir)?;
    }

    // Atomic move / rename
    if fs::rename(&final_content_dir, destination_dir).is_err() {
        // Fallback for cross-device or permission limits
        copy_dir_all(&final_content_dir, destination_dir)?;
    }

    Ok(())
}

pub fn remove_dir_all_force(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    #[cfg(windows)]
    {
        // Recursively clear read-only attributes so Windows doesn't fail with AccessDenied
        #[allow(clippy::permissions_set_readonly_false)]
        fn clear_readonly(p: &Path) {
            if let Ok(entries) = fs::read_dir(p) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_dir() {
                        clear_readonly(&entry_path);
                    } else if let Ok(metadata) = entry.metadata() {
                        let mut perms = metadata.permissions();
                        if perms.readonly() {
                            perms.set_readonly(false);
                            let _ = fs::set_permissions(&entry_path, perms);
                        }
                    }
                }
            }
        }
        clear_readonly(path);
    }

    fs::remove_dir_all(path)?;
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}
