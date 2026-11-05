//! Link rewriting for moves and renames. Pure text functions plus a vault
//! walk that, unlike `visible_files`, includes excluded folders.

use std::path::Path;

use crate::links::{parse_links, resolve_target};
use crate::vault::Vault;

/// Folders no tool may touch, not even to rewrite links.
const ALWAYS_HIDDEN: [&str; 2] = [".obsidian", ".trash"];

fn is_always_hidden(rel: &Path) -> bool {
    rel.components().next().is_some_and(|c| ALWAYS_HIDDEN.iter().any(|h| c.as_os_str() == *h))
}

/// Vault-relative paths (`/` separators) of every file, excluded folders
/// included, `.obsidian/` and `.trash/` skipped. Symlinks leaving the vault
/// are skipped. Sorted.
pub fn all_files(vault: &Vault) -> Vec<String> {
    let mut out = Vec::new();
    walk(vault, vault.root(), &mut out);
    out.sort();
    out
}

fn walk(vault: &Vault, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(canonical) = path.canonicalize() else { continue };
        let Ok(canonical_rel) = canonical.strip_prefix(vault.root()) else { continue };
        let Ok(rel) = path.strip_prefix(vault.root()) else { continue };
        if is_always_hidden(rel) || is_always_hidden(canonical_rel) {
            continue;
        }
        if canonical.is_dir() {
            if entry.file_type().is_ok_and(|t| !t.is_symlink()) {
                walk(vault, &path, out);
            }
        } else {
            out.push(rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"));
        }
    }
}

/// Target text for a link that now has to point at `to`. `files_after` is the
/// file list after the move. Keeps the user's style: a bare name stays bare
/// while it is still unique, a path stays a path.
fn new_target(old: &str, to: &str, files_after: &[String]) -> String {
    let strip_md = !old.trim().ends_with(".md") && to.ends_with(".md");
    let shorten = |s: &str| if strip_md { s.strip_suffix(".md").unwrap_or(s).to_string() } else { s.to_string() };
    let leading = if old.starts_with('/') { "/" } else { "" };
    let full = format!("{leading}{}", shorten(to));
    if !old.contains('/') {
        let name = shorten(to.rsplit('/').next().unwrap_or(to));
        if resolve_target(&name, files_after).as_deref() == Some(to) {
            return name;
        }
    }
    full
}

/// Rewrites the links in `text` that resolved to `from` (against
/// `files_before`) so they point at `to` (resolved against `files_after`).
/// Returns the new text and the number of links changed, or `None` when
/// nothing changes. Links in code are ignored. Alias, fragment and `!` stay.
pub fn relink_text(
    text: &str,
    files_before: &[String],
    files_after: &[String],
    from: &str,
    to: &str,
) -> Option<(String, usize)> {
    relink_text_many(text, files_before, files_after, &[(from.to_string(), to.to_string())])
}

/// Like `relink_text`, for many `(from, to)` pairs at once (a folder move).
pub fn relink_text_many(
    text: &str,
    files_before: &[String],
    files_after: &[String],
    moves: &[(String, String)],
) -> Option<(String, usize)> {
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    for link in parse_links(text) {
        let Some(resolved) = resolve_target(&link.target, files_before) else { continue };
        let Some((_, to)) = moves.iter().find(|(f, _)| *f == resolved) else { continue };
        let new = new_target(&link.target, to, files_after);
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
