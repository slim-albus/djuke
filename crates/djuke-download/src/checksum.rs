use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use djuke_core::{DjukeError, Result};

pub fn verify_checksum(path: &Path, expected_sha256: &str) -> Result<()> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let calculated = hex::encode(hasher.finalize());
    let expected = expected_sha256.trim().to_ascii_lowercase();

    if calculated != expected {
        return Err(DjukeError::ChecksumMismatch {
            expected,
            actual: calculated,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_verify_checksum_success() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "hello world").unwrap();

        // sha256 of "hello world" is b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_checksum(file.path(), expected).is_ok());
    }

    #[test]
    fn test_verify_checksum_mismatch() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "corrupted content").unwrap();

        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_checksum(file.path(), expected).is_err());
    }
}
