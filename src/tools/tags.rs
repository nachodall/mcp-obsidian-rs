use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::frontmatter::{self, all_tags, frontmatter_tags, inline_tags, normalize_tag, tag_matches};
use crate::fs_write::write_atomic;
use crate::output::truncate_with_notice;
use crate::server::Server;
use crate::vault::VaultError;

/// Arguments of the `get_tags` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTagsArgs {
    /// Vault-relative path of a note.
    pub path: String,
}

/// Arguments of `add_tag` and `remove_tag`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TagArgs {
    /// Vault-relative path of a note.
    pub path: String,
    /// Tag name, with or without the leading `#`.
    pub tag: String,
}

/// Arguments of the `search_by_tag` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchByTagArgs {
    /// Tag to find, with or without `#`. Nested tags match their parent
    /// (`subject` finds `subject/math`).
    pub tag: String,
}

impl Server {
    fn read_note_text(&self, path: &str) -> Result<(std::path::PathBuf, String), String> {
        let target = self.vault.resolve(path).map_err(|e| e.to_string())?;
        if !target.is_file() {
            return Err(VaultError::NotFound(path.to_string()).to_string());
        }
        let text = std::fs::read_to_string(&target).map_err(|e| format!("read failed: {e}"))?;
        Ok((target, text))
    }

    fn change_tag(&self, args: TagArgs, add: bool) -> Result<String, String> {
        if normalize_tag(&args.tag).is_empty() {
            return Err("tag must not be empty".to_string());
        }
        let (target, text) = self.read_note_text(&args.path)?;
        let updated = if add {
            frontmatter::add_tag(&text, &args.tag)
        } else {
            frontmatter::remove_tag(&text, &args.tag)
        };
        if updated != text {
            write_atomic(&target, updated.as_bytes()).map_err(|e| format!("write failed: {e}"))?;
            self.after_write(&args.path);
        }
        let tag = normalize_tag(&args.tag);
        Ok(if updated == text {
            format!("no change: {tag} in {}", args.path)
        } else if add {
            format!("added {tag} to {}", args.path)
        } else {
            format!("removed {tag} from {}", args.path)
        })
    }
}

fn join(tags: &[String]) -> String {
    if tags.is_empty() { "(none)".to_string() } else { tags.join(", ") }
}

#[tool_router(router = tags_router, vis = "pub(crate)")]
impl Server {
    /// Lists the tags of a note: frontmatter tags and inline `#tags`.
    #[tool(name = "get_tags")]
    async fn get_tags_tool(&self, args: Parameters<GetTagsArgs>) -> Result<String, String> {
        let (_, text) = self.read_note_text(&args.0.path)?;
        Ok(format!(
            "Frontmatter tags: {}\nInline tags: {}",
            join(&frontmatter_tags(&text)),
            join(&inline_tags(&text))
        ))
    }

    /// Finds notes that have a tag, in the frontmatter or inline.
    #[tool(name = "search_by_tag")]
    async fn search_by_tag_tool(&self, args: Parameters<SearchByTagArgs>) -> Result<String, String> {
        let wanted = normalize_tag(&args.0.tag);
        if wanted.is_empty() {
            return Err("tag must not be empty".to_string());
        }
        let phrase: String = wanted
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let candidates = self
            .index
            .search(&format!("tags:\"{phrase}\""), false, None)
            .map_err(|e| format!("search failed: {e}"))?;
        let mut paths: Vec<String> = Vec::new();
        for hit in candidates {
            // The index narrows down; the note text decides.
            let Ok((_, text)) = self.read_note_text(&hit.path) else { continue };
            if all_tags(&text).iter().any(|t| tag_matches(t, &wanted)) {
                paths.push(hit.path);
            }
        }
        paths.sort();
        let out = if paths.is_empty() { "No results.".to_string() } else { paths.join("\n") };
        Ok(truncate_with_notice(&out, self.config.max_response_bytes))
    }
}

#[tool_router(router = tags_write_router, vis = "pub(crate)")]
impl Server {
    /// Adds a tag to the frontmatter `tags` list. Creates the frontmatter when
    /// missing. Every other field and the body stay as they are.
    #[tool(name = "add_tag")]
    async fn add_tag_tool(&self, args: Parameters<TagArgs>) -> Result<String, String> {
        self.change_tag(args.0, true)
    }

    /// Removes a tag from the frontmatter `tags` list. Inline `#tags` stay.
    #[tool(name = "remove_tag")]
    async fn remove_tag_tool(&self, args: Parameters<TagArgs>) -> Result<String, String> {
        self.change_tag(args.0, false)
    }
}
