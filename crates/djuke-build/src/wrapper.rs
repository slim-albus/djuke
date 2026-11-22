use std::path::{Path, PathBuf};

use djuke_core::BuildTool;

#[derive(Debug, Clone)]
pub struct ResolvedBuildExecutable {
    pub tool: BuildTool,
    pub executable: PathBuf,
    pub is_wrapper: bool,
}

pub fn find_build_executable(
    project_root: &Path,
    tool: BuildTool,
) -> Option<ResolvedBuildExecutable> {
    match tool {
        BuildTool::Gradle => {
            // Check wrapper first
            let gradlew_bat = project_root.join("gradlew.bat");
            let gradlew = project_root.join("gradlew");

            if cfg!(windows) && gradlew_bat.is_file() {
                return Some(ResolvedBuildExecutable {
                    tool: BuildTool::Gradle,
                    executable: gradlew_bat,
                    is_wrapper: true,
                });
            } else if gradlew.is_file() {
                return Some(ResolvedBuildExecutable {
                    tool: BuildTool::Gradle,
                    executable: gradlew,
                    is_wrapper: true,
                });
            }

            // Fallback to system gradle
            if let Ok(sys_gradle) = which::which("gradle") {
                return Some(ResolvedBuildExecutable {
                    tool: BuildTool::Gradle,
                    executable: sys_gradle,
                    is_wrapper: false,
                });
            }
        }
        BuildTool::Maven => {
            // Check wrapper first
            let mvnw_cmd = project_root.join("mvnw.cmd");
            let mvnw = project_root.join("mvnw");

            if cfg!(windows) && mvnw_cmd.is_file() {
                return Some(ResolvedBuildExecutable {
                    tool: BuildTool::Maven,
                    executable: mvnw_cmd,
                    is_wrapper: true,
                });
            } else if mvnw.is_file() {
                return Some(ResolvedBuildExecutable {
                    tool: BuildTool::Maven,
                    executable: mvnw,
                    is_wrapper: true,
                });
            }

            // Fallback to system mvn
            if let Ok(sys_mvn) = which::which("mvn") {
                return Some(ResolvedBuildExecutable {
                    tool: BuildTool::Maven,
                    executable: sys_mvn,
                    is_wrapper: false,
                });
            }
        }
    }

    None
}
