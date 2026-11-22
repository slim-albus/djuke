pub mod detect;
pub mod global;
pub mod manifest;
pub mod resolver;

pub use detect::{detect_project_markers, ProjectMarkers};
pub use global::GlobalConfig;
pub use manifest::load_manifest;
pub use resolver::{
    resolve_project_config, resolve_project_config_with_tool, ResolvedProjectConfig,
};
