use std::path::Path;

/// Default response size limit: 50 KB.
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 50 * 1024;

/// Server configuration loaded from `.mcp-obsidian.toml` at the vault root.
/// TOML keys: `excluded_folders` (array of vault-relative folders) and
/// `max_response_bytes` (integer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub excluded_folders: Vec<String>,
    pub max_response_bytes: usize,
}

impl Config {
    /// Loads the config. Missing file gives defaults; an invalid file is an error
    /// whose message says why.
    pub fn load(_vault_root: &Path) -> anyhow::Result<Config> {
        todo!()
    }
}
