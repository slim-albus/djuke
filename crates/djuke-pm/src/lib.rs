pub mod audit;
pub mod central;
pub mod dependency;
pub mod gradle_editor;
pub mod outdated;
pub mod parser;
pub mod pom_editor;
pub mod tree;

pub use audit::{render_audit_report, AuditClient, VulnerabilityReport};
pub use central::{MavenArtifact, MavenCentralClient};
pub use dependency::{DependencyScope, DependencySpec};
pub use gradle_editor::{add_gradle_dependency, remove_gradle_dependency};
pub use outdated::{check_outdated_dependencies, render_outdated_table, OutdatedInfo, UpdateType};
pub use parser::extract_project_dependencies;
pub use pom_editor::{add_maven_dependency, remove_maven_dependency};
pub use tree::render_dependency_tree;
