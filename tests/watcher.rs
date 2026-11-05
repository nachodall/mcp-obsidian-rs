mod common;

use std::fs;
use std::time::{Duration, Instant};

use common::*;
use serde_json::json;

const TIMEOUT: Duration = Duration::from_secs(8);

/// Polls `search_notes` until `done(output)` is true; panics on timeout.
async fn wait_for(c: &Client, query: &str, what: &str, done: impl Fn(&str) -> bool) {
    let start = Instant::now();
    let mut last = String::new();
    while start.elapsed() < TIMEOUT {
        last = c.ok("search_notes", json!({"query": query})).await;
        if done(&last) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timeout waiting for: {what}; last output: {last}");
}

#[tokio::test]
async fn h17_created_note_becomes_searchable() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    fs::write(v.root.join("projects/fresh.md"), "zxcreatedword here\n").unwrap();
    wait_for(&c, "zxcreatedword", "created note", |o| o.contains("projects/fresh.md")).await;
}

#[tokio::test]
async fn h17_edited_note_is_reindexed() {
    let v = TestVault::standard();
    v.note("projects/edit.md", "zxoldword here\n");
    let c = start(&v.root).await;
    assert!(c.ok("search_notes", json!({"query": "zxoldword"})).await.contains("projects/edit.md"));
    fs::write(v.root.join("projects/edit.md"), "zxnewword here\n").unwrap();
    wait_for(&c, "zxnewword", "edited note (new text)", |o| o.contains("projects/edit.md")).await;
    wait_for(&c, "zxoldword", "edited note (old text gone)", |o| !o.contains("projects/edit.md")).await;
}

#[tokio::test]
async fn h17_deleted_note_disappears() {
    let v = TestVault::standard();
    v.note("projects/gone.md", "zxgoneword here\n");
    let c = start(&v.root).await;
    assert!(c.ok("search_notes", json!({"query": "zxgoneword"})).await.contains("projects/gone.md"));
    fs::remove_file(v.root.join("projects/gone.md")).unwrap();
    wait_for(&c, "zxgoneword", "deleted note", |o| !o.contains("projects/gone.md")).await;
}

#[tokio::test]
async fn h17_renamed_note_moves_in_the_index() {
    let v = TestVault::standard();
    v.note("projects/before.md", "zxrenameword here\n");
    let c = start(&v.root).await;
    fs::rename(v.root.join("projects/before.md"), v.root.join("projects/after.md")).unwrap();
    wait_for(&c, "zxrenameword", "renamed note", |o| {
        o.contains("projects/after.md") && !o.contains("projects/before.md")
    })
    .await;
}

#[tokio::test]
async fn h17_folder_rename_reindexes_notes_under_it() {
    let v = TestVault::standard();
    v.note("olddir/inner/n.md", "zxfolderword here\n");
    let c = start(&v.root).await;
    fs::rename(v.root.join("olddir"), v.root.join("newdir")).unwrap();
    wait_for(&c, "zxfolderword", "folder rename", |o| {
        o.contains("newdir/inner/n.md") && !o.contains("olddir/")
    })
    .await;
}

#[tokio::test]
async fn h17_folder_delete_removes_notes_under_it() {
    let v = TestVault::standard();
    v.note("doomed/n.md", "zxdoomedword here\n");
    let c = start(&v.root).await;
    assert!(c.ok("search_notes", json!({"query": "zxdoomedword"})).await.contains("doomed/n.md"));
    fs::remove_dir_all(v.root.join("doomed")).unwrap();
    wait_for(&c, "zxdoomedword", "folder delete", |o| !o.contains("doomed/n.md")).await;
}

#[tokio::test]
async fn h17_excluded_obsidian_and_trash_events_are_ignored() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    fs::write(v.root.join("secret/new.md"), "zxignoredword\n").unwrap();
    fs::write(v.root.join(".obsidian/new.md"), "zxignoredword\n").unwrap();
    fs::write(v.root.join(".trash/new.md"), "zxignoredword\n").unwrap();
    // Control note written last: once it shows up, earlier events were processed.
    fs::write(v.root.join("projects/control.md"), "zxcontrolword\n").unwrap();
    wait_for(&c, "zxcontrolword", "control note", |o| o.contains("projects/control.md")).await;
    let out = c.ok("search_notes", json!({"query": "zxignoredword"})).await;
    assert_eq!(out.trim(), "No results.", "{out}");
}

#[tokio::test]
async fn h17_burst_of_writes_ends_with_the_last_content() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let p = v.root.join("projects/burst.md");
    for i in 0..30 {
        fs::write(&p, format!("zxburst{i} filler\n")).unwrap();
    }
    wait_for(&c, "zxburst29", "last write of the burst", |o| o.contains("projects/burst.md")).await;
    let old = c.ok("search_notes", json!({"query": "zxburst3"})).await;
    assert!(!old.contains("projects/burst.md"), "stale content: {old}");
}
