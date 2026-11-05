use std::sync::Arc;

use crate::vault::Vault;

/// One search hit: vault-relative path and the matching lines only.
#[derive(Debug, Clone)]
pub struct Hit {
    pub path: String,
    pub lines: Vec<String>,
}

/// In-memory full-text index of the visible notes (never written to disk).
pub struct SearchIndex {}

impl SearchIndex {
    /// Builds the index from every visible note of the vault.
    pub fn build(_vault: Arc<Vault>) -> anyhow::Result<SearchIndex> {
        todo!()
    }

    /// Re-indexes one vault-relative note (adds or replaces it).
    pub fn upsert(&self, _path: &str) -> anyhow::Result<()> {
        todo!()
    }

    /// Removes one vault-relative note from the index.
    pub fn remove(&self, _path: &str) -> anyhow::Result<()> {
        todo!()
    }

    /// Ranked search. `folder` is a vault-relative scope (already validated).
    pub fn search(&self, _query: &str, _fuzzy: bool, _folder: Option<&str>) -> anyhow::Result<Vec<Hit>> {
        todo!()
    }
}
