use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::server::Server;

/// Arguments of the `move_folder` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveFolderArgs {
    /// Current vault-relative folder path.
    pub from: String,
    /// New vault-relative folder path. Must not exist.
    pub to: String,
}

pub fn move_folder(_server: &Server, _args: MoveFolderArgs) -> Result<String, String> {
    Err("not implemented".to_string())
}

#[tool_router(router = move_folder_router, vis = "pub(crate)")]
impl Server {
    /// Moves a folder with every note and attachment inside and updates every
    /// link and embed to them. Fails when the target exists.
    #[tool(name = "move_folder")]
    async fn move_folder_tool(&self, args: Parameters<MoveFolderArgs>) -> Result<String, String> {
        move_folder(self, args.0)
    }
}
