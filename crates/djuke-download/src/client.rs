use bytes::Bytes;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt, BufWriter};

use djuke_core::{DjukeError, Result};

use crate::checksum::verify_checksum;

const MIN_PARALLEL_SIZE: u64 = 6 * 1024 * 1024; // 6 MB minimum for multi-part
const CHUNK_SIZE: u64 = 16 * 1024 * 1024; // 16 MB chunk target
const MAX_CONCURRENT_STREAMS: usize = 8;
const BUFFER_CAPACITY: usize = 2 * 1024 * 1024; // 2 MB I/O buffer

struct WriteMessage {
    offset: u64,
    data: Bytes,
}

pub struct Downloader {
    client: reqwest::Client,
    quiet: bool,
}

impl Downloader {
    pub fn new(quiet: bool) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .user_agent("djuke/0.1.0")
            .tcp_nodelay(true)
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .pool_max_idle_per_host(32)
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .redirect(reqwest::redirect::Policy::limited(10))
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

        // Fast probe: send Range: bytes=0-0 to check range support, size, and resolve redirects
        let probe = self.probe_url(url).await;

        if let Ok((Some(total_size), true, final_url)) = probe {
            if total_size >= MIN_PARALLEL_SIZE {
                // Multi-connection parallel chunk download
                match self
                    .download_parallel(&final_url, destination, total_size, title)
                    .await
                {
                    Ok(()) => {
                        return self.verify_and_finalize(destination, expected_sha256);
                    }
                    Err(e) => {
                        // In case of parallel failure, fall back to sequential stream
                        tracing::warn!(
                            "Parallel download failed ({}), falling back to sequential stream...",
                            e
                        );
                    }
                }
            }
        }

        // Sequential buffered download fallback
        let target_url = reqwest::Url::parse(url)
            .map_err(|e| DjukeError::Network(format!("Invalid download URL '{}': {}", url, e)))?;

        self.download_sequential(&target_url, destination, title)
            .await?;
        self.verify_and_finalize(destination, expected_sha256)
    }

    async fn probe_url(&self, url: &str) -> Result<(Option<u64>, bool, reqwest::Url)> {
        // Send a 1-byte range request to test ranges, read total size, and follow CDN redirects
        if let Ok(res) = self
            .client
            .get(url)
            .header("Range", "bytes=0-0")
            .send()
            .await
        {
            let final_url = res.url().clone();
            if res.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                if let Some(range_hdr) = res.headers().get(reqwest::header::CONTENT_RANGE) {
                    if let Ok(hdr_str) = range_hdr.to_str() {
                        if let Some((_, total_str)) = hdr_str.split_once('/') {
                            if let Ok(total) = total_str.trim().parse::<u64>() {
                                return Ok((Some(total), true, final_url));
                            }
                        }
                    }
                }
            } else if res.status().is_success() {
                let total = res.content_length();
                let accepts_ranges = res
                    .headers()
                    .get(reqwest::header::ACCEPT_RANGES)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.to_ascii_lowercase().contains("bytes"))
                    .unwrap_or(false);
                return Ok((total, accepts_ranges, final_url));
            }
        }

        // Fallback: standard HEAD request
        let head_res =
            self.client.head(url).send().await.map_err(|e| {
                DjukeError::Network(format!("Failed to connect to '{}': {}", url, e))
            })?;

        let final_url = head_res.url().clone();
        let total = head_res.content_length();
        let accepts_ranges = head_res
            .headers()
            .get(reqwest::header::ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_ascii_lowercase().contains("bytes"))
            .unwrap_or(false);

        Ok((total, accepts_ranges, final_url))
    }

    async fn download_parallel(
        &self,
        url: &reqwest::Url,
        destination: &Path,
        total_size: u64,
        title: &str,
    ) -> Result<()> {
        let num_chunks = std::cmp::max(
            4,
            std::cmp::min(
                MAX_CONCURRENT_STREAMS as u64,
                total_size.div_ceil(CHUNK_SIZE),
            ),
        );
        let chunk_size = total_size.div_ceil(num_chunks);

        let mut ranges = Vec::new();
        for i in 0..num_chunks {
            let start = i * chunk_size;
            let end = std::cmp::min(start + chunk_size - 1, total_size - 1);
            if start <= end {
                ranges.push((start, end));
            }
        }

        // Pre-allocate the destination file
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(destination)
            .await?;
        file.set_len(total_size).await?;
        drop(file);

        // Progress bar with speed monitoring
        let pb = if self.quiet {
            None
        } else {
            let p = ProgressBar::new(total_size);
            p.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({binary_bytes_per_sec}, eta {eta}) {msg}")
                    .unwrap_or_else(|_| ProgressStyle::default_bar())
                    .progress_chars("#>-"),
            );
            p.set_message(title.to_string());
            p.enable_steady_tick(Duration::from_millis(150));
            Some(Arc::new(p))
        };

        // Dedicated writer task to handle non-blocking disk writes without seek contention
        let (tx, mut rx) = tokio::sync::mpsc::channel::<WriteMessage>(128);
        let dest_path = destination.to_path_buf();

        let writer_task = tokio::spawn(async move {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .open(&dest_path)
                .await?;

            while let Some(msg) = rx.recv().await {
                file.seek(std::io::SeekFrom::Start(msg.offset)).await?;
                file.write_all(&msg.data).await?;
            }

            file.flush().await?;
            Ok::<(), std::io::Error>(())
        });

        // Spawn parallel chunk download workers
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_STREAMS));
        let mut handles = Vec::new();

        for (start, end) in ranges {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let client = self.client.clone();
            let worker_url = url.clone();
            let tx = tx.clone();
            let pb = pb.clone();

            let handle = tokio::spawn(async move {
                let _permit = permit;
                let mut retries = 3;

                loop {
                    let req = client
                        .get(worker_url.clone())
                        .header("Range", format!("bytes={}-{}", start, end));

                    match req.send().await {
                        Ok(resp) if resp.status().is_success() => {
                            let mut stream = resp.bytes_stream();
                            let mut offset = start;
                            let mut stream_ok = true;

                            while let Some(chunk_res) = stream.next().await {
                                match chunk_res {
                                    Ok(bytes) => {
                                        let len = bytes.len() as u64;
                                        if let Err(e) = tx
                                            .send(WriteMessage {
                                                offset,
                                                data: bytes,
                                            })
                                            .await
                                        {
                                            return Err(DjukeError::Process(format!(
                                                "Writer channel closed: {}",
                                                e
                                            )));
                                        }
                                        offset += len;
                                        if let Some(p) = &pb {
                                            p.inc(len);
                                        }
                                    }
                                    Err(_) => {
                                        stream_ok = false;
                                        break;
                                    }
                                }
                            }

                            if stream_ok && offset > end {
                                return Ok(());
                            }
                        }
                        _ => {}
                    }

                    retries -= 1;
                    if retries == 0 {
                        return Err(DjukeError::Network(format!(
                            "Failed to download byte range {}-{} after 3 attempts",
                            start, end
                        )));
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            });

            handles.push(handle);
        }

        // Drop our sender so writer channel closes once workers complete
        drop(tx);

        for h in handles {
            h.await
                .map_err(|e| DjukeError::Process(format!("Download worker panicked: {}", e)))??;
        }

        writer_task
            .await
            .map_err(|e| DjukeError::Process(format!("Download writer panicked: {}", e)))?
            .map_err(DjukeError::Io)?;

        if let Some(p) = pb {
            p.finish_with_message(format!("Downloaded {}", title));
        }

        Ok(())
    }

    async fn download_sequential(
        &self,
        url: &reqwest::Url,
        destination: &Path,
        title: &str,
    ) -> Result<()> {
        let response =
            self.client.get(url.clone()).send().await.map_err(|e| {
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
                            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({binary_bytes_per_sec}, eta {eta}) {msg}")
                            .unwrap_or_else(|_| ProgressStyle::default_bar())
                            .progress_chars("#>-"),
                    );
                    pb
                }
                None => {
                    let pb = ProgressBar::new_spinner();
                    pb.set_style(
                        ProgressStyle::default_spinner()
                            .template("{spinner:.green} {bytes} downloaded ({binary_bytes_per_sec}) {msg}")
                            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                    );
                    pb
                }
            };
            p.set_message(title.to_string());
            p.enable_steady_tick(Duration::from_millis(150));
            Some(p)
        };

        let file = File::create(destination).await?;
        let mut writer = BufWriter::with_capacity(BUFFER_CAPACITY, file);
        let mut stream = response.bytes_stream();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res
                .map_err(|e| DjukeError::Network(format!("Error while reading stream: {}", e)))?;
            writer.write_all(&chunk).await?;
            if let Some(p) = &pb {
                p.inc(chunk.len() as u64);
            }
        }

        writer.flush().await?;
        drop(writer);

        if let Some(p) = pb {
            p.finish_with_message(format!("Downloaded {}", title));
        }

        Ok(())
    }

    fn verify_and_finalize(&self, destination: &Path, expected_sha256: Option<&str>) -> Result<()> {
        if let Some(expected) = expected_sha256 {
            if let Err(e) = verify_checksum(destination, expected) {
                let _ = std::fs::remove_file(destination);
                return Err(e);
            }
        }
        Ok(())
    }
}
