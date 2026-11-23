use std::path::{Path, PathBuf};
use crate::error::Result;
use crate::model::{BuildTool, JdkSpec};

/// Trait abstraction for build tool execution drivers (Gradle, Maven, etc.)
pub trait BuildDriver: Send + Sync {
    /// Return the build tool variant this driver handles
    fn tool(&self) -> BuildTool;

    /// Detect whether the project at `project_root` uses this build tool
    fn detect(&self, project_root: &Path) -> bool;

    /// Resolve the executable path (wrapper or system binary)
    fn resolve_executable(&self, project_root: &Path) -> Option<PathBuf>;

    /// Create wrapper files in the project directory
    fn create_wrapper(&self, project_root: &Path, version: Option<&str>) -> Result<()>;
}

/// Information about a remote JDK package discovered in a catalog
#[derive(Debug, Clone)]
pub struct DiscoveredJdkPackage {
    pub download_url: String,
    pub filename: String,
    pub sha256_checksum: Option<String>,
    pub size_bytes: Option<u64>,
}

/// Trait abstraction for remote JDK catalog discovery providers (e.g. Foojay Disco, Adoptium, mirrors)
pub trait JdkCatalogProvider: Send + Sync {
    /// The display name of this catalog provider
    fn name(&self) -> &'static str;

    /// Locate a package matching the requested JDK specification
    fn find_package(
        &self,
        spec: &JdkSpec,
        os: &str,
        arch: &str,
    ) -> impl std::future::Future<Output = Result<DiscoveredJdkPackage>> + Send;
}
