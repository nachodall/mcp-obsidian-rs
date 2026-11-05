use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::server::Server;

/// Arguments of the `read_note` tool.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ReadNoteArgs {
    /// Vault-relative path of the note.
    pub path: String,
    /// First line to return, 1-based. Default 1.
    pub offset: Option<usize>,
    /// Maximum number of lines to return. Default: all.
    pub limit: Option<usize>,
}

#[tool_router(router = read_router, vis = "pub(crate)")]
impl Server {
    /// Reads a note, whole or by line range.
    #[tool(name = "read_note")]
    async fn read_note_tool(&self, args: Parameters<ReadNoteArgs>) -> Result<String, String> {
        let _ = args;
        Err("not implemented".to_string())
    }
}
