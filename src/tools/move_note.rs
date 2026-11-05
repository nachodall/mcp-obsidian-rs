use std::path::Path;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::fs_write::write_atomic;
use crate::relink::{all_files, relink_text};
use crate::server::Server;
use crate::vault::VaultError;

/// Arguments of the `move_note` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveNoteArgs {
    /// Current vault-relative path.
    pub from: String,
    /// New vault-relative path. Must not exist. Same folder = rename.
    pub to: String,
}

fn rel_string(root: &Path, abs: &Path) -> Result<String, String> {
    let rel = abs.strip_prefix(root).map_err(|e| e.to_string())?;
    Ok(rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"))
}

pub fn move_note(server: &Server, args: MoveNoteArgs) -> Result<String, String> {
    let vault = &server.vault;
    let src = vault.resolve(&args.from).map_err(|e| e.to_string())?;
    if !src.is_file() {
        return Err(VaultError::NotFound(args.from).to_string());
    }
    let dst_raw = vault.resolve_for_write(&args.to).map_err(|e| e.to_string())?;
    let from = rel_string(vault.root(), &src)?;
    let to = rel_string(vault.root(), &dst_raw)?;
    // `resolve_for_write` may leave a trailing separator on an existing path.
    let dst = vault.root().join(&to);
    if dst.symlink_metadata().is_ok() {
        return Err(format!("already exists: {}", args.to));
    }

    // Links are computed before anything moves.
    let before = all_files(vault);
    let after: Vec<String> =
        before.iter().filter(|f| **f != from).cloned().chain(std::iter::once(to.clone())).collect();
    let mut rewrites: Vec<(String, String, usize)> = Vec::new(); // (rel in final layout, new text, links)
    for file in before.iter().filter(|f| f.ends_with(".md")) {
        let Ok(text) = std::fs::read_to_string(vault.root().join(file)) else { continue };
        if let Some((new, n)) = relink_text(&text, &before, &after, &from, &to) {
            let final_rel = if *file == from { to.clone() } else { file.clone() };
            rewrites.push((final_rel, new, n));
        }
    }

    // Move without ever replacing: hard link, then drop the old name.
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("move failed: {e}"))?;
    }
    std::fs::hard_link(&src, &dst).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format!("already exists: {}", args.to)
        } else {
            format!("move failed: {e}")
        }
    })?;
    std::fs::remove_file(&src).map_err(|e| format!("move failed: {e}"))?;
    server.after_write(&from);
    server.after_write(&to);

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
    let mut out = format!("moved {from} -> {to}\nupdated {links} link(s)");
    if !visible.is_empty() {
        out.push_str(&format!("\nnotes updated:\n{}", visible.join("\n")));
    }
    if excluded > 0 {
        let s = if excluded == 1 { "" } else { "s" };
        out.push_str(&format!("\n{excluded} note{s} in excluded folders updated"));
    }
    Ok(out)
}

#[tool_router(router = move_router, vis = "pub(crate)")]
impl Server {
    /// Moves or renames a note (or attachment) and updates every link to it,
    /// keeping aliases, headings and embeds. Fails when the target exists.
    #[tool(name = "move_note")]
    async fn move_note_tool(&self, args: Parameters<MoveNoteArgs>) -> Result<String, String> {
        move_note(self, args.0)
    }
}
