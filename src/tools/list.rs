use std::path::Path;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::output::truncate_with_notice;
use crate::server::Server;
use crate::vault::{Vault, VaultError};

/// Arguments of the `list_notes` tool.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ListNotesArgs {
    /// Optional vault-relative folder used as scope.
    pub folder: Option<String>,
}

/// Lists notes, folders and attachments (marked as attachments).
pub fn list_notes(vault: &Vault, args: ListNotesArgs) -> Result<String, VaultError> {
    let dir = vault.resolve(args.folder.as_deref().unwrap_or(""))?;
    if !dir.is_dir() {
        return Err(VaultError::InvalidPath(
            args.folder.unwrap_or_default() + " is not a folder",
        ));
    }
    let mut out = String::new();
    walk(vault, &dir, &mut out);
    Ok(out)
}

fn walk(vault: &Vault, dir: &Path, out: &mut String) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        // Canonical target must be inside the vault and not hidden.
        let Ok(canonical) = path.canonicalize() else { continue };
        let Ok(canonical_rel) = canonical.strip_prefix(vault.root()) else { continue };
        let Ok(rel) = path.strip_prefix(vault.root()) else { continue };
        if vault.is_hidden(rel) || vault.is_hidden(canonical_rel) {
            continue;
        }
        let rel = rel.display();
        if canonical.is_dir() {
            out.push_str(&format!("{rel}/\n"));
            // Do not follow symlinked folders: avoids cycles.
            if entry.file_type().is_ok_and(|t| !t.is_symlink()) {
                walk(vault, &path, out);
            }
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push_str(&format!("{rel}\n"));
        } else {
            out.push_str(&format!("{rel} (attachment)\n"));
        }
    }
}

#[tool_router(router = list_router, vis = "pub(crate)")]
impl Server {
    /// Lists notes, folders and attachments in the vault, or in one folder.
    #[tool(name = "list_notes")]
    async fn list_notes_tool(&self, args: Parameters<ListNotesArgs>) -> Result<String, String> {
        list_notes(&self.vault, args.0)
            .map(|text| truncate_with_notice(&text, self.config.max_response_bytes))
            .map_err(|e| e.to_string())
    }
}
