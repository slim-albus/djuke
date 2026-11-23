use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

use djuke_core::{DjukeError, Result};

use super::checksum::verify_checksum;

const MIN_PARALLEL_SIZE: u64 = 4 * 1024 * 1024; // 4 MB minimum for multi-part
const CHUNK_SIZE: u64 = 12 * 1024 * 1024; // 12 MB chunk target
const MAX_CONCURRENT_STREAMS: usize = 12; // Up to 12 parallel streams
const BUFFER_CAPACITY: usize = 4 * 1024 * 1024; // 4 MB I/O buffer

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
            .pool_max_idle_per_host(64)
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
                        return self.verify_and_finalize(destination, expected_sha256).await;
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
        self.verify_and_finalize(destination, expected_sha256).await
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
        ) as usize;
        let chunk_size = total_size.div_ceil(num_chunks as u64);

        let mut ranges = Vec::with_capacity(num_chunks);
        let mut part_paths: Vec<PathBuf> = Vec::with_capacity(num_chunks);

        for i in 0..num_chunks {
            let start = (i as u64) * chunk_size;
            let end = std::cmp::min(start + chunk_size - 1, total_size - 1);
            if start <= end {
                ranges.push((i, start, end));
                part_paths.push(PathBuf::from(format!(
                    "{}.part_{}.tmp",
                    destination.display(),
                    i
                )));
            }
        }

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
            p.enable_steady_tick(Duration::from_millis(100));
            Some(Arc::new(p))
        };

        // Spawn parallel chunk download workers writing directly to isolated sequential part files
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_STREAMS));
        let mut handles = Vec::new();

        for (idx, start, end) in ranges {
            let permit = semaphore.clone().acquire_owned().await.map_err(|_| {
                DjukeError::Generic("Download semaphore closed unexpectedly".into())
            })?;
            let client = self.client.clone();
            let worker_url = url.clone();
            let pb = pb.clone();
            let part_path = part_paths[idx].clone();

            let handle = tokio::spawn(async move {
                let _permit = permit;
                let mut retries = 3;

                loop {
                    let req = client
                        .get(worker_url.clone())
                        .header("Range", format!("bytes={}-{}", start, end));

                    match req.send().await {
                        Ok(resp) if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT => {
                            let file = File::create(&part_path).await?;
                            let mut writer = BufWriter::with_capacity(BUFFER_CAPACITY, file);
                            let mut stream = resp.bytes_stream();
                            let mut bytes_downloaded = 0u64;
                            let expected_bytes = end - start + 1;
                            let mut stream_ok = true;

                            while let Some(chunk_res) = stream.next().await {
                                match chunk_res {
                                    Ok(bytes) => {
                                        let len = bytes.len() as u64;
                                        if let Err(e) = writer.write_all(&bytes).await {
                                            return Err(DjukeError::Io(e));
                                        }
                                        bytes_downloaded += len;
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

                            if stream_ok {
                                if let Err(e) = writer.flush().await {
                                    return Err(DjukeError::Io(e));
                                }
                                drop(writer);

                                if bytes_downloaded == expected_bytes {
                                    return Ok(());
                                }
                            }
                        }
                        Ok(resp) if resp.status().is_success() => {
                            // Server returned 200 OK (full body) instead of 206 Partial Content.
                            // Terminate worker so caller falls back to sequential stream without duplicating data.
                            let _ = tokio::fs::remove_file(&part_path).await;
                            return Err(DjukeError::Network(format!(
                                "Server returned status {} instead of 206 Partial Content for byte range {}-{}",
                                resp.status(),
                                start,
                                end
                            )));
                        }
                        _ => {}
                    }

                    retries -= 1;
                    if retries == 0 {
                        let _ = tokio::fs::remove_file(&part_path).await;
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

        // Await all chunk downloaders
        for h in handles {
            match h.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    // Cleanup part files
                    for p in &part_paths {
                        let _ = tokio::fs::remove_file(p).await;
                    }
                    return Err(e);
                }
                Err(e) => {
                    for p in &part_paths {
                        let _ = tokio::fs::remove_file(p).await;
                    }
                    return Err(DjukeError::Process(format!(
                        "Download worker panicked: {}",
                        e
                    )));
                }
            }
        }

        // Fast sequential merge of part files on disk
        let dest_file = File::create(destination).await?;
        let mut dest_writer = BufWriter::with_capacity(BUFFER_CAPACITY, dest_file);

        for part in &part_paths {
            if part.exists() {
                let mut part_file = File::open(part).await?;
                tokio::io::copy(&mut part_file, &mut dest_writer).await?;
                drop(part_file);
                let _ = tokio::fs::remove_file(part).await;
            }
        }

        dest_writer.flush().await?;
        drop(dest_writer);

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
            p.enable_steady_tick(Duration::from_millis(100));
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

    async fn verify_and_finalize(
        &self,
        destination: &Path,
        expected_sha256: Option<&str>,
    ) -> Result<()> {
        if let Some(expected) = expected_sha256.filter(|c| !c.trim().is_empty()) {
            let dest = destination.to_path_buf();
            let exp = expected.to_string();
            let res = tokio::task::spawn_blocking(move || verify_checksum(&dest, &exp))
                .await
                .map_err(|e| DjukeError::Process(format!("Checksum task panicked: {}", e)))?;
            if let Err(e) = res {
                let _ = std::fs::remove_file(destination);
                return Err(e);
            }
        }
        Ok(())
    }
}
