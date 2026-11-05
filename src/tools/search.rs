use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::index::Hit;
use crate::output::truncate_with_notice;
use crate::server::Server;

/// Most matching lines shown per note.
const MAX_LINES_PER_NOTE: usize = 5;
/// Longest matching line shown, in characters.
const MAX_LINE_CHARS: usize = 300;

/// Arguments of the `search_notes` tool.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct SearchNotesArgs {
    /// Words to find. Use double quotes for an exact phrase.
    pub query: String,
    /// Tolerate small typos.
    pub fuzzy: Option<bool>,
    /// Optional vault-relative folder used as scope.
    pub folder: Option<String>,
}

fn format_hits(hits: &[Hit]) -> String {
    if hits.is_empty() {
        return "No results.".to_string();
    }
    let mut out = String::new();
    for hit in hits {
        out.push_str(&hit.path);
        out.push('\n');
        for (n, line) in hit.lines.iter().take(MAX_LINES_PER_NOTE) {
            let line: String = line.chars().take(MAX_LINE_CHARS).collect();
            out.push_str(&format!("  {n}: {line}\n"));
        }
    }
    out
}

#[tool_router(router = search_router, vis = "pub(crate)")]
impl Server {
    /// Full-text search: ranked notes with path and matching lines only.
    #[tool(name = "search_notes")]
    async fn search_notes_tool(&self, args: Parameters<SearchNotesArgs>) -> Result<String, String> {
        let args = args.0;
        // The scope goes through the resolver: hidden and missing look the same.
        let folder = match args.folder.as_deref() {
            Some(f) => {
                let dir = self.vault.resolve(f).map_err(|e| e.to_string())?;
                if !dir.is_dir() {
                    return Err(format!("invalid path: {f} is not a folder"));
                }
                let rel = dir.strip_prefix(self.vault.root()).map_err(|e| e.to_string())?;
                Some(rel.to_string_lossy().into_owned())
            }
            None => None,
        };
        let hits = self
            .index
            .search(&args.query, args.fuzzy.unwrap_or(false), folder.as_deref())
            .map_err(|e| format!("search failed: {e}"))?;
        Ok(truncate_with_notice(&format_hits(&hits), self.config.max_response_bytes))
    }
}
