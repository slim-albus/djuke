pub mod generator;
pub mod initializr;
pub mod model;

pub use generator::generate_project;
pub use initializr::{InitializrDependency, SpringInitializrClient};
pub use model::{BuildToolChoice, NewProjectOptions, TemplateType, TestFramework};
