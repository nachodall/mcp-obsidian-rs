use std::path::{Path, PathBuf};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::error::ToolError;
use crate::relink::{Rewrite, apply_rewrites, plan_rewrites};
use crate::server::Server;
use crate::vault::VaultError;

/// Arguments of the `move_folder` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveFolderArgs {
    /// Current vault-relative folder path.
    pub from: String,
    /// New vault-relative folder path. Must not exist.
    pub to: String,
}

/// A checked move: both paths resolved, the target free.
struct Checked {
    /// Vault-relative source and target (`/` separators).
    from: String,
    to: String,
    src: PathBuf,
    dst: PathBuf,
}

fn validate(server: &Server, args: &MoveFolderArgs) -> Result<Checked, ToolError> {
    let vault = &server.vault;
    let not_found = || VaultError::NotFound(args.from.clone());
    let src = vault.resolve(&args.from)?;
    let from = vault.rel_of(&src).ok_or_else(not_found)?;
    if from.is_empty() || !src.is_dir() {
        return Err(not_found().into());
    }
    let dst = vault.resolve_for_write(&args.to)?;
    let to = vault.rel_of(&dst).ok_or_else(|| VaultError::NotFound(args.to.clone()))?;
    if dst.symlink_metadata().is_ok() {
        return Err(ToolError::AlreadyExists(args.to.clone()));
    }
    if Path::new(&to).starts_with(&from) {
        return Err(format!("cannot move a folder into itself: {}", args.from).into());
    }
    Ok(Checked { from, to, src, dst })
}

/// Every file under the folder with its new path. Computed before anything moves.
fn plan(server: &Server, checked: &Checked) -> (Vec<(String, String)>, Vec<Rewrite>) {
    let before = server.vault.files(true);
    let prefix = format!("{}/", checked.from);
    let moves: Vec<(String, String)> = before
        .iter()
        .filter_map(|f| f.strip_prefix(&prefix).map(|rest| (f.clone(), format!("{}/{rest}", checked.to))))
        .collect();
    let rewrites = plan_rewrites(&server.vault, &before, &moves);
    (moves, rewrites)
}

fn execute(
    server: &Server,
    checked: Checked,
    moves: Vec<(String, String)>,
    rewrites: Vec<Rewrite>,
) -> Result<String, ToolError> {
    let Checked { from, to, src, dst } = checked;
    // One directory rename; the destination was checked not to exist.
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(ToolError::io("move"))?;
    }
    std::fs::rename(&src, &dst).map_err(ToolError::io("move"))?;
    for (old, new) in &moves {
        server.after_write(old);
        server.after_write(new);
    }
    let summary = apply_rewrites(server, rewrites)?;
    Ok(format!("moved {from} -> {to}\n{} file(s) moved\n{}", moves.len(), summary.render()))
}

pub fn move_folder(server: &Server, args: MoveFolderArgs) -> Result<String, ToolError> {
    let checked = validate(server, &args)?;
    let (moves, rewrites) = plan(server, &checked);
    execute(server, checked, moves, rewrites)
}

#[tool_router(router = move_folder_router, vis = "pub(crate)")]
impl Server {
    /// Moves a folder with every note and attachment inside and updates every
    /// link and embed to them. Fails when the target exists.
    #[tool(name = "move_folder")]
    async fn move_folder_tool(&self, args: Parameters<MoveFolderArgs>) -> Result<String, String> {
        move_folder(self, args.0).map_err(String::from)
    }
}
