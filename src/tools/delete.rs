use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::server::Server;

/// Arguments of the `delete_note` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteNoteArgs {
    /// Vault-relative path of the note to move to `.trash/`.
    pub path: String,
}

pub fn delete_note(_server: &Server, _args: DeleteNoteArgs) -> Result<String, String> {
    Err("not implemented".to_string())
}

#[tool_router(router = delete_router, vis = "pub(crate)")]
impl Server {
    /// Moves a note to the vault's `.trash/`. Nothing is removed from disk.
    #[tool(name = "delete_note")]
    async fn delete_note_tool(&self, args: Parameters<DeleteNoteArgs>) -> Result<String, String> {
        delete_note(self, args.0)
    }
}
