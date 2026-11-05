use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::error::ToolError;
use crate::fs_write::move_no_replace;
use crate::relink::{Rewrite, apply_rewrites, plan_rewrites};
use crate::server::{Server, blocking};
use crate::vault::VaultError;

/// Arguments of the `move_note` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveNoteArgs {
    /// Current vault-relative path.
    pub from: String,
    /// New vault-relative path. Must not exist. Same folder = rename.
    pub to: String,
}

/// A checked move: both paths resolved, the target free.
struct Checked {
    /// Vault-relative source and target (`/` separators).
    from: String,
    to: String,
    src: std::path::PathBuf,
    dst: std::path::PathBuf,
}

fn validate(server: &Server, args: &MoveNoteArgs) -> Result<Checked, ToolError> {
    let vault = &server.vault;
    let src = vault.resolve_file(&args.from)?;
    let dst = vault.resolve_for_write(&args.to)?;
    let from = vault.rel_of(&src).ok_or_else(|| VaultError::NotFound(args.from.clone()))?;
    let to = vault.rel_of(&dst).ok_or_else(|| VaultError::NotFound(args.to.clone()))?;
    if dst.symlink_metadata().is_ok() {
        return Err(ToolError::AlreadyExists(args.to.clone()));
    }
    Ok(Checked { from, to, src, dst })
}

/// Links are computed before anything moves.
fn plan(server: &Server, checked: &Checked) -> Vec<Rewrite> {
    let before = server.vault.files(true);
    plan_rewrites(&server.vault, &before, &[(checked.from.clone(), checked.to.clone())])
}

fn execute(
    server: &Server,
    args: &MoveNoteArgs,
    checked: Checked,
    rewrites: Vec<Rewrite>,
) -> Result<String, ToolError> {
    let Checked { from, to, src, dst } = checked;
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(ToolError::io("move"))?;
    }
    move_no_replace(&src, &dst).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            ToolError::AlreadyExists(args.to.clone())
        } else {
            ToolError::io("move")(e)
        }
    })?;
    server.after_write_many(&[from.clone(), to.clone()]);
    let summary = apply_rewrites(server, rewrites)?;
    Ok(format!("moved {from} -> {to}\n{}", summary.render()))
}

pub fn move_note(server: &Server, args: MoveNoteArgs) -> Result<String, ToolError> {
    let checked = validate(server, &args)?;
    let rewrites = plan(server, &checked);
    execute(server, &args, checked, rewrites)
}

#[tool_router(router = move_router, vis = "pub(crate)")]
impl Server {
    /// Moves or renames a note (or attachment) and updates every link to it,
    /// keeping aliases, headings and embeds. Fails when the target exists.
    #[tool(name = "move_note")]
    async fn move_note_tool(&self, args: Parameters<MoveNoteArgs>) -> Result<String, String> {
        blocking(self, move |s| move_note(s, args.0)).await
    }
}
