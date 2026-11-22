use miette::Diagnostic;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, DjukeError>;

#[derive(Error, Diagnostic, Debug)]
pub enum DjukeError {
    #[error("I/O error: {0}")]
    #[diagnostic(code(djuke::io_error))]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    #[diagnostic(code(djuke::serialization))]
    Serialization(String),

    #[error("HTTP network error: {0}")]
    #[diagnostic(code(djuke::network))]
    Network(String),

    #[error("Checksum verification failed: expected {expected}, calculated {actual}")]
    #[diagnostic(
        code(djuke::checksum_mismatch),
        help("The downloaded archive might be corrupted or tampered with. Please retry.")
    )]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Archive extraction error: {0}")]
    #[diagnostic(code(djuke::archive_error))]
    Archive(String),

    #[error("Invalid JDK specification '{0}': expected format '<major>' or '<distro>@<major>'")]
    #[diagnostic(
        code(djuke::invalid_jdk_spec),
        help("Try using formats like '21', 'temurin@21', or 'corretto@17'")
    )]
    InvalidJdkSpec(String),

    #[error(
        "JDK not found in catalog for version '{version}' and distro '{distro}' on {os}/{arch}"
    )]
    #[diagnostic(code(djuke::jdk_not_found))]
    JdkNotFound {
        version: String,
        distro: String,
        os: String,
        arch: String,
    },

    #[error("No JDK installed matching '{0}'")]
    #[diagnostic(
        code(djuke::jdk_not_installed),
        help("Run 'djuke java install {0}' to install this version.")
    )]
    JdkNotInstalled(String),

    #[error("No build tool detected in project '{0}'")]
    #[diagnostic(
        code(djuke::build_tool_not_found),
        help("Add a build.gradle or pom.xml to the project, or specify the build tool in djuke.toml.")
    )]
    NoBuildToolFound(String),

    #[error("Process execution failed: {0}")]
    #[diagnostic(code(djuke::process_error))]
    Process(String),

    #[error("Environment configuration error: {0}")]
    #[diagnostic(code(djuke::config_error))]
    Config(String),

    #[error("{0}")]
    #[diagnostic(code(djuke::generic))]
    Generic(String),
}
