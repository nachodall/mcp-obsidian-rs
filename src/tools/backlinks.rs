use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::error::ToolError;
use crate::links::{FileIndex, parse_links};
use crate::server::{Server, blocking};
use crate::vault::Vault;

/// Arguments of the `get_backlinks` tool.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct GetBacklinksArgs {
    /// Vault-relative path of the note (or attachment).
    pub path: String,
}

/// Notes that link to `rel` (vault-relative, `/` separators), sorted.
pub fn backlinks(vault: &Vault, rel: &str) -> Vec<String> {
    let files = vault.files(false);
    let index = FileIndex::new(&files);
    let mut found = Vec::new();
    for file in files.iter().filter(|f| f.ends_with(".md") && f.as_str() != rel) {
        let Ok(text) = std::fs::read_to_string(vault.root().join(file)) else { continue };
        let hit = parse_links(&text)
            .iter()
            .any(|l| index.resolve(&l.target).as_deref() == Some(rel));
        if hit {
            found.push(file.clone());
        }
    }
    found
}

fn get_backlinks(server: &Server, path: &str) -> Result<String, ToolError> {
    let abs = server.vault.resolve_file(path)?;
    let rel = server.vault.rel_of(&abs).ok_or("invalid path")?;
    Ok(backlinks(&server.vault, &rel).join("\n"))
}

#[tool_router(router = backlinks_router, vis = "pub(crate)")]
impl Server {
    /// Lists the notes that link to a note, in any wikilink or embed form.
    #[tool(name = "get_backlinks")]
    async fn get_backlinks_tool(&self, args: Parameters<GetBacklinksArgs>) -> Result<String, String> {
        blocking(self, move |s| get_backlinks(s, &args.0.path)).await
    }
}
