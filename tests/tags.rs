mod common;

use std::fs;

use common::fs::{read};
use common::*;
use mcp_obsidian::server::Options;
use serde_json::json;

fn tag_vault() -> TestVault {
    let v = TestVault::standard();
    v.note("fm.md", "---\ntags:\n  - project\n---\n# Title\nbody\n")
        .note("inline.md", "# Title\nsome text #project here\n")
        .note("nested.md", "# Title\nstudy #subject/math today\n")
        .note("flow.md", "---\ntags: [project, other]\n---\nbody\n")
        .note("code.md", "# Title\n```\n#project\n```\nuse `#project` here\n")
        .note("plain.md", "# project\nnothing here\n")
        .note("secret/tagged.md", "---\ntags: [project]\n---\n#project\n");
    v
}

#[tokio::test]
async fn h16_search_by_tag_finds_frontmatter_and_inline() {
    let v = tag_vault();
    let c = start(&v.root).await;
    let out = c.ok("search_by_tag", json!({"tag": "project"})).await;
    for p in ["fm.md", "inline.md", "flow.md"] {
        assert!(out.contains(p), "missing {p}: {out}");
    }
    assert!(!out.contains("code.md"), "code is not a tag: {out}");
    assert!(!out.contains("plain.md"), "heading is not a tag: {out}");
    assert!(!out.contains("nested.md"), "{out}");
}

#[tokio::test]
async fn h16_search_by_nested_tag_and_hash_prefix() {
    let v = tag_vault();
    let c = start(&v.root).await;
    let out = c.ok("search_by_tag", json!({"tag": "#subject/math"})).await;
    assert!(out.contains("nested.md"), "{out}");
    assert!(!out.contains("fm.md"), "{out}");
}

#[tokio::test]
async fn h16_search_by_tag_never_returns_excluded_folders() {
    let v = tag_vault();
    let c = start(&v.root).await;
    let out = c.ok("search_by_tag", json!({"tag": "project"})).await;
    assert!(!out.contains("secret"), "{out}");
}

#[tokio::test]
async fn h19_get_tags_returns_frontmatter_and_inline() {
    let v = tag_vault();
    v.note("both.md", "---\ntags: [alpha, beta]\n---\n# H\ntext #gamma and #subject/math\n# not-a-tag\n");
    let c = start(&v.root).await;
    let out = c.ok("get_tags", json!({"path": "both.md"})).await;
    for t in ["alpha", "beta", "gamma", "subject/math"] {
        assert!(out.contains(t), "missing {t}: {out}");
    }
    assert!(!out.contains("not-a-tag"), "{out}");
}

#[tokio::test]
async fn h19_get_tags_on_excluded_note_fails() {
    let v = tag_vault();
    let c = start(&v.root).await;
    let err = c.err("get_tags", json!({"path": "secret/tagged.md"})).await;
    assert!(!err.contains("project"), "{err}");
}

#[tokio::test]
async fn h20_add_tag_to_block_list_keeps_everything_else() {
    let v = TestVault::standard();
    v.note(
        "n.md",
        "---\ntitle: \"Hello: world\"\ntags:\n  - one\naliases: [a, b]\nzeta: 1\n---\n# Body\n#inline stays\n",
    );
    let c = start(&v.root).await;
    c.ok("add_tag", json!({"path": "n.md", "tag": "two"})).await;
    assert_eq!(
        read(&v, "n.md"),
        "---\ntitle: \"Hello: world\"\ntags:\n  - one\n  - two\naliases: [a, b]\nzeta: 1\n---\n# Body\n#inline stays\n"
    );
}

#[tokio::test]
async fn h20_add_tag_to_flow_list() {
    let v = TestVault::standard();
    v.note("n.md", "---\ntags: [one]\nother: x\n---\nbody");
    let c = start(&v.root).await;
    c.ok("add_tag", json!({"path": "n.md", "tag": "#two"})).await;
    assert_eq!(read(&v, "n.md"), "---\ntags: [one, two]\nother: x\n---\nbody");
}

#[tokio::test]
async fn h20_add_tag_without_tags_field_adds_it() {
    let v = TestVault::standard();
    v.note("n.md", "---\ntitle: T\n---\nbody\n");
    let c = start(&v.root).await;
    c.ok("add_tag", json!({"path": "n.md", "tag": "new"})).await;
    let after = read(&v, "n.md");
    assert!(after.starts_with("---\ntitle: T\n"), "{after}");
    assert!(after.ends_with("---\nbody\n"), "{after}");
    assert!(after.contains("tags:") && after.contains("new"), "{after}");
}

#[tokio::test]
async fn h20_add_tag_to_note_without_frontmatter_creates_it() {
    let v = TestVault::standard();
    v.note("n.md", "# Title\nbody\n");
    let c = start(&v.root).await;
    c.ok("add_tag", json!({"path": "n.md", "tag": "new"})).await;
    let after = read(&v, "n.md");
    assert!(after.starts_with("---\n"), "{after}");
    assert!(after.ends_with("---\n# Title\nbody\n"), "{after}");
    let out = c.ok("get_tags", json!({"path": "n.md"})).await;
    assert!(out.contains("new"), "{out}");
}

#[tokio::test]
async fn h20_add_existing_tag_changes_nothing() {
    let v = TestVault::standard();
    let original = "---\ntags:\n  - one\n---\nbody\n";
    v.note("n.md", original);
    let c = start(&v.root).await;
    c.ok("add_tag", json!({"path": "n.md", "tag": "one"})).await;
    assert_eq!(read(&v, "n.md"), original);
}

#[tokio::test]
async fn h20_remove_tag_keeps_everything_else() {
    let v = TestVault::standard();
    v.note("n.md", "---\ntitle: T\ntags:\n  - one\n  - two\n  - three\nzeta: 1\n---\nbody #two\n");
    let c = start(&v.root).await;
    c.ok("remove_tag", json!({"path": "n.md", "tag": "two"})).await;
    assert_eq!(
        read(&v, "n.md"),
        "---\ntitle: T\ntags:\n  - one\n  - three\nzeta: 1\n---\nbody #two\n"
    );
}

#[tokio::test]
async fn h20_remove_tag_from_flow_list() {
    let v = TestVault::standard();
    v.note("n.md", "---\ntags: [one, two]\nx: 1\n---\nbody");
    let c = start(&v.root).await;
    c.ok("remove_tag", json!({"path": "n.md", "tag": "one"})).await;
    assert_eq!(read(&v, "n.md"), "---\ntags: [two]\nx: 1\n---\nbody");
}

#[tokio::test]
async fn tag_writes_update_the_index() {
    let v = TestVault::standard();
    v.note("n.md", "---\ntags: [one]\n---\nbody");
    let c = start(&v.root).await;
    c.ok("add_tag", json!({"path": "n.md", "tag": "fresh"})).await;
    assert!(c.ok("search_by_tag", json!({"tag": "fresh"})).await.contains("n.md"));
    c.ok("remove_tag", json!({"path": "n.md", "tag": "fresh"})).await;
    assert!(!c.ok("search_by_tag", json!({"tag": "fresh"})).await.contains("n.md"));
}

#[tokio::test]
async fn tag_write_leaves_no_temp_file_and_refuses_excluded() {
    let v = tag_vault();
    let c = start(&v.root).await;
    c.ok("add_tag", json!({"path": "fm.md", "tag": "x"})).await;
    let names: Vec<String> = fs::read_dir(&v.root)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.contains("tmp")), "{names:?}");
    let before = read(&v, "secret/tagged.md");
    c.err("add_tag", json!({"path": "secret/tagged.md", "tag": "x"})).await;
    assert_eq!(read(&v, "secret/tagged.md"), before);
}

#[tokio::test]
async fn read_only_hides_tag_writes_but_keeps_reads() {
    let v = tag_vault();
    let (c, _) = try_start(&v.root, Options { read_only: true }).await.unwrap();
    assert!(c.ok("get_tags", json!({"path": "fm.md"})).await.contains("project"));
    assert!(c.ok("search_by_tag", json!({"tag": "project"})).await.contains("fm.md"));
    let before = read(&v, "fm.md");
    c.err("add_tag", json!({"path": "fm.md", "tag": "x"})).await;
    c.err("remove_tag", json!({"path": "fm.md", "tag": "project"})).await;
    assert_eq!(read(&v, "fm.md"), before);
}
