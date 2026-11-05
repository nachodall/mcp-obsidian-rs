mod common;

use common::*;
use mcp_obsidian::server::Options;
use serde_json::json;

fn line_with<'a>(text: &'a str, needle: &str) -> Option<&'a str> {
    text.lines().find(|l| l.contains(needle))
}

#[tokio::test]
async fn h1_list_without_scope_returns_everything_visible() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let out = c.ok("list_notes", json!({})).await;
    assert!(out.contains("root.md"), "{out}");
    assert!(out.contains("projects"), "{out}");
    assert!(out.contains("alpha.md"), "{out}");
    assert!(out.contains("beta.md"), "{out}");
    let att = line_with(&out, "diagram.png").unwrap_or_else(|| panic!("no attachment: {out}"));
    assert!(att.to_lowercase().contains("attachment"), "attachment not marked: {att}");
    assert!(!line_with(&out, "alpha.md").unwrap().to_lowercase().contains("attachment"));
    for hidden in ["secret", "hidden-note", "leak.pdf", ".obsidian", "app.json", ".trash", "deleted.md"] {
        assert!(!out.contains(hidden), "{hidden} leaked: {out}");
    }
}

#[tokio::test]
async fn h2_list_with_scope_returns_only_that_folder() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let out = c.ok("list_notes", json!({"folder": "projects"})).await;
    assert!(out.contains("alpha.md"), "{out}");
    assert!(out.contains("diagram.png"), "{out}");
    assert!(!out.contains("root.md"), "{out}");
}

#[tokio::test]
async fn h3_hidden_scopes_give_same_error_as_missing_folder() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let norm = |msg: String, scope: &str| msg.replace(scope, "<SCOPE>");
    let missing = norm(c.err("list_notes", json!({"folder": "does-not-exist"})).await, "does-not-exist");
    assert!(!missing.is_empty());
    for scope in ["secret", ".obsidian", ".trash", "secret/sub", ".trash/old"] {
        let got = norm(c.err("list_notes", json!({"folder": scope})).await, scope);
        assert_eq!(got, missing, "scope {scope} gives a different error");
    }
}

#[tokio::test]
async fn h36_dotdot_and_absolute_paths_are_rejected() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let abs = v.outside.to_string_lossy().to_string();
    for scope in ["..", "../outside", "projects/../../outside", abs.as_str(), "/"] {
        let e = c.err("list_notes", json!({"folder": scope})).await;
        assert!(!e.contains("outside-file"), "leaked via {scope}: {e}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn h37_symlink_pointing_outside_is_rejected() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let e = c.err("list_notes", json!({"folder": "link"})).await;
    assert!(!e.contains("outside-file"), "{e}");
    let e2 = c.err("list_notes", json!({"folder": "link/sub"})).await;
    assert!(!e2.contains("outside-file"), "{e2}");
    let all = c.ok("list_notes", json!({})).await;
    assert!(!all.contains("outside-file"), "symlink target leaked: {all}");
}

#[tokio::test]
async fn h40_missing_config_means_no_exclusions_and_50kb_limit() {
    let v = TestVault::new();
    v.note("a.md", "# a").note("private/b.md", "# b");
    let (c, server) = try_start(&v.root, Options::default()).await.expect("must start");
    assert!(server.config().excluded_folders.is_empty());
    assert_eq!(server.config().max_response_bytes, 50 * 1024);
    let out = c.ok("list_notes", json!({})).await;
    assert!(out.contains("a.md") && out.contains("b.md"), "{out}");
}

#[tokio::test]
async fn h41_invalid_config_refuses_to_start_and_says_why() {
    let v = TestVault::new();
    v.note("a.md", "# a").config("excluded_folders = [\"unterminated\n");
    let err = try_start(&v.root, Options::default()).await.err().expect("must refuse to start");
    assert!(err.contains(".mcp-obsidian.toml"), "error must name the file: {err}");
    assert!(err.len() > ".mcp-obsidian.toml".len() + 10, "error must say why: {err}");
}
