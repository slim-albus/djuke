use std::path::Path;
use miette::Result;
use djuke_doctor::run_doctor_checks;
use djuke_core::DjukePaths;

pub async fn handle_doctor(current_dir: &Path, paths: &DjukePaths, json: bool) -> Result<()> {
    let report = run_doctor_checks(current_dir, paths).await?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| djuke_core::DjukeError::Serialization(e.to_string()))?
        );
    } else {
        report.print_human();
    }
    Ok(())
}
