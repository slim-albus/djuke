use std::fs;
use std::path::{Path, PathBuf};

use djuke_core::BuildTool;
use djuke_platform::DjukePaths;

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
    find_build_executable_with_paths(project_root, tool, None)
}

pub fn find_build_executable_with_paths(
    project_root: &Path,
    tool: BuildTool,
    paths: Option<&DjukePaths>,
) -> Option<ResolvedBuildExecutable> {
    match tool {
        BuildTool::Gradle => {
            // 1. Check project wrapper first
            let gradlew_bat = project_root.join("gradlew.bat");
            let gradlew = project_root.join("gradlew");

            if cfg!(windows) {
                if gradlew_bat.is_file() {
                    return Some(ResolvedBuildExecutable {
                        tool: BuildTool::Gradle,
                        executable: gradlew_bat,
                        is_wrapper: true,
                    });
                }
            } else if gradlew.is_file() {
                return Some(ResolvedBuildExecutable {
                    tool: BuildTool::Gradle,
                    executable: gradlew,
                    is_wrapper: true,
                });
            }

            // 2. Check ~/.djuke/tools/gradle/
            if let Some(djuke_paths) = paths {
                let gradle_tools = djuke_paths.tools_dir().join("gradle");
                if let Ok(entries) = fs::read_dir(gradle_tools) {
                    for entry in entries.flatten() {
                        let bin = if cfg!(windows) {
                            entry.path().join("bin").join("gradle.bat")
                        } else {
                            entry.path().join("bin").join("gradle")
                        };
                        if bin.is_file() {
                            return Some(ResolvedBuildExecutable {
                                tool: BuildTool::Gradle,
                                executable: bin,
                                is_wrapper: false,
                            });
                        }
                    }
                }
            }

            // 3. Fallback to system gradle on PATH
            if let Ok(sys_gradle) = which::which("gradle") {
                return Some(ResolvedBuildExecutable {
                    tool: BuildTool::Gradle,
                    executable: sys_gradle,
                    is_wrapper: false,
                });
            }
        }
        BuildTool::Maven => {
            // 1. Check project wrapper first
            let mvnw_cmd = project_root.join("mvnw.cmd");
            let mvnw = project_root.join("mvnw");

            if cfg!(windows) {
                if mvnw_cmd.is_file() {
                    return Some(ResolvedBuildExecutable {
                        tool: BuildTool::Maven,
                        executable: mvnw_cmd,
                        is_wrapper: true,
                    });
                }
            } else if mvnw.is_file() {
                return Some(ResolvedBuildExecutable {
                    tool: BuildTool::Maven,
                    executable: mvnw,
                    is_wrapper: true,
                });
            }

            // 2. Check ~/.djuke/tools/maven/
            if let Some(djuke_paths) = paths {
                let maven_tools = djuke_paths.tools_dir().join("maven");
                if let Ok(entries) = fs::read_dir(maven_tools) {
                    for entry in entries.flatten() {
                        let bin = if cfg!(windows) {
                            entry.path().join("bin").join("mvn.cmd")
                        } else {
                            entry.path().join("bin").join("mvn")
                        };
                        if bin.is_file() {
                            return Some(ResolvedBuildExecutable {
                                tool: BuildTool::Maven,
                                executable: bin,
                                is_wrapper: false,
                            });
                        }
                    }
                }
            }

            // 3. Fallback to system mvn on PATH
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
