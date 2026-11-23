use crate::bootstrap::{create_gradle_wrapper, create_maven_wrapper};
use crate::wrapper::find_build_executable;
use djuke_core::{BuildDriver, BuildTool, Result};
use std::path::{Path, PathBuf};

pub struct GradleDriver;

impl BuildDriver for GradleDriver {
    fn tool(&self) -> BuildTool {
        BuildTool::Gradle
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("build.gradle").is_file()
            || project_root.join("build.gradle.kts").is_file()
            || project_root.join("settings.gradle").is_file()
            || project_root.join("settings.gradle.kts").is_file()
            || project_root.join("gradlew").is_file()
            || project_root.join("gradlew.bat").is_file()
    }

    fn resolve_executable(&self, project_root: &Path) -> Option<PathBuf> {
        find_build_executable(project_root, BuildTool::Gradle).map(|r| r.executable)
    }

    fn create_wrapper(&self, project_root: &Path, version: Option<&str>) -> Result<()> {
        create_gradle_wrapper(project_root, version)
    }
}

pub struct MavenDriver;

impl BuildDriver for MavenDriver {
    fn tool(&self) -> BuildTool {
        BuildTool::Maven
    }

    fn detect(&self, project_root: &Path) -> bool {
        project_root.join("pom.xml").is_file()
            || project_root.join("mvnw").is_file()
            || project_root.join("mvnw.cmd").is_file()
    }

    fn resolve_executable(&self, project_root: &Path) -> Option<PathBuf> {
        find_build_executable(project_root, BuildTool::Maven).map(|r| r.executable)
    }

    fn create_wrapper(&self, project_root: &Path, version: Option<&str>) -> Result<()> {
        create_maven_wrapper(project_root, version)
    }
}
