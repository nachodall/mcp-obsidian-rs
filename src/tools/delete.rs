use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::error::ToolError;
use crate::fs_write::move_no_replace;
use crate::server::Server;

/// Arguments of the `delete_note` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteNoteArgs {
    /// Vault-relative path of the note to move to `.trash/`.
    pub path: String,
}

/// Candidate file name in `.trash/`: the name itself, then `stem 1.ext`, `stem 2.ext`, ...
fn unique_trash_name(name: &str, n: u32) -> String {
    if n == 0 {
        return name.to_string();
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem} {n}.{ext}"),
        None => format!("{name} {n}"),
    }
}

pub fn delete_note(server: &Server, args: DeleteNoteArgs) -> Result<String, ToolError> {
    let source = server.vault.resolve_file(&args.path)?;
    let is_note = source
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if !is_note {
        return Err(format!("only notes can be deleted: {}", args.path).into());
    }
    let name = source.file_name().ok_or("invalid path")?.to_string_lossy().to_string();
    let trash = server.vault.root().join(".trash");
    std::fs::create_dir_all(&trash).map_err(ToolError::io("delete"))?;
    let mut n = 0u32;
    loop {
        let dest = trash.join(unique_trash_name(&name, n));
        // Never replaces: the link fails when the destination exists.
        match move_no_replace(&source, &dest) {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(ToolError::io("delete")(e)),
        }
    }
    server.after_write(&args.path);
    Ok(format!("deleted {} (moved to .trash/)", args.path))
}

#[tool_router(router = delete_router, vis = "pub(crate)")]
impl Server {
    /// Moves a note to the vault's `.trash/`. Nothing is removed from disk.
    #[tool(name = "delete_note")]
    async fn delete_note_tool(&self, args: Parameters<DeleteNoteArgs>) -> Result<String, String> {
        delete_note(self, args.0).map_err(String::from)
    }
}
