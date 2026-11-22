pub mod detect;
pub mod global;
pub mod manifest;
pub mod resolver;

pub use detect::{detect_project_markers, ProjectMarkers};
pub use global::{load_global_config, save_global_config, GlobalConfig, GlobalJavaConfig};
pub use manifest::load_manifest;
pub use resolver::{
    resolve_project_config, resolve_project_config_with_tool, JdkResolutionSource,
    ResolvedProjectConfig,
};
