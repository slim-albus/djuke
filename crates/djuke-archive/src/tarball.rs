use flate2::read::GzDecoder;
use std::fs::File;
use std::path::Path;
use tar::Archive;

use djuke_core::{DjukeError, Result};

pub fn extract_tar_gz(archive_path: &Path, target_dir: &Path) -> Result<()> {
    let file = File::open(archive_path)?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    for entry_res in archive.entries()? {
        let mut entry = entry_res.map_err(|e| DjukeError::Archive(e.to_string()))?;
        let entry_path = entry.path()?.to_path_buf();

        // Path traversal check
        for component in entry_path.components() {
            if component == std::path::Component::ParentDir {
                return Err(DjukeError::Archive(format!(
                    "Path traversal detected in archive entry: {}",
                    entry_path.display()
                )));
            }
        }

        let dest = target_dir.join(&entry_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        entry.unpack(&dest)?;
    }

    Ok(())
}
