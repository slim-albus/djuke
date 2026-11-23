use colored::Colorize;
use miette::Result;
use djuke_core::DjukePaths;
use djuke_io as djuke_cache;
use crate::cli::{CacheArgs, CacheSubcommands};

pub fn handle_cache(args: CacheArgs, paths: &DjukePaths, json: bool) -> Result<()> {
    match args.command {
        CacheSubcommands::Stats => {
            let stats = djuke_cache::get_cache_stats(paths)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&stats)
                        .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?
                );
            } else {
                println!("{}", "djuke Cache Statistics:".bold().cyan());
                println!("  • Cached Files: {}", stats.file_count);
                println!("  • Total Disk Usage: {}", stats.human_size.bold().green());
                println!("  • Cache Directory: {}", paths.cache_dir().display());
            }
        }
        CacheSubcommands::List => {
            let files = djuke_cache::list_cached_files(paths)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&files)
                        .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?
                );
            } else if files.is_empty() {
                println!("No cached archives in {}", paths.cache_dir().display());
            } else {
                println!("{}", "Cached Archive Downloads:".bold().cyan());
                for f in files {
                    println!("  • {:<50} ({})", f.name.green(), f.path.display());
                }
            }
        }
        CacheSubcommands::Clean => {
            let (count, _total_bytes) = djuke_cache::clean_cache(paths)?;
            println!("✔ Cleared {} files from cache.", count);
        }
    }
    Ok(())
}
