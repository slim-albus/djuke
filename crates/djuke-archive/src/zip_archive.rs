use std::fs::File;
use std::io;
use std::path::Path;
use zip::ZipArchive;

use djuke_core::{DjukeError, Result};

pub fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<()> {
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file).map_err(|e| DjukeError::Archive(e.to_string()))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| DjukeError::Archive(e.to_string()))?;
        let enclosed_name = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => {
                return Err(DjukeError::Archive(format!(
                    "Illegal entry path in zip: {}",
                    file.name()
                )))
            }
        };

        let dest = target_dir.join(enclosed_name);

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&dest)?;
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }

            #[cfg(unix)]
            let is_symlink = file
                .unix_mode()
                .map(|m| (m & 0o170000) == 0o120000)
                .unwrap_or(false);
            #[cfg(not(unix))]
            let is_symlink = false;

            if is_symlink {
                #[cfg(unix)]
                {
                    use std::io::Read;
                    let mut link_target = String::new();
                    file.read_to_string(&mut link_target)?;
                    if dest.exists() || dest.is_symlink() {
                        let _ = std::fs::remove_file(&dest);
                    }
                    std::os::unix::fs::symlink(&link_target, &dest)?;
                }
            } else {
                let mut outfile = File::create(&dest)?;
                io::copy(&mut file, &mut outfile)?;
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                if (mode & 0o170000) != 0o120000 {
                    let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(mode));
                }
            }
        }
    }

    Ok(())
}
