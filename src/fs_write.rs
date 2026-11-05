use std::io;
use std::path::Path;

/// Writes `bytes` to `path` atomically (temp file in the same folder + rename).
/// Replaces an existing file. Creates no folders.
pub fn write_atomic(_path: &Path, _bytes: &[u8]) -> io::Result<()> {
    todo!("ticket 06")
}

/// Like `write_atomic`, but fails with `AlreadyExists` when `path` exists.
/// Creates missing parent folders.
pub fn create_atomic(_path: &Path, _bytes: &[u8]) -> io::Result<()> {
    todo!("ticket 06")
}
