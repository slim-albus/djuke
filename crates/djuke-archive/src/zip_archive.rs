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
            let mut outfile = File::create(&dest)?;
            io::copy(&mut file, &mut outfile)?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(mode));
            }
        }
    }

    Ok(())
}
