//! Filesystem helpers shared by the integration tests.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::TestVault;

/// Vault-relative paths of every non-folder entry under `root`.
pub fn files_under(root: &Path) -> BTreeSet<String> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeSet<String>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if e.file_type().unwrap().is_dir() {
                walk(&p, root, out);
            } else {
                out.insert(p.strip_prefix(root).unwrap().display().to_string());
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(root, root, &mut out);
    out
}

/// Snapshot of every regular file under `root`: one `"<path>\n<content>"` entry each.
pub fn files_with_content(root: &Path) -> BTreeSet<String> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeSet<String>) {
        for e in fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if e.file_type().unwrap().is_dir() {
                walk(&p, root, out);
            } else if e.file_type().unwrap().is_file() {
                let rel = p.strip_prefix(root).unwrap().display().to_string();
                let content = fs::read_to_string(&p).unwrap_or_default();
                out.insert(format!("{rel}\n{content}"));
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(root, root, &mut out);
    out
}

/// Content of a vault file.
pub fn read(v: &TestVault, rel: &str) -> String {
    fs::read_to_string(v.root.join(rel)).unwrap()
}
