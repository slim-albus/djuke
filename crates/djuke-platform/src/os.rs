use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }

    pub fn as_disco_name(&self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::MacOs => "macos",
            Self::Linux => "linux",
        }
    }

    pub fn default_archive_ext(&self) -> &'static str {
        match self {
            Self::Windows => "zip",
            Self::MacOs | Self::Linux => "tar.gz",
        }
    }
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Windows => write!(f, "windows"),
            Self::MacOs => write!(f, "macos"),
            Self::Linux => write!(f, "linux"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    X64,
    Aarch64,
}

impl Arch {
    pub fn current() -> Self {
        if cfg!(target_arch = "x86_64") {
            Self::X64
        } else if cfg!(target_arch = "aarch64") {
            Self::Aarch64
        } else {
            Self::X64
        }
    }

    pub fn as_disco_name(&self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::Aarch64 => "aarch64",
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X64 => write!(f, "x64"),
            Self::Aarch64 => write!(f, "aarch64"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    pub fn current() -> Self {
        Self {
            os: Os::current(),
            arch: Arch::current(),
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.os, self.arch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_current() {
        let p = Platform::current();
        assert!(!p.to_string().is_empty());
        assert!(!p.os.as_disco_name().is_empty());
        assert!(!p.arch.as_disco_name().is_empty());
    }

    #[test]
    fn test_os_archive_extension() {
        assert_eq!(Os::Windows.default_archive_ext(), "zip");
        assert_eq!(Os::Linux.default_archive_ext(), "tar.gz");
        assert_eq!(Os::MacOs.default_archive_ext(), "tar.gz");
    }
}
