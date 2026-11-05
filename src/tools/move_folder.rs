use std::path::Path;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::fs_write::write_atomic;
use crate::relink::{all_files, relink_text_many};
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

fn rel_string(root: &Path, abs: &Path) -> Result<String, String> {
    let rel = abs.strip_prefix(root).map_err(|e| e.to_string())?;
    Ok(rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"))
}

pub fn move_folder(server: &Server, args: MoveFolderArgs) -> Result<String, String> {
    let vault = &server.vault;
    let src = vault.resolve(&args.from).map_err(|e| e.to_string())?;
    let from = rel_string(vault.root(), &src)?;
    if from.is_empty() || !src.is_dir() {
        return Err(VaultError::NotFound(args.from).to_string());
    }
    let dst_raw = vault.resolve_for_write(&args.to).map_err(|e| e.to_string())?;
    let to = rel_string(vault.root(), &dst_raw)?;
    // `resolve_for_write` may leave a trailing separator on an existing path.
    let dst = vault.root().join(&to);
    if dst.symlink_metadata().is_ok() {
        return Err(format!("already exists: {}", args.to));
    }
    if to.starts_with(&format!("{from}/")) {
        return Err(format!("cannot move a folder into itself: {}", args.from));
    }

    // Links are computed before anything moves.
    let prefix = format!("{from}/");
    let before = all_files(vault);
    let moves: Vec<(String, String)> = before
        .iter()
        .filter_map(|f| f.strip_prefix(&prefix).map(|rest| (f.clone(), format!("{to}/{rest}"))))
        .collect();
    let after: Vec<String> = before
        .iter()
        .map(|f| moves.iter().find(|(o, _)| o == f).map_or_else(|| f.clone(), |(_, n)| n.clone()))
        .collect();
    let mut rewrites: Vec<(String, String, usize)> = Vec::new(); // (rel in final layout, new text, links)
    for (file, final_rel) in before.iter().zip(after.iter()).filter(|(f, _)| f.ends_with(".md")) {
        let Ok(text) = std::fs::read_to_string(vault.root().join(file)) else { continue };
        if let Some((new, n)) = relink_text_many(&text, &before, &after, &moves) {
            rewrites.push((final_rel.clone(), new, n));
        }
    }

    // One directory rename; the destination was checked not to exist.
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("move failed: {e}"))?;
    }
    std::fs::rename(&src, &dst).map_err(|e| format!("move failed: {e}"))?;
    for (old, new) in &moves {
        server.after_write(old);
        server.after_write(new);
    }

    let mut visible = Vec::new();
    let mut excluded = 0usize;
    let mut links = 0usize;
    for (rel, new, n) in rewrites {
        write_atomic(&vault.root().join(&rel), new.as_bytes()).map_err(|e| format!("write failed: {e}"))?;
        links += n;
        if vault.is_hidden(Path::new(&rel)) {
            excluded += 1;
        } else {
            server.after_write(&rel);
            visible.push(rel);
        }
    }
    let mut out = format!("moved {from} -> {to}\n{} file(s) moved\nupdated {links} link(s)", moves.len());
    if !visible.is_empty() {
        out.push_str(&format!("\nnotes updated:\n{}", visible.join("\n")));
    }
    if excluded > 0 {
        let s = if excluded == 1 { "" } else { "s" };
        out.push_str(&format!("\n{excluded} note{s} in excluded folders updated"));
    }
    Ok(out)
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
