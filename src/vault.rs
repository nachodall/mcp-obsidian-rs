use std::path::{Component, Path, PathBuf};

use crate::config::Config;

/// Folders that are always hidden from every tool.
const HIDDEN_FOLDERS: [&str; 2] = [".obsidian", ".trash"];

/// One entry found by `Vault::walk`.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Vault-relative path with `/` separators.
    pub rel: String,
    /// Absolute path of the entry itself (a symlink is not resolved).
    pub abs: PathBuf,
    /// True when the entry (after following symlinks) is a folder.
    pub is_dir: bool,
    /// True when the entry itself is a symlink.
    pub is_symlink: bool,
}

/// Joins the components of a relative path with `/`.
pub fn join_rel(rel: &Path) -> String {
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// Rejects `..`, absolute paths and prefixes; drops `.` components.
fn clean(relative: &str) -> Result<PathBuf, VaultError> {
    let mut clean = PathBuf::new();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err(VaultError::InvalidPath(relative.to_string())),
        }
    }
    Ok(clean)
}

/// Config file at the vault root. Hidden from every tool.
pub const CONFIG_FILE: &str = ".mcp-obsidian.toml";

/// Prefix of the temp files `fs_write` creates.
pub const TEMP_PREFIX: &str = ".mcp-tmp-";

/// True for what no tool may touch: `.obsidian/`, `.trash/`, the config file
/// at the root and temp files of atomic writes.
fn always_hidden(relative: &Path) -> bool {
    let mut parts = relative.components().filter_map(|c| match c {
        Component::Normal(name) => Some(name.to_string_lossy()),
        _ => None,
    });
    let first = parts.next();
    let first_hidden = first.as_deref().is_some_and(|n| {
        HIDDEN_FOLDERS.contains(&n) || n.eq_ignore_ascii_case(CONFIG_FILE) && relative.components().count() == 1
    });
    first_hidden
        || first.iter().cloned().chain(parts).any(|n| n.starts_with(TEMP_PREFIX))
}

/// True when the path names a markdown note.
pub fn is_note(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// Lowercased path text used to compare against excluded folders.
fn fold(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().to_lowercase())
}

fn not_a_note(relative: &str) -> VaultError {
    VaultError::InvalidPath(format!("only notes can be used here (.md files): {relative}"))
}

/// Single entry point that turns a tool argument into a filesystem path.
#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
    config: Config,
}

/// Errors for path resolution. `NotFound` is used for missing, excluded and
/// hidden paths alike, so the message gives no hint.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

impl Vault {
    pub fn new(root: &Path, config: Config) -> anyhow::Result<Vault> {
        let root = root.canonicalize()?;
        Ok(Vault { root, config })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a vault-relative argument. Empty string means the vault root.
    /// The result is canonical and always inside the vault.
    pub fn resolve(&self, relative: &str) -> Result<PathBuf, VaultError> {
        let clean = clean(relative)?;
        let not_found = || VaultError::NotFound(relative.to_string());
        if self.is_hidden(&clean) {
            return Err(not_found());
        }
        let canonical = self.root.join(&clean).canonicalize().map_err(|_| not_found())?;
        let inside = canonical.strip_prefix(&self.root).map_err(|_| not_found())?;
        // A symlink inside the vault may point at a hidden folder.
        if self.is_hidden(inside) {
            return Err(not_found());
        }
        Ok(canonical)
    }

    /// Resolves a vault-relative path that may not exist yet (write target).
    /// Same rules as `resolve`: no `..`, no absolute paths, nothing hidden or
    /// excluded, no symlink leaving the vault. The deepest existing ancestor is
    /// canonicalized; the missing tail is appended as is.
    pub fn resolve_for_write(&self, relative: &str) -> Result<PathBuf, VaultError> {
        let clean = clean(relative)?;
        let not_found = || VaultError::NotFound(relative.to_string());
        if clean.as_os_str().is_empty() || self.is_hidden(&clean) {
            return Err(not_found());
        }
        let mut existing = self.root.join(&clean);
        let mut tail = PathBuf::new();
        let canonical = loop {
            // A dangling symlink exists but cannot be canonicalized: refuse it.
            if let Ok(canonical) = existing.canonicalize() {
                break canonical;
            }
            if existing.symlink_metadata().is_ok() {
                return Err(not_found());
            }
            let name = existing.file_name().ok_or_else(not_found)?.to_owned();
            let mut next = PathBuf::from(name);
            if !tail.as_os_str().is_empty() {
                next.push(&tail);
            }
            tail = next;
            if !existing.pop() {
                return Err(not_found());
            }
        };
        let inside = canonical.strip_prefix(&self.root).map_err(|_| not_found())?;
        if self.is_hidden(&inside.join(&tail)) {
            return Err(not_found());
        }
        Ok(if tail.as_os_str().is_empty() { canonical } else { canonical.join(tail) })
    }

    /// Like `resolve`, and the target must be an existing file.
    pub fn resolve_file(&self, relative: &str) -> Result<PathBuf, VaultError> {
        let path = self.resolve(relative)?;
        if path.is_file() { Ok(path) } else { Err(VaultError::NotFound(relative.to_string())) }
    }

    /// Like `resolve_file`, and the target must be a `.md` note.
    pub fn resolve_note(&self, relative: &str) -> Result<PathBuf, VaultError> {
        let path = self.resolve_file(relative)?;
        if is_note(&path) { Ok(path) } else { Err(not_a_note(relative)) }
    }

    /// Like `resolve_for_write`, and the target must be named `*.md`.
    pub fn resolve_note_for_write(&self, relative: &str) -> Result<PathBuf, VaultError> {
        let path = self.resolve_for_write(relative)?;
        if is_note(&path) { Ok(path) } else { Err(not_a_note(relative)) }
    }

    /// Like `resolve`, and the target must be an existing folder.
    pub fn resolve_dir(&self, relative: &str) -> Result<PathBuf, VaultError> {
        let path = self.resolve(relative)?;
        if path.is_dir() {
            Ok(path)
        } else {
            Err(VaultError::InvalidPath(format!("{relative} is not a folder")))
        }
    }

    /// Vault-relative path (`/` separators) of an absolute path inside the
    /// vault. The vault root gives an empty string.
    pub fn rel_of(&self, abs: &Path) -> Option<String> {
        abs.strip_prefix(&self.root).ok().map(join_rel)
    }

    /// True when a vault-relative path is excluded or hidden (`.obsidian/`, `.trash/`).
    pub fn is_hidden(&self, relative: &Path) -> bool {
        always_hidden(relative) || {
            // Entries are stored lowercase (see `Config::load`).
            let folded = fold(relative);
            self.config.excluded_folders.iter().any(|f| folded.starts_with(f))
        }
    }

    /// Vault-relative paths of every file, sorted. Excluded folders are listed
    /// only with `include_excluded`; `.obsidian/` and `.trash/` never are.
    pub fn files(&self, include_excluded: bool) -> Vec<String> {
        let mut out: Vec<String> = self
            .walk(&self.root, include_excluded)
            .into_iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.rel)
            .collect();
        out.sort();
        out
    }

    /// Walks `start` (a folder inside the vault) depth first, in file name
    /// order, a folder before its content. Skips entries whose own path or
    /// symlink target (which must be inside the vault) is hidden. Never
    /// follows symlinked folders. With `include_excluded`, only `.obsidian/`
    /// and `.trash/` are hidden; excluded folders are walked.
    pub fn walk(&self, start: &Path, include_excluded: bool) -> Vec<Entry> {
        let mut out = Vec::new();
        self.walk_into(start, include_excluded, &mut out);
        out
    }

    fn walk_into(&self, dir: &Path, include_excluded: bool, out: &mut Vec<Entry>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        let hidden = |p: &Path| if include_excluded { always_hidden(p) } else { self.is_hidden(p) };
        for entry in entries {
            let abs = entry.path();
            // The canonical target must be inside the vault and not hidden.
            let Ok(canonical) = abs.canonicalize() else { continue };
            let Ok(canonical_rel) = canonical.strip_prefix(&self.root) else { continue };
            let Ok(rel) = abs.strip_prefix(&self.root) else { continue };
            if hidden(rel) || hidden(canonical_rel) {
                continue;
            }
            let file_type = entry.file_type().ok();
            let is_symlink = file_type.is_some_and(|t| t.is_symlink());
            let is_dir = canonical.is_dir();
            out.push(Entry { rel: join_rel(rel), abs: abs.clone(), is_dir, is_symlink });
            // Symlinked folders are not followed: avoids cycles.
            if is_dir && file_type.is_some_and(|t| !t.is_symlink()) {
                self.walk_into(&abs, include_excluded, out);
            }
        }
    }
}
