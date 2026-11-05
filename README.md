# mcp-obsidian

A local [MCP](https://modelcontextprotocol.io) server that lets Claude read,
search and organize an Obsidian vault. It runs over stdio, opens no network
port and calls no external service.

> Work in progress.

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
