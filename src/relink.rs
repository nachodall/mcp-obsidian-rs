//! Link rewriting for moves and renames. Pure text functions plus the
//! plan/apply steps shared by `move_note` and `move_folder`. The plan reads
//! every note, excluded folders included (`Vault::files(true)`).

use crate::error::ToolError;
use crate::fs_write::write_atomic;
use crate::links::{FileIndex, parse_links};
use crate::server::Server;
use crate::vault::Vault;

/// New text of one note after a move.
#[derive(Debug)]
pub struct Rewrite {
    /// Vault-relative path of the note in the layout after the move.
    pub rel: String,
    pub text: String,
    /// Number of links changed.
    pub links: usize,
}

/// Plans the rewrites for `moves` (`(from, to)` pairs of vault-relative files)
/// against `before`, the file list before the move. Nothing is written.
pub fn plan_rewrites(vault: &Vault, before: &[String], moves: &[(String, String)]) -> Vec<Rewrite> {
    let after: Vec<String> = before
        .iter()
        .map(|f| moves.iter().find(|(o, _)| o == f).map_or_else(|| f.clone(), |(_, n)| n.clone()))
        .collect();
    let (index_before, index_after) = (FileIndex::new(before), FileIndex::new(&after));
    let mut rewrites = Vec::new();
    for (file, final_rel) in before.iter().zip(after.iter()).filter(|(f, _)| f.ends_with(".md")) {
        let Ok(text) = std::fs::read_to_string(vault.root().join(file)) else { continue };
        if let Some((new, links)) = relink_with(&text, &index_before, &index_after, moves) {
            rewrites.push(Rewrite { rel: final_rel.clone(), text: new, links });
        }
    }
    rewrites
}

/// What `apply_rewrites` did.
#[derive(Debug, Default)]
pub struct RewriteSummary {
    pub links: usize,
    /// Rewritten notes outside excluded folders.
    pub visible: Vec<String>,
    /// Number of rewritten notes inside excluded folders (paths are not shown).
    pub excluded: usize,
}

impl RewriteSummary {
    /// Report lines: link count, visible notes, excluded notes count.
    pub fn render(&self) -> String {
        let mut out = format!("updated {} link(s)", self.links);
        if !self.visible.is_empty() {
            out.push_str(&format!("\nnotes updated:\n{}", self.visible.join("\n")));
        }
        if self.excluded > 0 {
            let s = if self.excluded == 1 { "" } else { "s" };
            out.push_str(&format!("\n{} note{s} in excluded folders updated", self.excluded));
        }
        out
    }
}

/// Writes the rewrites and keeps the index in sync. Notes in excluded folders
/// are written but never indexed or listed.
pub fn apply_rewrites(server: &Server, rewrites: Vec<Rewrite>) -> Result<RewriteSummary, ToolError> {
    let vault = &server.vault;
    let mut summary = RewriteSummary::default();
    let mut indexed = Vec::new();
    let mut failure = None;
    for Rewrite { rel, text, links } in rewrites {
        if let Err(e) = write_atomic(&vault.root().join(&rel), text.as_bytes()) {
            failure = Some(ToolError::io("write")(e));
            break;
        }
        summary.links += links;
        if vault.is_hidden(std::path::Path::new(&rel)) {
            summary.excluded += 1;
        } else {
            indexed.push(rel.clone());
            summary.visible.push(rel);
        }
    }
    server.after_write_many(&indexed);
    match failure {
        Some(e) => Err(e),
        None => Ok(summary),
    }
}

/// Target text for a link that now has to point at `to`. `files_after` is the
/// file list after the move. Keeps the user's style: a bare name stays bare
/// while it is still unique, a path stays a path.
fn new_target(old: &str, to: &str, after: &FileIndex) -> String {
    let strip_md = !old.trim().ends_with(".md") && to.ends_with(".md");
    let shorten = |s: &str| if strip_md { s.strip_suffix(".md").unwrap_or(s).to_string() } else { s.to_string() };
    let leading = if old.starts_with('/') { "/" } else { "" };
    let full = format!("{leading}{}", shorten(to));
    if !old.contains('/') {
        let name = shorten(to.rsplit('/').next().unwrap_or(to));
        if after.resolve(&name).as_deref() == Some(to) {
            return name;
        }
    }
    full
}

/// Rewrites the links in `text` that resolved to a `from` of `moves` (against
/// `files_before`) so they point at its `to` (resolved against `files_after`).
/// Returns the new text and the number of links changed, or `None` when
/// nothing changes. Links in code are ignored. Alias, fragment and `!` stay.
pub fn relink_text_many(
    text: &str,
    files_before: &[String],
    files_after: &[String],
    moves: &[(String, String)],
) -> Option<(String, usize)> {
    relink_with(text, &FileIndex::new(files_before), &FileIndex::new(files_after), moves)
}

/// Same as `relink_text_many` with the name indexes already built.
fn relink_with(
    text: &str,
    before: &FileIndex,
    after: &FileIndex,
    moves: &[(String, String)],
) -> Option<(String, usize)> {
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for link in parse_links(text) {
        let Some(resolved) = before.resolve(&link.target) else { continue };
        let Some((_, to)) = moves.iter().find(|(f, _)| *f == resolved) else { continue };
        let new = new_target(&link.target, to, after);
        if new != link.target {
            edits.push((link.target_range.clone(), new));
        }
    }
    if edits.is_empty() {
        return None;
    }
    let count = edits.len();
    let mut out = text.to_string();
    for (range, new) in edits.into_iter().rev() {
        out.replace_range(range, &new);
    }
    Some((out, count))
}
