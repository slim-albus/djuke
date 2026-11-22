use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use djuke_core::{DjukeError, Result};

pub struct Downloader {
    client: reqwest::Client,
    quiet: bool,
}

impl Downloader {
    pub fn new(quiet: bool) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .user_agent("djuke/0.1.0")
            .build()
            .unwrap_or_default();
        Self { client, quiet }
    }

    pub async fn download_file(
        &self,
        url: &str,
        destination: &Path,
        expected_sha256: Option<&str>,
        title: &str,
    ) -> Result<()> {
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let response =
            self.client.get(url).send().await.map_err(|e| {
                DjukeError::Network(format!("Failed to connect to '{}': {}", url, e))
            })?;

        if !response.status().is_success() {
            return Err(DjukeError::Network(format!(
                "HTTP request to '{}' returned status {}",
                url,
                response.status()
            )));
        }

        let total_size = response.content_length();
        let pb = if self.quiet {
            None
        } else {
            let p = match total_size {
                Some(len) => {
                    let pb = ProgressBar::new(len);
                    pb.set_style(
                        ProgressStyle::default_bar()
                            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
                            .unwrap_or_else(|_| ProgressStyle::default_bar())
                            .progress_chars("#>-"),
                    );
                    pb
                }
                None => {
                    let pb = ProgressBar::new_spinner();
                    pb.set_style(
                        ProgressStyle::default_spinner()
                            .template("{spinner:.green} {bytes} downloaded {msg}")
                            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                    );
                    pb
                }
            };
            p.set_message(title.to_string());
            Some(p)
        };

        let mut file = File::create(destination).await?;
        let mut stream = response.bytes_stream();
        let mut hasher = Sha256::new();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res
                .map_err(|e| DjukeError::Network(format!("Error while reading stream: {}", e)))?;
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
            if let Some(p) = &pb {
                p.inc(chunk.len() as u64);
            }
        }

        file.flush().await?;
        drop(file);

        if let Some(p) = pb {
            p.finish_with_message(format!("Downloaded {}", title));
        }

        // Verify SHA-256 if provided
        if let Some(expected) = expected_sha256 {
            let actual = hex::encode(hasher.finalize());
            let exp = expected.trim().to_ascii_lowercase();
            if actual != exp {
                // Remove corrupted file
                let _ = tokio::fs::remove_file(destination).await;
                return Err(DjukeError::ChecksumMismatch {
                    expected: exp,
                    actual,
                });
            }
        }

        Ok(())
    }
}
