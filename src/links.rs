//! Wikilink and embed parser and resolver.

use std::ops::Range;

/// One `[[...]]` or `![[...]]` occurrence in a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Byte range of the whole link, including brackets and a leading `!`.
    pub range: Range<usize>,
    /// Byte range of the target name only (no `#heading`, `#^block` or `|alias`).
    pub target_range: Range<usize>,
    /// Target name as written.
    pub target: String,
    /// Part after `#`, if any (`heading` or `^block`).
    pub fragment: Option<String>,
    /// Part after `|`, if any.
    pub alias: Option<String>,
    /// True for `![[...]]`.
    pub embed: bool,
}

/// Finds every wikilink and embed, ignoring fenced code blocks and inline code.
pub fn parse_links(_text: &str) -> Vec<Link> {
    todo!()
}

/// Resolves a link target like Obsidian. `files` are vault-relative paths with `/`
/// separators. Exact path wins (`.md` optional), then the shortest path whose
/// trailing segments match the target.
pub fn resolve_target(_target: &str, _files: &[String]) -> Option<String> {
    todo!()
}
