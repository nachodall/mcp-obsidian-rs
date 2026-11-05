//! Atomic file writes: temp file in the same folder, then rename.
//! Shared by every write tool (create, edit, move, ...).

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(path: &Path) -> io::Result<PathBuf> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(dir.join(format!(".mcp-tmp-{}-{n}", std::process::id())))
}

/// Writes the temp file next to `path`. Removes it again on failure.
fn write_temp(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let tmp = temp_path(path)?;
    let result = (|| {
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    match result {
        Ok(()) => Ok(tmp),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Writes `bytes` to `path` atomically (temp file in the same folder + rename).
/// Replaces an existing file. Creates no folders.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = write_temp(path, bytes)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Like `write_atomic`, but fails with `AlreadyExists` when `path` exists
/// (never replaces a file, even under a race). Creates missing parent folders.
pub fn create_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = write_temp(path, bytes)?;
    // hard_link fails when the target exists, so it cannot overwrite.
    let linked = std::fs::hard_link(&tmp, path);
    let _ = std::fs::remove_file(&tmp);
    linked
}
