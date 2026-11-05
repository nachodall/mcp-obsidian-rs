mod common;


use common::fs::{files_with_content as files_under, read};
use common::*;
use mcp_obsidian::server::Options;
use serde_json::json;

#[tokio::test]
async fn h33_moves_every_file_and_updates_links_and_embeds() {
    let v = TestVault::standard();
    v.note("refs.md", "[[alpha]] [[projects/deep/beta|b]] ![[diagram.png]] ![[projects/diagram.png|200]] [[root]]");
    let c = start(&v.root).await;
    let out = c.ok("move_folder", json!({"from": "projects", "to": "archive/old"})).await;
    assert!(out.contains("projects") && out.contains("archive/old"), "{out}");
    assert!(!v.root.join("projects").exists());
    assert_eq!(read(&v, "archive/old/alpha.md"), "# alpha");
    assert_eq!(read(&v, "archive/old/deep/beta.md"), "# beta");
    assert!(v.root.join("archive/old/diagram.png").exists());
    assert_eq!(
        read(&v, "refs.md"),
        "[[alpha]] [[archive/old/deep/beta|b]] ![[diagram.png]] ![[archive/old/diagram.png|200]] [[root]]"
    );
}

#[tokio::test]
async fn h33_links_inside_moved_notes_and_excluded_notes() {
    let v = TestVault::standard();
    v.note("projects/alpha.md", "[[projects/deep/beta]] [[root]] ![[projects/diagram.png]]");
    v.note("secret/hidden-note.md", "[[projects/alpha]] [[beta]]");
    let c = start(&v.root).await;
    let out = c.ok("move_folder", json!({"from": "projects", "to": "work"})).await;
    assert_eq!(read(&v, "work/alpha.md"), "[[work/deep/beta]] [[root]] ![[work/diagram.png]]");
    assert_eq!(read(&v, "secret/hidden-note.md"), "[[work/alpha]] [[beta]]");
    assert!(!out.contains("secret") && !out.contains("hidden-note"), "{out}");
    assert!(out.contains("1 note in excluded folders"), "{out}");
}

#[tokio::test]
async fn h33_move_folder_updates_search_index() {
    let v = TestVault::standard();
    v.note("projects/alpha.md", "zebracorn content");
    let c = start(&v.root).await;
    c.ok("move_folder", json!({"from": "projects", "to": "work"})).await;
    let hits = c.ok("search_notes", json!({"query": "zebracorn"})).await;
    assert!(hits.contains("work/alpha.md") && !hits.contains("projects/alpha.md"), "{hits}");
}

#[tokio::test]
async fn move_folder_onto_existing_path_fails_and_changes_nothing() {
    let v = TestVault::standard();
    v.note("refs.md", "[[alpha]]").note("other/x.md", "x");
    let before = files_under(&v.root);
    let c = start(&v.root).await;
    let err = c.err("move_folder", json!({"from": "projects", "to": "other"})).await;
    assert!(err.contains("already exists"), "{err}");
    assert_eq!(files_under(&v.root), before);
}

#[tokio::test]
async fn move_folder_into_itself_or_hidden_fails_and_changes_nothing() {
    let v = TestVault::standard();
    v.note("refs.md", "[[alpha]]");
    let before = files_under(&v.root);
    let c = start(&v.root).await;
    let cases = [
        ("projects", "projects/sub"),
        ("projects", "secret/p"),
        ("projects", ".obsidian/p"),
        ("projects", ".trash/p"),
        ("secret", "out"),
        (".trash", "out"),
        ("missing", "out"),
    ];
    for (from, to) in cases {
        c.err("move_folder", json!({"from": from, "to": to})).await;
    }
    assert_eq!(files_under(&v.root), before);
}

#[tokio::test]
async fn read_only_hides_move_folder() {
    let v = TestVault::standard();
    let (c, _) = try_start(&v.root, Options { read_only: true }).await.unwrap();
    let names: Vec<String> =
        c.service.list_all_tools().await.unwrap().into_iter().map(|t| t.name.to_string()).collect();
    assert!(!names.contains(&"move_folder".to_string()), "{names:?}");
    c.err("move_folder", json!({"from": "projects", "to": "x"})).await;
    assert!(v.root.join("projects").exists());
}
