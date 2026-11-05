//! Textual YAML frontmatter and tag handling.
//!
//! Only the `tags` field is read or edited, line by line. Every other line of
//! the note is copied back untouched, so field order and formatting survive.

/// Line index range of the frontmatter content (between the `---` lines).
struct Block {
    /// First line after the opening `---`.
    start: usize,
    /// Index of the closing `---` line.
    end: usize,
}

fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

fn bare(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

fn frontmatter_block(lines: &[&str]) -> Option<Block> {
    if lines.first().map(|l| bare(l).trim_end()) != Some("---") {
        return None;
    }
    let end = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| matches!(bare(l).trim_end(), "---" | "..."))?
        .0;
    Some(Block { start: 1, end })
}

/// The `tags` field inside the frontmatter: line range and the parsed items.
struct TagsField {
    /// Line of the `tags:` key.
    key: usize,
    /// One past the last line that belongs to the field.
    stop: usize,
}

fn find_tags_field(lines: &[&str], block: &Block) -> Option<TagsField> {
    let key = (block.start..block.end).find(|&i| {
        let l = bare(lines[i]);
        l.strip_prefix("tags").is_some_and(|r| r.trim_start().starts_with(':'))
    })?;
    let value = bare(lines[key]).split_once(':').map(|x| x.1).unwrap_or("").trim();
    let mut stop = key + 1;
    if value.starts_with('[') && !value.contains(']') {
        // Multi-line flow list.
        while stop < block.end {
            let done = lines[stop].contains(']');
            stop += 1;
            if done {
                break;
            }
        }
    } else {
        while stop < block.end {
            let l = bare(lines[stop]);
            let continues = l.starts_with(char::is_whitespace) || l.starts_with("- ") || l == "-";
            if !continues {
                break;
            }
            stop += 1;
        }
    }
    Some(TagsField { key, stop })
}

fn clean_tag(raw: &str) -> String {
    let t = raw.trim().trim_matches(['"', '\'']).trim();
    t.trim_start_matches('#').to_string()
}

/// Normalizes a tag given by a caller: no `#`, no surrounding space.
pub fn normalize_tag(tag: &str) -> String {
    clean_tag(tag)
}

fn parse_items(lines: &[&str], field: &TagsField) -> Vec<String> {
    let first = bare(lines[field.key]).split_once(':').map(|x| x.1).unwrap_or("").trim();
    let mut items = Vec::new();
    if first.starts_with('[') {
        let mut joined = first.to_string();
        for l in &lines[field.key + 1..field.stop] {
            joined.push(' ');
            joined.push_str(bare(l));
        }
        let inner = joined.trim_start_matches('[');
        let inner = inner.split(']').next().unwrap_or("");
        items.extend(inner.split(',').map(clean_tag));
    } else if !first.is_empty() && !first.starts_with('#') {
        let value = first.split(" #").next().unwrap_or(first);
        items.extend(value.split([',', ' ']).map(clean_tag));
    }
    for l in &lines[field.key + 1..field.stop] {
        if let Some(item) = bare(l).trim_start().strip_prefix('-') {
            items.push(clean_tag(item.split(" #").next().unwrap_or(item)));
        }
    }
    items.retain(|t| !t.is_empty());
    items
}

/// Tags listed in the frontmatter `tags` field.
pub fn frontmatter_tags(text: &str) -> Vec<String> {
    let lines = lines_of(text);
    let Some(block) = frontmatter_block(&lines) else { return Vec::new() };
    let Some(field) = find_tags_field(&lines, &block) else { return Vec::new() };
    parse_items(&lines, &field)
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '/')
}

/// Inline `#tag` tags in the body. Ignores the frontmatter, fenced code,
/// inline code, headings and `#` glued to a word (links such as `[[a#b]]`).
pub fn inline_tags(text: &str) -> Vec<String> {
    let lines = lines_of(text);
    let body_start = frontmatter_block(&lines).map_or(0, |b| b.end + 1);
    let mut out: Vec<String> = Vec::new();
    let mut fence: Option<String> = None;
    for raw in &lines[body_start..] {
        let line = bare(raw);
        let trimmed = line.trim_start();
        let marker: String = trimmed.chars().take_while(|c| matches!(c, '`' | '~')).collect();
        if marker.len() >= 3 {
            match &fence {
                None => fence = Some(marker),
                Some(open) if marker.starts_with(open.as_str()) => fence = None,
                _ => {}
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with([' ', '\t']) {
            continue;
        }
        scan_line(line, &mut out);
    }
    out
}

fn scan_line(line: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut in_code = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            in_code = !in_code;
        } else if c == '#' && !in_code {
            let before_ok = i == 0 || !(is_tag_char(chars[i - 1]) || matches!(chars[i - 1], '#' | '&' | '\\'));
            let mut j = i + 1;
            while j < chars.len() && is_tag_char(chars[j]) {
                j += 1;
            }
            let tag: String = chars[i + 1..j].iter().collect();
            let tag = tag.trim_end_matches('/').to_string();
            if before_ok && !tag.is_empty() && tag.chars().any(|c| !c.is_ascii_digit()) {
                out.push(tag);
            }
            i = j.max(i + 1);
            continue;
        }
        i += 1;
    }
}

/// Every tag of the note (frontmatter first, then inline), without duplicates.
pub fn all_tags(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in frontmatter_tags(text).into_iter().chain(inline_tags(text)) {
        if !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// Case-insensitive match; `a` also matches the nested `a/b`.
pub fn tag_matches(tag: &str, wanted: &str) -> bool {
    let (tag, wanted) = (tag.to_lowercase(), wanted.to_lowercase());
    tag == wanted || tag.starts_with(&format!("{wanted}/"))
}

fn flow(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

fn line_end(line: &str) -> &str {
    if line.ends_with("\r\n") { "\r\n" } else { "\n" }
}

/// Returns the note with `tag` added to the frontmatter `tags` list.
/// Creates the field, or the whole frontmatter, when missing.
pub fn add_tag(text: &str, tag: &str) -> String {
    let tag = normalize_tag(tag);
    let lines = lines_of(text);
    let Some(block) = frontmatter_block(&lines) else {
        return format!("---\ntags:\n  - {tag}\n---\n{text}");
    };
    let eol = line_end(lines[block.start - 1]);
    let Some(field) = find_tags_field(&lines, &block) else {
        let mut out: String = lines[..block.end].concat();
        if !out.ends_with('\n') {
            out.push_str(eol);
        }
        out.push_str(&format!("tags:{eol}  - {tag}{eol}"));
        out.push_str(&lines[block.end..].concat());
        return out;
    };
    let items = parse_items(&lines, &field);
    if items.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
        return text.to_string();
    }
    let key_line = bare(lines[field.key]);
    let value = key_line.split_once(':').map(|x| x.1).unwrap_or("").trim();
    let mut out: String = lines[..field.key].concat();
    let key_eol = line_end(lines[field.key]);
    if value.starts_with('[') || (!value.is_empty() && !value.starts_with('#')) {
        let mut items = items;
        items.push(tag);
        let key = key_line.split_once(':').map(|x| x.0).unwrap_or("tags");
        out.push_str(&format!("{key}: {}{key_eol}", flow(&items)));
    } else {
        // Block list (or empty field).
        let indent = lines[field.key + 1..field.stop]
            .iter()
            .find(|l| bare(l).trim_start().starts_with('-'))
            .map(|l| bare(l)[..bare(l).len() - bare(l).trim_start().len()].to_string())
            .unwrap_or_else(|| "  ".to_string());
        out.push_str(lines[field.key]);
        if !out.ends_with('\n') {
            out.push_str(key_eol);
        }
        let mut last_item = field.key;
        for (i, l) in lines.iter().enumerate().take(field.stop).skip(field.key + 1) {
            if bare(l).trim_start().starts_with('-') {
                last_item = i;
            }
        }
        for l in &lines[field.key + 1..=last_item] {
            out.push_str(l);
        }
        if !out.ends_with('\n') {
            out.push_str(key_eol);
        }
        out.push_str(&format!("{indent}- {tag}{key_eol}"));
        out.push_str(&lines[last_item.max(field.key) + 1..field.stop].concat());
    }
    out.push_str(&lines[field.stop..].concat());
    out
}

/// Returns the note with `tag` removed from the frontmatter `tags` list.
/// Unchanged when the tag is not there. Inline tags are never touched.
pub fn remove_tag(text: &str, tag: &str) -> String {
    let tag = normalize_tag(tag);
    let lines = lines_of(text);
    let Some(block) = frontmatter_block(&lines) else { return text.to_string() };
    let Some(field) = find_tags_field(&lines, &block) else { return text.to_string() };
    let items = parse_items(&lines, &field);
    if !items.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
        return text.to_string();
    }
    let key_line = bare(lines[field.key]);
    let key_eol = line_end(lines[field.key]);
    let value = key_line.split_once(':').map(|x| x.1).unwrap_or("").trim();
    let mut out: String = lines[..field.key].concat();
    if value.starts_with('[') || (!value.is_empty() && !value.starts_with('#')) {
        let rest: Vec<String> = items.into_iter().filter(|t| !t.eq_ignore_ascii_case(&tag)).collect();
        let key = key_line.split_once(':').map(|x| x.0).unwrap_or("tags");
        out.push_str(&format!("{key}: {}{key_eol}", flow(&rest)));
    } else {
        let kept: Vec<&str> = lines[field.key + 1..field.stop]
            .iter()
            .copied()
            .filter(|l| match bare(l).trim_start().strip_prefix('-') {
                Some(item) => !clean_tag(item.split(" #").next().unwrap_or(item)).eq_ignore_ascii_case(&tag),
                None => true,
            })
            .collect();
        if kept.iter().any(|l| bare(l).trim_start().starts_with('-')) {
            out.push_str(lines[field.key]);
            if !out.ends_with('\n') {
                out.push_str(key_eol);
            }
            out.push_str(&kept.concat());
        } else {
            out.push_str(&format!("tags: []{key_eol}"));
        }
    }
    out.push_str(&lines[field.stop..].concat());
    out
}
