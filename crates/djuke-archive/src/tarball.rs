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

        // Path traversal check: reject absolute paths and parent directory components
        if entry_path.is_absolute() {
            return Err(DjukeError::Archive(format!(
                "Absolute path detected in archive entry: {}",
                entry_path.display()
            )));
        }

        for component in entry_path.components() {
            match component {
                std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_) => {
                    return Err(DjukeError::Archive(format!(
                        "Path traversal detected in archive entry: {}",
                        entry_path.display()
                    )));
                }
                _ => {}
            }
        }

        let dest = target_dir.join(&entry_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let is_symlink = entry.header().entry_type().is_symlink();
        if is_symlink {
            #[cfg(unix)]
            {
                entry.unpack(&dest)?;
            }
            #[cfg(not(unix))]
            {
                // On Windows without SeCreateSymbolicLinkPrivilege, catch privilege errors gracefully
                if let Err(e) = entry.unpack(&dest) {
                    tracing::warn!(
                        "Skipping symlink in tarball on Windows: {} ({})",
                        dest.display(),
                        e
                    );
                }
            }
        } else {
            entry.unpack(&dest)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use tempfile::tempdir;

    #[test]
    fn test_extract_tar_gz_rejects_parent_dir_traversal() {
        let dir = tempdir().unwrap();
        let tar_path = dir.path().join("malicious.tar.gz");

        let file = File::create(&tar_path).unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = tar::Builder::new(encoder);

        let mut header = tar::Header::new_gnu();
        header.set_size(5);
        let name_bytes = b"../escape.txt\0";
        header.as_mut_bytes()[..name_bytes.len()].copy_from_slice(name_bytes);
        header.set_cksum();
        builder.append(&header, b"hello".as_slice()).unwrap();
        builder.into_inner().unwrap().finish().unwrap();

        let target_dir = dir.path().join("extracted");
        let result = extract_tar_gz(&tar_path, &target_dir);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Path traversal detected"));
    }
}
