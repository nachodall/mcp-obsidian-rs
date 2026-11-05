use std::path::{Component, Path, PathBuf};

use crate::config::Config;

/// Folders that are always hidden from every tool.
const HIDDEN_FOLDERS: [&str; 2] = [".obsidian", ".trash"];

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
    pub fn new(root: &Path, config: Config) -> anyhow::Result<Vault> {
        let root = root.canonicalize()?;
        Ok(Vault { root, config })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a vault-relative argument. Empty string means the vault root.
    /// The result is canonical and always inside the vault.
    pub fn resolve(&self, relative: &str) -> Result<PathBuf, VaultError> {
        let mut clean = PathBuf::new();
        for component in Path::new(relative).components() {
            match component {
                Component::Normal(part) => clean.push(part),
                Component::CurDir => {}
                _ => return Err(VaultError::InvalidPath(relative.to_string())),
            }
        }
        let not_found = || VaultError::NotFound(relative.to_string());
        if self.is_hidden(&clean) {
            return Err(not_found());
        }
        let canonical = self.root.join(&clean).canonicalize().map_err(|_| not_found())?;
        let inside = canonical.strip_prefix(&self.root).map_err(|_| not_found())?;
        // A symlink inside the vault may point at a hidden folder.
        if self.is_hidden(inside) {
            return Err(not_found());
        }
        Ok(canonical)
    }

    /// True when a vault-relative path is excluded or hidden (`.obsidian/`, `.trash/`).
    pub fn is_hidden(&self, relative: &Path) -> bool {
        let first = relative.components().next();
        if let Some(Component::Normal(name)) = first
            && HIDDEN_FOLDERS.iter().any(|h| name == *h)
        {
            return true;
        }
        self.config
            .excluded_folders
            .iter()
            .any(|f| relative.starts_with(f))
    }
}
