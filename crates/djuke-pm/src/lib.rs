pub mod central;
pub mod dependency;
pub mod gradle_editor;
pub mod pom_editor;

pub use central::{MavenArtifact, MavenCentralClient};
pub use dependency::{DependencyScope, DependencySpec};
pub use gradle_editor::{add_gradle_dependency, remove_gradle_dependency};
pub use pom_editor::{add_maven_dependency, remove_maven_dependency};
