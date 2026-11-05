use std::path::Path;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::links::{parse_links, resolve_target};
use crate::output::truncate_with_notice;
use crate::server::Server;
use crate::vault::Vault;

/// Arguments of the `get_backlinks` tool.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct GetBacklinksArgs {
    /// Vault-relative path of the note (or attachment).
    pub path: String,
}

/// Vault-relative paths (`/` separators) of every visible file.
pub fn visible_files(vault: &Vault) -> Vec<String> {
    let mut out = Vec::new();
    collect(vault, vault.root(), &mut out);
    out.sort();
    out
}

fn collect(vault: &Vault, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(canonical) = path.canonicalize() else { continue };
        let Ok(canonical_rel) = canonical.strip_prefix(vault.root()) else { continue };
        let Ok(rel) = path.strip_prefix(vault.root()) else { continue };
        if vault.is_hidden(rel) || vault.is_hidden(canonical_rel) {
            continue;
        }
        if canonical.is_dir() {
            if entry.file_type().is_ok_and(|t| !t.is_symlink()) {
                collect(vault, &path, out);
            }
        } else {
            let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            out.push(rel);
        }
    }
}

/// Notes that link to `rel` (vault-relative, `/` separators), sorted.
pub fn backlinks(vault: &Vault, rel: &str) -> Vec<String> {
    let files = visible_files(vault);
    let mut found = Vec::new();
    for file in files.iter().filter(|f| f.ends_with(".md") && f.as_str() != rel) {
        let Ok(text) = std::fs::read_to_string(vault.root().join(file)) else { continue };
        let hit = parse_links(&text)
            .iter()
            .any(|l| resolve_target(&l.target, &files).as_deref() == Some(rel));
        if hit {
            found.push(file.clone());
        }
    }
    found
}

#[tool_router(router = backlinks_router, vis = "pub(crate)")]
impl Server {
    /// Lists the notes that link to a note, in any wikilink or embed form.
    #[tool(name = "get_backlinks")]
    async fn get_backlinks_tool(&self, args: Parameters<GetBacklinksArgs>) -> Result<String, String> {
        let abs = self.vault.resolve(&args.0.path).map_err(|e| e.to_string())?;
        if !abs.is_file() {
            return Err(crate::vault::VaultError::NotFound(args.0.path.clone()).to_string());
        }
        let rel = abs
            .strip_prefix(self.vault.root())
            .map_err(|e| e.to_string())?
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let text = backlinks(&self.vault, &rel).join("\n");
        Ok(truncate_with_notice(&text, self.config.max_response_bytes))
    }
}
