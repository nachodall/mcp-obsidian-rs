use serde::Deserialize;

/// Arguments of the `list_notes` tool.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ListNotesArgs {
    /// Optional vault-relative folder used as scope.
    pub folder: Option<String>,
}

/// Lists notes, folders and attachments (marked as attachments).
pub fn list_notes(_vault: &crate::vault::Vault, _args: ListNotesArgs) -> Result<String, crate::vault::VaultError> {
    todo!()
}
