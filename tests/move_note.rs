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

fn read(v: &TestVault, rel: &str) -> String {
    fs::read_to_string(v.root.join(rel)).unwrap()
}

#[tokio::test]
async fn h30_move_updates_every_link_form() {
    let v = TestVault::standard();
    v.note(
        "refs.md",
        "[[alpha]] [[projects/alpha]] [[alpha|al]] [[alpha#H]] ![[alpha]] [[projects/alpha.md]] [[projects/alpha#^b|x]]",
    );
    let c = start(&v.root).await;
    c.ok("move_note", json!({"from": "projects/alpha.md", "to": "archive/alpha.md"})).await;
    assert!(!v.root.join("projects/alpha.md").exists());
    assert_eq!(read(&v, "archive/alpha.md"), "# alpha");
    assert_eq!(
        read(&v, "refs.md"),
        "[[alpha]] [[archive/alpha]] [[alpha|al]] [[alpha#H]] ![[alpha]] [[archive/alpha.md]] [[archive/alpha#^b|x]]"
    );
}

#[tokio::test]
async fn h30_bare_name_becomes_path_when_no_longer_unique() {
    let v = TestVault::standard();
    v.note("m/alpha.md", "# moved").note("p/q/alpha.md", "# other").note("refs.md", "[[alpha]] [[p/q/alpha]]");
    let c = start(&v.root).await;
    c.ok("move_note", json!({"from": "m/alpha.md", "to": "p/q/r/alpha.md"})).await;
    assert_eq!(read(&v, "refs.md"), "[[p/q/r/alpha]] [[p/q/alpha]]");
    assert_eq!(read(&v, "p/q/alpha.md"), "# other");
}

#[tokio::test]
async fn h31_rename_updates_names_keeps_alias_and_heading() {
    let v = TestVault::standard();
    v.note("refs.md", "[[alpha|A]] [[projects/alpha#Head]] ![[alpha]] [[alpha#^blk|B]]");
    let c = start(&v.root).await;
    c.ok("move_note", json!({"from": "projects/alpha.md", "to": "projects/gamma.md"})).await;
    assert_eq!(read(&v, "refs.md"), "[[gamma|A]] [[projects/gamma#Head]] ![[gamma]] [[gamma#^blk|B]]");
}

#[tokio::test]
async fn move_ignores_links_in_code_and_other_targets() {
    let v = TestVault::standard();
    v.note("refs.md", "`[[alpha]]`\n```\n[[alpha]]\n```\n[[beta]] [[alpha]]");
    let c = start(&v.root).await;
    c.ok("move_note", json!({"from": "projects/alpha.md", "to": "projects/gamma.md"})).await;
    assert_eq!(read(&v, "refs.md"), "`[[alpha]]`\n```\n[[alpha]]\n```\n[[beta]] [[gamma]]");
}

#[tokio::test]
async fn move_updates_self_links_in_moved_note() {
    let v = TestVault::standard();
    v.note("projects/alpha.md", "see [[alpha#Top]]");
    let c = start(&v.root).await;
    c.ok("move_note", json!({"from": "projects/alpha.md", "to": "projects/gamma.md"})).await;
    assert_eq!(read(&v, "projects/gamma.md"), "see [[gamma#Top]]");
}

#[tokio::test]
async fn d27_move_attachment_updates_embeds() {
    let v = TestVault::standard();
    v.note("refs.md", "![[projects/diagram.png]] ![[diagram.png|200]]");
    let c = start(&v.root).await;
    c.ok("move_note", json!({"from": "projects/diagram.png", "to": "assets/diagram.png"})).await;
    assert!(v.root.join("assets/diagram.png").exists());
    assert_eq!(read(&v, "refs.md"), "![[assets/diagram.png]] ![[diagram.png|200]]");
}

#[tokio::test]
async fn h32_excluded_links_updated_and_only_counted() {
    let v = TestVault::standard();
    v.note("secret/hidden-note.md", "[[alpha]] and [[projects/alpha|a]]");
    let c = start(&v.root).await;
    let out = c.ok("move_note", json!({"from": "projects/alpha.md", "to": "archive/alpha.md"})).await;
    assert_eq!(read(&v, "secret/hidden-note.md"), "[[alpha]] and [[archive/alpha|a]]");
    assert!(!out.contains("secret") && !out.contains("hidden-note"), "{out}");
    assert!(out.contains("1 note in excluded folders"), "{out}");
}

#[tokio::test]
async fn h34_move_onto_existing_path_fails_and_changes_nothing() {
    let v = TestVault::standard();
    v.note("refs.md", "[[alpha]]");
    let before = files_under(&v.root);
    let c = start(&v.root).await;
    let err = c.err("move_note", json!({"from": "projects/alpha.md", "to": "root.md"})).await;
    assert!(err.contains("already exists"), "{err}");
    assert_eq!(files_under(&v.root), before);
}

#[tokio::test]
async fn h38_moves_into_or_out_of_hidden_fail_as_not_found() {
    let v = TestVault::standard();
    v.note("refs.md", "[[alpha]]");
    let before = files_under(&v.root);
    let c = start(&v.root).await;
    let cases = [
        ("projects/alpha.md", "secret/alpha.md"),
        ("projects/alpha.md", ".obsidian/alpha.md"),
        ("projects/alpha.md", ".trash/alpha.md"),
        ("secret/hidden-note.md", "out.md"),
        (".trash/deleted.md", "out.md"),
        ("projects/missing.md", "out.md"),
    ];
    for (from, to) in cases {
        let err = c.err("move_note", json!({"from": from, "to": to})).await;
        assert!(err.contains("not found"), "{from} -> {to}: {err}");
    }
    assert_eq!(files_under(&v.root), before);
}

#[tokio::test]
async fn move_updates_search_index() {
    let v = TestVault::standard();
    v.note("projects/alpha.md", "zebracorn content");
    v.note("refs.md", "[[alpha]] quokkaword");
    let c = start(&v.root).await;
    c.ok("move_note", json!({"from": "projects/alpha.md", "to": "archive/gamma.md"})).await;
    let hits = c.ok("search_notes", json!({"query": "zebracorn"})).await;
    assert!(hits.contains("archive/gamma.md") && !hits.contains("projects/alpha.md"), "{hits}");
    let hits = c.ok("search_notes", json!({"query": "quokkaword"})).await;
    assert!(hits.contains("[[gamma]]"), "{hits}");
}

#[tokio::test]
async fn read_only_hides_move_note() {
    let v = TestVault::standard();
    let (c, _) = try_start(&v.root, Options { read_only: true }).await.unwrap();
    let names: Vec<String> =
        c.service.list_all_tools().await.unwrap().into_iter().map(|t| t.name.to_string()).collect();
    assert!(!names.contains(&"move_note".to_string()), "{names:?}");
    c.err("move_note", json!({"from": "root.md", "to": "x.md"})).await;
    assert!(v.root.join("root.md").exists());
}
