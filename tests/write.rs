mod common;

use std::collections::BTreeSet;

use common::fs::{files_under, read};
use common::*;
use mcp_obsidian::server::Options;
use serde_json::json;

#[tokio::test]
async fn h22_create_writes_content() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    c.ok("create_note", json!({"path": "new.md", "content": "# hello\nbody"})).await;
    assert_eq!(read(&v, "new.md"), "# hello\nbody");
}

#[tokio::test]
async fn h23_create_in_missing_folder_creates_it() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    c.ok("create_note", json!({"path": "a/b/c/new.md", "content": "x"})).await;
    assert_eq!(read(&v, "a/b/c/new.md"), "x");
}

#[tokio::test]
async fn h24_create_on_existing_path_fails_and_keeps_note() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let err = c.err("create_note", json!({"path": "root.md", "content": "other"})).await;
    assert!(!err.is_empty());
    assert_eq!(read(&v, "root.md"), "# root");
}

#[tokio::test]
async fn h25_replace_unique_string_changes_only_it() {
    let v = TestVault::standard();
    v.note("n.md", "one two three\nfour");
    let c = start(&v.root).await;
    c.ok("edit_note", json!({"path": "n.md", "old_text": "two", "new_text": "2"})).await;
    assert_eq!(read(&v, "n.md"), "one 2 three\nfour");
}

#[tokio::test]
async fn h26_replace_zero_or_many_matches_fails_and_keeps_note() {
    let v = TestVault::standard();
    v.note("n.md", "aa bb aa");
    let c = start(&v.root).await;
    c.err("edit_note", json!({"path": "n.md", "old_text": "zz", "new_text": "y"})).await;
    assert_eq!(read(&v, "n.md"), "aa bb aa");
    c.err("edit_note", json!({"path": "n.md", "old_text": "aa", "new_text": "y"})).await;
    assert_eq!(read(&v, "n.md"), "aa bb aa");
}

#[tokio::test]
async fn h27_append_adds_at_end() {
    let v = TestVault::standard();
    v.note("n.md", "first\n");
    let c = start(&v.root).await;
    c.ok("edit_note", json!({"path": "n.md", "append": "second\n"})).await;
    assert_eq!(read(&v, "n.md"), "first\nsecond\n");
}

#[tokio::test]
async fn h28_overwrite_needs_explicit_flag() {
    let v = TestVault::standard();
    v.note("n.md", "old");
    let c = start(&v.root).await;
    c.err("edit_note", json!({"path": "n.md", "content": "new"})).await;
    assert_eq!(read(&v, "n.md"), "old");
    c.err("edit_note", json!({"path": "n.md", "content": "new", "overwrite": false})).await;
    assert_eq!(read(&v, "n.md"), "old");
    c.ok("edit_note", json!({"path": "n.md", "content": "new", "overwrite": true})).await;
    assert_eq!(read(&v, "n.md"), "new");
}

#[tokio::test]
async fn edit_missing_note_fails() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    c.err("edit_note", json!({"path": "nope.md", "append": "x"})).await;
    assert!(!v.root.join("nope.md").exists());
}

#[tokio::test]
async fn h29_no_temp_files_remain() {
    let v = TestVault::standard();
    v.note("n.md", "abc abc");
    let c = start(&v.root).await;
    let before = files_under(&v.root);
    c.ok("create_note", json!({"path": "d/new.md", "content": "x"})).await;
    c.ok("edit_note", json!({"path": "n.md", "append": "!"})).await;
    c.ok("edit_note", json!({"path": "n.md", "content": "z", "overwrite": true})).await;
    c.err("edit_note", json!({"path": "n.md", "old_text": "q", "new_text": "r"})).await;
    c.err("create_note", json!({"path": "n.md", "content": "r"})).await;
    let mut expected = before;
    expected.insert("d/new.md".to_string());
    assert_eq!(files_under(&v.root), expected);
}

#[tokio::test]
async fn h38_writes_into_hidden_places_are_not_found_and_change_nothing() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let before = files_under(&v.root);
    for p in [
        "secret/new.md",
        "secret/sub/new.md",
        ".obsidian/new.md",
        ".trash/new.md",
        ".obsidian/app.json",
        "link/new.md",
        "link/sub/new.md",
    ] {
        let e = c.err("create_note", json!({"path": p, "content": "c"})).await;
        assert!(e.to_lowercase().contains("not found"), "{p}: {e}");
        assert!(!e.contains("secret") || p.contains("secret"), "{e}");
    }
    for (p, args) in [
        ("secret/hidden-note.md", json!({"append": "x"})),
        (".trash/deleted.md", json!({"append": "x"})),
        (".obsidian/app.json", json!({"content": "x", "overwrite": true})),
        ("link/outside-file.md", json!({"append": "x"})),
    ] {
        let mut a = args;
        a["path"] = json!(p);
        let e = c.err("edit_note", a).await;
        assert!(e.to_lowercase().contains("not found"), "{p}: {e}");
    }
    assert_eq!(files_under(&v.root), before);
    assert_eq!(files_under(&v.outside), BTreeSet::from(["outside-file.md".to_string()]));
    assert_eq!(read(&v, "secret/hidden-note.md"), "# secret");
}

#[tokio::test]
async fn write_rejects_dotdot_and_absolute_paths() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    c.err("create_note", json!({"path": "../escape.md", "content": "c"})).await;
    c.err("create_note", json!({"path": "a/../../escape.md", "content": "c"})).await;
    let abs = v.outside.join("abs.md").display().to_string();
    c.err("create_note", json!({"path": abs, "content": "c"})).await;
    assert!(!v.outside.join("escape.md").exists());
    assert!(!v.outside.join("abs.md").exists());
}

#[tokio::test]
async fn h39_read_only_hides_write_tools() {
    let v = TestVault::standard();
    let (c, _) = try_start(&v.root, Options { read_only: true }).await.unwrap();
    let names: Vec<String> =
        c.service.list_all_tools().await.unwrap().into_iter().map(|t| t.name.to_string()).collect();
    assert!(names.contains(&"list_notes".to_string()), "{names:?}");
    for w in ["create_note", "edit_note"] {
        assert!(!names.contains(&w.to_string()), "{w} listed: {names:?}");
    }
    c.err("create_note", json!({"path": "x.md", "content": "c"})).await;
    c.err("edit_note", json!({"path": "root.md", "append": "x"})).await;
    assert!(!v.root.join("x.md").exists());
    assert_eq!(read(&v, "root.md"), "# root");
}

#[tokio::test]
async fn write_tools_are_listed_without_read_only() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let names: Vec<String> =
        c.service.list_all_tools().await.unwrap().into_iter().map(|t| t.name.to_string()).collect();
    for w in ["create_note", "edit_note"] {
        assert!(names.contains(&w.to_string()), "{w} missing: {names:?}");
    }
}
