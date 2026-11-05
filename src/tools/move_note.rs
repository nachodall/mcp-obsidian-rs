use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::server::Server;

/// Arguments of the `move_note` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveNoteArgs {
    /// Current vault-relative path.
    pub from: String,
    /// New vault-relative path. Must not exist. Same folder = rename.
    pub to: String,
}

#[tool_router(router = move_router, vis = "pub(crate)")]
impl Server {
    /// Moves or renames a note (or attachment) and updates every link to it.
    #[tool(name = "move_note")]
    async fn move_note_tool(&self, args: Parameters<MoveNoteArgs>) -> Result<String, String> {
        let _ = args;
        Err("not implemented".to_string())
    }
}
