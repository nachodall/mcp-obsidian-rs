use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::fs_write::{create_atomic, write_atomic};
use crate::server::Server;
use crate::vault::VaultError;

/// Arguments of the `create_note` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateNoteArgs {
    /// Vault-relative path of the new note. Missing folders are created.
    pub path: String,
    /// Content of the note.
    pub content: String,
}

/// Arguments of the `edit_note` tool. Use exactly one mode: replace
/// (`old_text` + `new_text`), `append`, or `content` with `overwrite: true`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditNoteArgs {
    /// Vault-relative path of an existing note.
    pub path: String,
    /// Exact text to replace. Must appear exactly once in the note.
    pub old_text: Option<String>,
    /// Replacement for `old_text`.
    pub new_text: Option<String>,
    /// Text added at the end of the note.
    pub append: Option<String>,
    /// New full content. Only used together with `overwrite: true`.
    pub content: Option<String>,
    /// Explicit flag required to replace the whole note.
    pub overwrite: Option<bool>,
}

fn io_err(e: std::io::Error) -> String {
    format!("write failed: {e}")
}

impl Server {
    /// Hook for the search index after a successful write, move or delete.
    /// TODO(ticket 03): call `index.upsert(rel_path)` here once the index exists.
    pub(crate) fn after_write(&self, _rel_path: &str) {}
}

pub fn create_note(server: &Server, args: CreateNoteArgs) -> Result<String, String> {
    let target = server.vault.resolve_for_write(&args.path).map_err(|e| e.to_string())?;
    create_atomic(&target, args.content.as_bytes()).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format!("already exists: {}", args.path)
        } else {
            io_err(e)
        }
    })?;
    server.after_write(&args.path);
    Ok(format!("created {}", args.path))
}

pub fn edit_note(server: &Server, args: EditNoteArgs) -> Result<String, String> {
    let modes = [
        args.old_text.is_some() || args.new_text.is_some(),
        args.append.is_some(),
        args.content.is_some() || args.overwrite.is_some(),
    ];
    if modes.iter().filter(|m| **m).count() != 1 {
        return Err("use exactly one mode: old_text+new_text, append, or content+overwrite"
            .to_string());
    }
    let target = server.vault.resolve(&args.path).map_err(|e| e.to_string())?;
    if !target.is_file() {
        return Err(VaultError::NotFound(args.path).to_string());
    }
    let current = std::fs::read_to_string(&target).map_err(io_err)?;
    let updated = if let (Some(old), Some(new)) = (&args.old_text, &args.new_text) {
        if old.is_empty() {
            return Err("old_text must not be empty".to_string());
        }
        match current.matches(old.as_str()).count() {
            1 => current.replacen(old.as_str(), new, 1),
            0 => return Err("old_text not found in the note".to_string()),
            n => return Err(format!("old_text matches {n} places; it must match exactly once")),
        }
    } else if args.old_text.is_some() || args.new_text.is_some() {
        return Err("replace needs both old_text and new_text".to_string());
    } else if let Some(text) = &args.append {
        format!("{current}{text}")
    } else if args.overwrite == Some(true) {
        args.content.clone().ok_or("overwrite needs content")?
    } else {
        return Err("full overwrite needs content and overwrite: true".to_string());
    };
    write_atomic(&target, updated.as_bytes()).map_err(io_err)?;
    server.after_write(&args.path);
    Ok(format!("edited {}", args.path))
}

#[tool_router(router = write_router, vis = "pub(crate)")]
impl Server {
    /// Creates a new note. Fails when the path already exists.
    #[tool(name = "create_note")]
    async fn create_note_tool(&self, args: Parameters<CreateNoteArgs>) -> Result<String, String> {
        create_note(self, args.0)
    }

    /// Edits an existing note: exact replace (must match once), append, or
    /// full overwrite (only with `overwrite: true`).
    #[tool(name = "edit_note")]
    async fn edit_note_tool(&self, args: Parameters<EditNoteArgs>) -> Result<String, String> {
        edit_note(self, args.0)
    }
}
