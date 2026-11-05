use std::path::{Path, PathBuf};

use crate::config::Config;

/// Single entry point that turns a tool argument into a filesystem path.
#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
    config: Config,
}

/// Errors for path resolution. `NotFound` is used for missing, excluded and
/// hidden paths alike, so the message gives no hint.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

impl Vault {
    pub fn new(_root: &Path, _config: Config) -> anyhow::Result<Vault> {
        todo!()
    }

    pub fn root(&self) -> &Path {
        todo!()
    }

    /// Resolves a vault-relative argument. Empty string means the vault root.
    pub fn resolve(&self, _relative: &str) -> Result<PathBuf, VaultError> {
        todo!()
    }

    /// True when a vault-relative path is excluded or hidden (`.obsidian/`, `.trash/`).
    pub fn is_hidden(&self, _relative: &Path) -> bool {
        todo!()
    }
}
