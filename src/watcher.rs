//! Keeps the search index in sync with edits made on disk by other programs.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::index::SearchIndex;
use crate::vault::Vault;

/// Events that arrive within this window are handled as one batch.
const DEBOUNCE: Duration = Duration::from_millis(100);

/// Handle of the running watcher. Dropping it stops the watcher thread.
pub struct VaultWatcher {
    _watcher: Mutex<RecommendedWatcher>,
}

impl std::fmt::Debug for VaultWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VaultWatcher")
    }
}

/// Starts watching the vault root. Errors while running go to stderr.
pub fn start(vault: Arc<Vault>, index: Arc<SearchIndex>) -> anyhow::Result<VaultWatcher> {
    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    watcher.watch(vault.root(), RecursiveMode::Recursive)?;

    let mut known = BTreeSet::new();
    collect_notes(&vault, vault.root(), &mut known);

    std::thread::spawn(move || {
        let mut known = known;
        loop {
            let Ok(first) = rx.recv() else { return };
            let mut paths: HashSet<PathBuf> = HashSet::new();
            let mut take = |res: notify::Result<notify::Event>| match res {
                Ok(ev) => paths.extend(ev.paths),
                Err(e) => eprintln!("watcher error: {e}"),
            };
            take(first);
            loop {
                match rx.recv_timeout(DEBOUNCE) {
                    Ok(res) => take(res),
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            for p in paths {
                handle(&vault, &index, &mut known, &p);
            }
        }
    });
    Ok(VaultWatcher { _watcher: Mutex::new(watcher) })
}

fn rel_of(vault: &Vault, abs: &Path) -> Option<PathBuf> {
    let rel = abs.strip_prefix(vault.root()).ok()?;
    if rel.as_os_str().is_empty() || vault.is_hidden(rel) {
        return None;
    }
    Some(rel.to_path_buf())
}

fn rel_string(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Adds every note under `dir` (not following symlinks) to `out`.
fn collect_notes(vault: &Vault, dir: &Path, out: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(rel) = rel_of(vault, &path) else { continue };
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            collect_notes(vault, &path, out);
        } else if kind.is_file() && path.extension().is_some_and(|e| e == "md") {
            out.insert(rel_string(&rel));
        }
    }
}

fn handle(vault: &Vault, index: &SearchIndex, known: &mut BTreeSet<String>, abs: &Path) {
    let Some(rel) = rel_of(vault, abs) else { return };
    let rel_s = rel_string(&rel);
    let meta = std::fs::symlink_metadata(abs);
    let result = match meta {
        Err(_) => {
            // Gone: a note, or a folder with notes under it.
            let prefix = format!("{rel_s}/");
            let gone: Vec<String> = known
                .iter()
                .filter(|k| **k == rel_s || k.starts_with(&prefix))
                .cloned()
                .collect();
            let mut res = Ok(());
            for k in gone {
                known.remove(&k);
                res = res.and(index.remove(&k));
            }
            res
        }
        Ok(m) if m.is_dir() => {
            let mut found = BTreeSet::new();
            collect_notes(vault, abs, &mut found);
            let mut res = Ok(());
            for k in found {
                res = res.and(index.upsert(&k));
                known.insert(k);
            }
            res
        }
        Ok(_) if rel_s.ends_with(".md") => {
            known.insert(rel_s.clone());
            index.upsert(&rel_s)
        }
        Ok(_) => Ok(()),
    };
    if let Err(e) = result {
        eprintln!("watcher: failed to update the index for {rel_s}: {e:#}");
    }
}
