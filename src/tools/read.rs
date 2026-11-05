use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde::Deserialize;

use crate::error::ToolError;
use crate::server::Server;
use crate::vault::{Vault, VaultError};

/// Arguments of the `read_note` tool.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ReadNoteArgs {
    /// Vault-relative path of the note.
    pub path: String,
    /// First line to return, 1-based. Default 1.
    pub offset: Option<usize>,
    /// Maximum number of lines to return. Default: all.
    pub limit: Option<usize>,
}

/// Reads a note whole or by line range (`offset` is 1-based). When a range is
/// requested the result starts with a `[lines a-b of N]` header. Output over
/// `max_bytes` is cut at a line boundary and ends with a notice that gives the
/// `offset` to continue from.
pub fn read_note(vault: &Vault, args: ReadNoteArgs, max_bytes: usize) -> Result<String, ToolError> {
    let path = vault.resolve(&args.path)?;
    if path.extension().is_none_or(|e| e != "md") {
        return Err(VaultError::InvalidPath("only notes can be read (.md files)".into()).into());
    }
    if !path.is_file() {
        return Err(VaultError::NotFound(args.path).into());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|_| VaultError::InvalidPath("the note could not be read as text".into()))?;
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let total = lines.len();
    let ranged = args.offset.is_some() || args.limit.is_some();
    let start = args.offset.unwrap_or(1).max(1) - 1;
    let end = args.limit.map_or(total, |l| start.saturating_add(l)).min(total);
    let selected = if start < end { &lines[start..end] } else { &[][..] };

    let mut out = String::new();
    if ranged {
        let first = if selected.is_empty() { 0 } else { start + 1 };
        out.push_str(&format!("[lines {first}-{end} of {total}]\n", end = start + selected.len()));
    }
    for (used, line) in selected.iter().enumerate() {
        if out.len() + line.len() > max_bytes {
            let next = start + used + 1;
            if used == 0 {
                // Not even one line fits: cut inside the line.
                let mut cut = max_bytes.saturating_sub(out.len()).min(line.len());
                while !line.is_char_boundary(cut) {
                    cut -= 1;
                }
                out.push_str(&line[..cut]);
                out.push('\n');
                out.push_str(&format!(
                    "\n[truncated: response exceeded the {max_bytes} byte limit; line {next} is cut, use offset={} to continue]",
                    next + 1
                ));
            } else {
                out.push_str(&format!(
                    "\n[truncated: response exceeded the {max_bytes} byte limit; use offset={next} to read the rest]"
                ));
            }
            return Ok(out);
        }
        out.push_str(line);
    }
    Ok(out)
}

#[tool_router(router = read_router, vis = "pub(crate)")]
impl Server {
    /// Reads a note, whole or by line range.
    #[tool(name = "read_note")]
    async fn read_note_tool(&self, args: Parameters<ReadNoteArgs>) -> Result<String, String> {
        read_note(&self.vault, args.0, self.config.max_response_bytes).map_err(String::from)
    }
}
