# mcp-obsidian

A local [MCP](https://modelcontextprotocol.io) server that lets Claude read,
search and organize an Obsidian vault. It runs over stdio, opens no network
port and calls no external service.

## Install

```sh
cargo install --path .
```

This puts the `mcp-obsidian` binary in `~/.cargo/bin`. Run it as:

```sh
mcp-obsidian <vault-path> [--read-only]
```

With `--read-only`, the server hides every write tool.

## Register in Claude Code

```sh
claude mcp add obsidian --scope user -- mcp-obsidian "/path/to/your/vault"
```

For a read-only server, add `--read-only` after the vault path.

## Register in Claude Desktop

Add this entry to `claude_desktop_config.json`, then restart the app:

```json
{
  "mcpServers": {
    "obsidian": {
      "command": "mcp-obsidian",
      "args": ["/path/to/your/vault"]
    }
  }
}
```

If the app cannot find the binary, use the full path to `mcp-obsidian` as
`command`.

## Tools

Tools marked **write** change the vault. `--read-only` hides them.

| Tool | Arguments | What it does |
| --- | --- | --- |
| `list_notes` | `folder?` | Lists notes, folders and attachments in the vault, or in one folder. |
| `read_note` | `path`, `offset?`, `limit?` | Reads a note, whole or by line range. |
| `search_notes` | `query`, `fuzzy?`, `folder?` | Full-text search. Returns ranked notes with path and matching lines. |
| `get_backlinks` | `path` | Lists the notes that link to a note, in any wikilink or embed form. |
| `get_tags` | `path` | Lists the tags of a note: frontmatter tags and inline `#tags`. |
| `search_by_tag` | `tag` | Finds notes by tag. Nested tags match their parent. |
| `create_note` (write) | `path`, `content` | Creates a new note. Fails when the path exists. |
| `edit_note` (write) | `path`, `old_text`+`new_text` or `append` or `content`+`overwrite` | Replaces text that appears once, appends, or overwrites the whole note. |
| `add_tag` (write) | `path`, `tag` | Adds a tag to the frontmatter `tags` list. |
| `remove_tag` (write) | `path`, `tag` | Removes a tag from the frontmatter `tags` list. |
| `move_note` (write) | `from`, `to` | Moves or renames a note or attachment. Updates every link. Fails when the target exists. |
| `move_folder` (write) | `from`, `to` | Moves a folder with all its notes and attachments. Updates every link. |
| `delete_note` (write) | `path` | Moves a note to `.trash/`. Nothing is removed from disk. |

All paths are relative to the vault root.

## Manual or automatic approval

You choose how much Claude may do without asking.

### Claude Code

Set permissions in `settings.json`. Allow the read tools. Ask for the write
tools.

```json
{
  "permissions": {
    "allow": [
      "mcp__obsidian__list_notes",
      "mcp__obsidian__read_note",
      "mcp__obsidian__search_notes",
      "mcp__obsidian__get_backlinks",
      "mcp__obsidian__get_tags",
      "mcp__obsidian__search_by_tag"
    ],
    "ask": [
      "mcp__obsidian__create_note",
      "mcp__obsidian__edit_note",
      "mcp__obsidian__add_tag",
      "mcp__obsidian__remove_tag",
      "mcp__obsidian__move_note",
      "mcp__obsidian__move_folder",
      "mcp__obsidian__delete_note"
    ]
  }
}
```

### Claude Desktop

Claude Desktop asks before each tool call. Choose "Always allow" for the tools
you trust.

### Hard switch

`--read-only` does not depend on any client setting. The write tools do not
exist, so Claude cannot call them.

## Protect excluded folders from Claude Code's own tools

Excluded folders only apply to this server. Claude Code also has its own
`Read`, `Edit` and `Bash` tools. They read files straight from disk and
ignore the exclusion. Add `deny` rules for each excluded folder:

```json
{
  "permissions": {
    "deny": [
      "Read(/path/to/your/vault/Private/**)",
      "Edit(/path/to/your/vault/Private/**)"
    ]
  }
}
```

Do not auto-allow `Bash` in the vault folder. A shell command can read any
file, and `deny` rules for `Read` do not stop it.

## Configuration

Put a `.mcp-obsidian.toml` file at the vault root. The file is optional.

```toml
# Folders no tool lists, reads or searches.
excluded_folders = ["Private", "Journal/2020"]

# Maximum size of a tool response, in bytes. Default: 51200 (50 KB).
max_response_bytes = 51200
```

| Key | Type | Meaning |
| --- | --- | --- |
| `excluded_folders` | list of strings | Vault-relative folders that stay hidden. Default: none. |
| `max_response_bytes` | integer | Longer responses are cut, with a notice. Default: 51200. |

Rules:

- Unknown keys are errors.
- An invalid file stops the server. The error says what is wrong. Fix the file
  and start again.
- `.obsidian/`, `.trash/` and `.mcp-obsidian.toml` itself are always hidden.
  You do not need to list them. No tool can read, move or edit the config file.
- Folder names in `excluded_folders` ignore case, a leading `./` and slashes at
  the ends. `"Private"`, `"private/"` and `"./private"` all hide `private/`.
- An empty, absolute or `..` entry stops the server.
- `create_note`, `edit_note`, `add_tag`, `remove_tag`, `get_tags` and
  `delete_note` work on `.md` notes only. `move_note` can still move
  attachments.
- `delete_note` moves the note to `.trash/`. You can restore it from there.
  If `.trash` is a link that leaves the vault, `delete_note` fails.
- Every tool response, errors included, is cut at `max_response_bytes`.
- Edits keep the file permissions.

## Privacy

- The server uses stdio only. It opens no network port and makes no network
  calls.
- The search index lives in memory only. The server writes no index to disk.
- Claude sends the notes it reads to its model provider. Treat any note Claude
  reads as shared with that provider.
- Excluded folders never leave your machine. No tool lists, reads or searches
  them. Add the `deny` rules above so Claude Code's own tools respect that too.

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

## Glossary

### Vault
The Obsidian folder the server exposes. One per server process.
Avoid: brain, repository, workspace.

### Note
A `.md` file inside the vault.
Avoid: page, document, file (when it means a `.md` note).

### Attachment
A non-`.md` file inside the vault (PDF, image). Listed and moved, never read.
Avoid: asset, media, binary.

### Excluded folder
A vault folder that no tool lists, reads or searches. Configured in
`.mcp-obsidian.toml`.
Avoid: private folder, hidden folder, ignored folder.

### Scope
The optional folder that limits a list or search call. Excluded folders stay
out even inside a scope.
Avoid: filter, root, path filter.

### Trash
The vault's `.trash/` folder. Delete moves notes there; nothing is removed
from disk.
Avoid: bin, recycle bin, archive.
