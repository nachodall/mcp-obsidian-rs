use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::server::Server;

/// Arguments of the `get_backlinks` tool.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct GetBacklinksArgs {
    /// Vault-relative path of the note.
    pub path: String,
}

#[tool_router(router = backlinks_router, vis = "pub(crate)")]
impl Server {
    /// Lists the notes that link to a note.
    #[tool(name = "get_backlinks")]
    async fn get_backlinks_tool(&self, _args: Parameters<GetBacklinksArgs>) -> Result<String, String> {
        Err("not implemented".to_string())
    }
}
