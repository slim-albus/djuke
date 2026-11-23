use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use djuke_core::{DjukePaths, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFile {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub file_count: usize,
    pub total_size_bytes: u64,
    pub human_size: String,
}

pub fn list_cached_files(paths: &DjukePaths) -> Result<Vec<CachedFile>> {
    let cache_dir = paths.cache_dir();
    if !cache_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(cache_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let metadata = entry.metadata()?;
            files.push(CachedFile {
                name: entry.file_name().to_string_lossy().to_string(),
                path,
                size_bytes: metadata.len(),
            });
        }
    }

    files.sort_by_key(|a| std::cmp::Reverse(a.size_bytes));
    Ok(files)
}

pub fn get_cache_stats(paths: &DjukePaths) -> Result<CacheStats> {
    let files = list_cached_files(paths)?;
    let total_size_bytes: u64 = files.iter().map(|f| f.size_bytes).sum();
    let human_size = format_size(total_size_bytes);

    Ok(CacheStats {
        file_count: files.len(),
        total_size_bytes,
        human_size,
    })
}

pub fn clean_cache(paths: &DjukePaths) -> Result<(usize, u64)> {
    let files = list_cached_files(paths)?;
    let count = files.len();
    let total_bytes: u64 = files.iter().map(|f| f.size_bytes).sum();

    for f in &files {
        let _ = fs::remove_file(&f.path);
    }

    // Also clean tmp directory
    let tmp_dir = paths.tmp_dir();
    if tmp_dir.is_dir() {
        let _ = fs::remove_dir_all(&tmp_dir);
        let _ = fs::create_dir_all(&tmp_dir);
    }

    Ok((count, total_bytes))
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
