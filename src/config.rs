use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

const CONFIG_FILE: &str = ".mcp-obsidian.toml";

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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    excluded_folders: Vec<String>,
    max_response_bytes: Option<usize>,
}

impl Config {
    /// Loads the config. Missing file gives defaults; an invalid file is an error
    /// whose message says why.
    pub fn load(vault_root: &Path) -> anyhow::Result<Config> {
        let path = vault_root.join(CONFIG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Config {
                    excluded_folders: Vec::new(),
                    max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
                });
            }
            Err(e) => return Err(e).with_context(|| format!("cannot read {CONFIG_FILE}")),
        };
        let raw: RawConfig =
            toml::from_str(&text).with_context(|| format!("invalid {CONFIG_FILE}"))?;
        Ok(Config {
            excluded_folders: raw.excluded_folders,
            max_response_bytes: raw.max_response_bytes.unwrap_or(DEFAULT_MAX_RESPONSE_BYTES),
        })
    }
}
