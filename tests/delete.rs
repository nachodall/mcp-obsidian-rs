mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use common::*;
use mcp_obsidian::server::Options;
use serde_json::json;

fn files_under(root: &Path) -> BTreeSet<String> {
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

#[tokio::test]
async fn h35_delete_moves_note_to_trash_intact_and_hides_it() {
    let v = TestVault::standard();
    v.note("projects/gone.md", "# gone\nzebrafish unique body");
    let c = start(&v.root).await;
    assert!(c.ok("search_notes", json!({"query": "zebrafish"})).await.contains("projects/gone.md"));

    c.ok("delete_note", json!({"path": "projects/gone.md"})).await;

    assert!(!v.root.join("projects/gone.md").exists());
    assert_eq!(
        fs::read_to_string(v.root.join(".trash/gone.md")).unwrap(),
        "# gone\nzebrafish unique body"
    );
    let list = c.ok("list_notes", json!({"folder": "projects"})).await;
    assert!(!list.contains("gone.md"), "{list}");
    let e = c.err("read_note", json!({"path": "projects/gone.md"})).await;
    assert!(e.contains("not found"), "{e}");
    let found = c.ok("search_notes", json!({"query": "zebrafish"})).await;
    assert!(!found.contains("gone.md"), "{found}");
}

#[tokio::test]
async fn delete_creates_trash_folder_when_missing() {
    let v = TestVault::new();
    v.note("a.md", "# a");
    let c = start(&v.root).await;
    c.ok("delete_note", json!({"path": "a.md"})).await;
    assert_eq!(fs::read_to_string(v.root.join(".trash/a.md")).unwrap(), "# a");
}

#[tokio::test]
async fn trash_name_clash_never_overwrites() {
    let v = TestVault::standard();
    v.note("one/deleted.md", "# first");
    v.note("two/deleted.md", "# second");
    let c = start(&v.root).await;
    c.ok("delete_note", json!({"path": "one/deleted.md"})).await;
    c.ok("delete_note", json!({"path": "two/deleted.md"})).await;

    assert_eq!(fs::read_to_string(v.root.join(".trash/deleted.md")).unwrap(), "# deleted");
    let trash: Vec<String> = fs::read_dir(v.root.join(".trash"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(trash.len(), 3, "{trash:?}");
    let mut contents: Vec<String> = trash
        .iter()
        .map(|n| fs::read_to_string(v.root.join(".trash").join(n)).unwrap())
        .collect();
    contents.sort();
    assert_eq!(contents, vec!["# deleted", "# first", "# second"]);
}

#[tokio::test]
async fn delete_attachment_fails_and_changes_nothing() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let before = files_under(&v.root);
    c.err("delete_note", json!({"path": "projects/diagram.png"})).await;
    assert_eq!(files_under(&v.root), before);
}

#[tokio::test]
async fn h38_delete_hidden_or_missing_is_not_found_and_changes_nothing() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let before = files_under(&v.root);
    for p in [
        "secret/hidden-note.md",
        ".trash/deleted.md",
        ".obsidian/app.json",
        "link/outside-file.md",
        "nothing/there.md",
    ] {
        let e = c.err("delete_note", json!({"path": p})).await;
        assert!(e.to_lowercase().contains("not found"), "{p}: {e}");
    }
    assert_eq!(files_under(&v.root), before);
    assert_eq!(files_under(&v.outside), BTreeSet::from(["outside-file.md".to_string()]));
}

#[tokio::test]
async fn read_only_hides_delete_tool() {
    let v = TestVault::standard();
    let (c, _) = try_start(&v.root, Options { read_only: true }).await.unwrap();
    let names: Vec<String> =
        c.service.list_all_tools().await.unwrap().into_iter().map(|t| t.name.to_string()).collect();
    assert!(!names.contains(&"delete_note".to_string()), "{names:?}");
    c.err("delete_note", json!({"path": "root.md"})).await;
    assert!(v.root.join("root.md").exists());
}

#[tokio::test]
async fn delete_tool_is_listed_without_read_only() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let names: Vec<String> =
        c.service.list_all_tools().await.unwrap().into_iter().map(|t| t.name.to_string()).collect();
    assert!(names.contains(&"delete_note".to_string()), "{names:?}");
}
