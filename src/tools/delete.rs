use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::server::Server;
use crate::vault::VaultError;

/// Arguments of the `delete_note` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteNoteArgs {
    /// Vault-relative path of the note to move to `.trash/`.
    pub path: String,
}

pub fn delete_note(server: &Server, args: DeleteNoteArgs) -> Result<String, String> {
    let source = server.vault.resolve(&args.path).map_err(|e| e.to_string())?;
    if !source.is_file() {
        return Err(VaultError::NotFound(args.path).to_string());
    }
    let is_note = source
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if !is_note {
        return Err(format!("only notes can be deleted: {}", args.path));
    }
    let name = source.file_name().ok_or("invalid path")?.to_string_lossy().to_string();
    let trash = server.vault.root().join(".trash");
    std::fs::create_dir_all(&trash).map_err(io_err)?;
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.clone(), String::new()),
    };
    let mut n = 0u32;
    loop {
        let candidate =
            if n == 0 { name.clone() } else { format!("{stem} {n}{ext}") };
        let dest = trash.join(&candidate);
        // A hard link fails when the destination exists, so nothing is overwritten.
        match std::fs::hard_link(&source, &dest) {
            Ok(()) => {
                std::fs::remove_file(&source).map_err(io_err)?;
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(io_err(e)),
        }
    }
    server.after_write(&args.path);
    Ok(format!("deleted {} (moved to .trash/)", args.path))
}

fn io_err(e: std::io::Error) -> String {
    format!("delete failed: {e}")
}

#[tool_router(router = delete_router, vis = "pub(crate)")]
impl Server {
    /// Moves a note to the vault's `.trash/`. Nothing is removed from disk.
    #[tool(name = "delete_note")]
    async fn delete_note_tool(&self, args: Parameters<DeleteNoteArgs>) -> Result<String, String> {
        delete_note(self, args.0)
    }
}
