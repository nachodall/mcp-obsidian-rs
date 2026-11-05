//! Wikilink and embed parser and resolver.

use std::collections::HashMap;
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
pub fn parse_links(text: &str) -> Vec<Link> {
    let bytes = text.as_bytes();
    let mut links = Vec::new();
    let mut fence: Option<(u8, usize)> = None; // (marker char, marker length)
    let mut line_start = 0;
    while line_start < bytes.len() {
        let line_end = text[line_start..].find('\n').map_or(bytes.len(), |i| line_start + i);
        let line = &text[line_start..line_end];
        let trimmed = line.trim_start_matches([' ', '\t']);
        let indent = line.len() - trimmed.len();
        let marker = trimmed.bytes().next().filter(|b| *b == b'`' || *b == b'~');
        let run = marker.map_or(0, |m| trimmed.bytes().take_while(|b| *b == m).count());
        match fence {
            Some((m, len)) => {
                if marker == Some(m) && run >= len && trimmed[run..].trim().is_empty() {
                    fence = None;
                }
            }
            None => {
                if let Some(m) = marker.filter(|_| run >= 3 && indent < 4) {
                    // A backtick fence cannot have backticks in its info string.
                    if m == b'~' || !trimmed[run..].contains('`') {
                        fence = Some((m, run));
                    }
                }
            }
        }
        if fence.is_none() && !(marker.is_some() && run >= 3 && indent < 4 && is_fence_close_line(trimmed, run)) {
            scan_line(text, line_start, line_end, &mut links);
        }
        line_start = line_end + 1;
    }
    links
}

fn is_fence_close_line(trimmed: &str, run: usize) -> bool {
    trimmed[run..].trim().is_empty()
}

/// Scans one line outside a fenced block, skipping inline code spans.
fn scan_line(text: &str, start: usize, end: usize, out: &mut Vec<Link>) {
    let bytes = text.as_bytes();
    let mut i = start;
    while i < end {
        match bytes[i] {
            b'`' => {
                let run = bytes[i..end].iter().take_while(|b| **b == b'`').count();
                match find_backtick_run(bytes, i + run, end, run) {
                    Some(close) => i = close + run,
                    None => i += run, // unmatched: literal backticks
                }
            }
            b'[' if bytes[i..end].starts_with(b"[[") => {
                let embed = i > start && bytes[i - 1] == b'!';
                let inner_start = i + 2;
                match text[inner_start..end].find("]]") {
                    Some(rel) if rel > 0 => {
                        let inner_end = inner_start + rel;
                        if let Some(link) = build_link(text, i, inner_start, inner_end, embed) {
                            out.push(link);
                        }
                        i = inner_end + 2;
                    }
                    _ => i += 2,
                }
            }
            _ => i += 1,
        }
    }
}

fn find_backtick_run(bytes: &[u8], from: usize, end: usize, len: usize) -> Option<usize> {
    let mut i = from;
    while i < end {
        if bytes[i] == b'`' {
            let run = bytes[i..end].iter().take_while(|b| **b == b'`').count();
            if run == len {
                return Some(i);
            }
            i += run;
        } else {
            i += 1;
        }
    }
    None
}

fn build_link(text: &str, open: usize, inner_start: usize, inner_end: usize, embed: bool) -> Option<Link> {
    let inner = &text[inner_start..inner_end];
    if inner.contains('[') {
        return None;
    }
    let name_end_rel = inner.find(['#', '|']).unwrap_or(inner.len());
    let name = &inner[..name_end_rel];
    // Trim whitespace around the name but keep exact byte offsets.
    let lead = name.len() - name.trim_start().len();
    let target = name.trim();
    if target.is_empty() && name_end_rel == 0 && !inner.starts_with('#') {
        return None;
    }
    let target_start = inner_start + lead;
    let rest = &inner[name_end_rel..];
    let (frag_part, alias) = match rest.find('|') {
        Some(p) => (&rest[..p], Some(rest[p + 1..].to_string())),
        None => (rest, None),
    };
    let fragment = frag_part.strip_prefix('#').map(str::to_string);
    let start = if embed { open - 1 } else { open };
    Some(Link {
        range: start..inner_end + 2,
        target_range: target_start..target_start + target.len(),
        target: target.to_string(),
        fragment,
        alias,
        embed,
    })
}

/// Name index over a file list, built once so that each link resolves with
/// hash lookups instead of a scan of every file.
#[derive(Debug, Default)]
pub struct FileIndex<'a> {
    files: &'a [String],
    /// Whole lowercase path -> file positions.
    exact: HashMap<String, Vec<usize>>,
    /// Every lowercase suffix that follows a `/` -> file positions.
    suffix: HashMap<String, Vec<usize>>,
}

impl<'a> FileIndex<'a> {
    pub fn new(files: &'a [String]) -> Self {
        let mut exact: HashMap<String, Vec<usize>> = HashMap::new();
        let mut suffix: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, f) in files.iter().enumerate() {
            let lower = f.to_lowercase();
            for (pos, _) in lower.match_indices('/') {
                suffix.entry(lower[pos + 1..].to_string()).or_default().push(i);
            }
            exact.entry(lower).or_default().push(i);
        }
        FileIndex { files, exact, suffix }
    }

    fn candidates<'s>(
        &'s self,
        map: &'s HashMap<String, Vec<usize>>,
        keys: &'s [String; 2],
    ) -> impl Iterator<Item = &'s String> {
        keys.iter().filter_map(|k| map.get(k)).flatten().map(|&i| &self.files[i])
    }

    /// Resolves a link target like Obsidian. Exact path wins (`.md` optional),
    /// then the shortest path whose trailing segments match the target (ties:
    /// alphabetical). Matching ignores case.
    pub fn resolve(&self, target: &str) -> Option<String> {
        let t = target.trim().trim_start_matches('/').to_lowercase();
        if t.is_empty() {
            return None;
        }
        let keys = [t.clone(), format!("{t}.md")];
        if let Some(f) = self.candidates(&self.exact, &keys).min() {
            return Some(f.clone());
        }
        self.candidates(&self.suffix, &keys)
            .min_by(|a, b| a.matches('/').count().cmp(&b.matches('/').count()).then_with(|| a.cmp(b)))
            .cloned()
    }
}

/// One-shot resolve against a plain list. Build a `FileIndex` when resolving
/// many targets.
pub fn resolve_target(target: &str, files: &[String]) -> Option<String> {
    FileIndex::new(files).resolve(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_form() {
        let text = "[[a]] [[f/b]] [[c|al]] [[d#H]] [[e#^blk]] ![[g.pdf]]";
        let l = parse_links(text);
        assert_eq!(l.len(), 6);
        assert_eq!(l[2].alias.as_deref(), Some("al"));
        assert_eq!(l[3].fragment.as_deref(), Some("H"));
        assert_eq!(l[4].fragment.as_deref(), Some("^blk"));
        assert!(l[5].embed && l[5].target == "g.pdf");
        assert_eq!(&text[l[1].target_range.clone()], "f/b");
        assert_eq!(&text[l[5].range.clone()], "![[g.pdf]]");
    }

    #[test]
    fn ignores_code() {
        let text = "`[[a]]`\n```\n[[b]]\n```\n``x [[c]] y``\n[[d]]";
        let l = parse_links(text);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].target, "d");
    }

    #[test]
    fn resolves_like_obsidian() {
        let files: Vec<String> = ["a/note.md", "b/c/note.md", "x.png", "a/y.md"].map(String::from).to_vec();
        assert_eq!(resolve_target("note", &files).as_deref(), Some("a/note.md"));
        assert_eq!(resolve_target("c/note", &files).as_deref(), Some("b/c/note.md"));
        assert_eq!(resolve_target("x.png", &files).as_deref(), Some("x.png"));
        assert_eq!(resolve_target("a/y.md", &files).as_deref(), Some("a/y.md"));
        assert_eq!(resolve_target("nope", &files), None);
    }
}
