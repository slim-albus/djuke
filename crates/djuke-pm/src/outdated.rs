use colored::Colorize;
use djuke_core::Result;

use crate::central::MavenCentralClient;
use crate::dependency::DependencySpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateType {
    Major,
    Minor,
    Patch,
    UpToDate,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct OutdatedInfo {
    pub group_id: String,
    pub artifact_id: String,
    pub current: String,
    pub latest: String,
    pub update_type: UpdateType,
}

pub async fn check_outdated_dependencies(
    client: &MavenCentralClient,
    dependencies: &[DependencySpec],
) -> Result<Vec<OutdatedInfo>> {
    let mut results = Vec::new();

    for dep in dependencies {
        let current_ver = dep.version.as_deref().unwrap_or("unknown");
        let query = format!("g:\"{}\" AND a:\"{}\"", dep.group_id, dep.artifact_id);

        let search_res = client.search(&query, 1).await;
        let latest_ver = match search_res {
            Ok(artifacts) if !artifacts.is_empty() => artifacts[0]
                .latest_version
                .clone()
                .unwrap_or_else(|| "unknown".into()),
            _ => "unknown".to_string(),
        };

        let update_type = classify_update(current_ver, &latest_ver);

        results.push(OutdatedInfo {
            group_id: dep.group_id.clone(),
            artifact_id: dep.artifact_id.clone(),
            current: current_ver.to_string(),
            latest: latest_ver,
            update_type,
        });
    }

    Ok(results)
}

fn classify_update(current: &str, latest: &str) -> UpdateType {
    if current == "unknown" || latest == "unknown" {
        return UpdateType::Unknown;
    }
    if current == latest {
        return UpdateType::UpToDate;
    }

    let parse_nums = |s: &str| -> Vec<u64> {
        s.split(['.', '-', '+'])
            .filter_map(|part| part.parse::<u64>().ok())
            .collect()
    };

    let cur_nums = parse_nums(current);
    let lat_nums = parse_nums(latest);

    if !cur_nums.is_empty() && !lat_nums.is_empty() {
        if lat_nums[0] > cur_nums[0] {
            return UpdateType::Major;
        } else if cur_nums.len() > 1 && lat_nums.len() > 1 && lat_nums[1] > cur_nums[1] {
            return UpdateType::Minor;
        } else if cur_nums.len() > 2 && lat_nums.len() > 2 && lat_nums[2] > cur_nums[2] {
            return UpdateType::Patch;
        }
    }

    UpdateType::Minor
}

pub fn render_outdated_table(infos: &[OutdatedInfo]) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "{:<45} {:<15} {:<15} {}\n",
        "DEPENDENCY".bold(),
        "CURRENT".bold(),
        "LATEST".bold(),
        "STATUS".bold()
    ));
    out.push_str(&format!("{}\n", "-".repeat(85).dimmed()));

    for info in infos {
        let coord = format!("{}:{}", info.group_id, info.artifact_id);
        let status_str = match info.update_type {
            UpdateType::Major => "Major Update".red().bold(),
            UpdateType::Minor => "Minor Update".yellow(),
            UpdateType::Patch => "Patch Update".cyan(),
            UpdateType::UpToDate => "Up to date".green(),
            UpdateType::Unknown => "Unknown".dimmed(),
        };

        let current_colored = if info.update_type == UpdateType::UpToDate {
            info.current.green()
        } else {
            info.current.red()
        };

        out.push_str(&format!(
            "{:<45} {:<15} {:<15} {}\n",
            if coord.len() > 44 {
                &coord[..44]
            } else {
                &coord
            },
            current_colored,
            info.latest.green(),
            status_str
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_update() {
        assert_eq!(classify_update("1.0.0", "1.0.0"), UpdateType::UpToDate);
        assert_eq!(classify_update("1.0.0", "2.0.0"), UpdateType::Major);
        assert_eq!(classify_update("1.2.0", "1.3.0"), UpdateType::Minor);
        assert_eq!(classify_update("1.2.3", "1.2.4"), UpdateType::Patch);
    }
}
