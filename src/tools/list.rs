use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::error::ToolError;
use crate::server::{Server, blocking};
use crate::vault::Vault;

/// Arguments of the `list_notes` tool.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ListNotesArgs {
    /// Optional vault-relative folder used as scope.
    pub folder: Option<String>,
}

/// Lists notes, folders and attachments (marked as attachments).
pub fn list_notes(vault: &Vault, args: ListNotesArgs) -> Result<String, ToolError> {
    let dir = vault.resolve_dir(args.folder.as_deref().unwrap_or(""))?;
    let mut out = String::new();
    for entry in vault.walk(&dir, false) {
        let rel = &entry.rel;
        if entry.is_dir {
            out.push_str(&format!("{rel}/\n"));
        } else if entry.abs.extension().is_some_and(|e| e == "md") {
            out.push_str(&format!("{rel}\n"));
        } else {
            out.push_str(&format!("{rel} (attachment)\n"));
        }
    }
    Ok(out)
}

#[tool_router(router = list_router, vis = "pub(crate)")]
impl Server {
    /// Lists notes, folders and attachments in the vault, or in one folder.
    #[tool(name = "list_notes")]
    async fn list_notes_tool(&self, args: Parameters<ListNotesArgs>) -> Result<String, String> {
        blocking(self, move |s| list_notes(&s.vault, args.0)).await
    }
}
