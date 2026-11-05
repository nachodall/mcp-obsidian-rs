/// Truncates `text` to `max_bytes` (on a char boundary) and appends a notice.
pub fn truncate_with_notice(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n\n[truncated: response exceeded the {max_bytes} byte limit]",
        &text[..end]
    )
}
