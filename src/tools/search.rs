use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::server::Server;

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

#[tool_router(router = search_router, vis = "pub(crate)")]
impl Server {
    /// Full-text search: ranked notes with path and matching lines only.
    #[tool(name = "search_notes")]
    async fn search_notes_tool(&self, _args: Parameters<SearchNotesArgs>) -> Result<String, String> {
        Err("search_notes is not implemented".to_string())
    }
}
