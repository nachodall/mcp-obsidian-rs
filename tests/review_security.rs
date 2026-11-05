mod common;

use std::fs;

use common::fs::{files_with_content, read};
use common::*;
use mcp_obsidian::server::Options;
use serde_json::{Value, json};

// ---- D38: note tools accept only .md ----

#[tokio::test]
async fn edit_note_refuses_attachments_and_config_file() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let before = files_with_content(&v.root);
    let cfg = ".mcp-obsidian.toml";
    for path in ["projects/diagram.png", cfg] {
        for args in [
            json!({"path": path, "old_text": "binary", "new_text": "x"}),
            json!({"path": path, "append": "x"}),
            json!({"path": path, "content": "x", "overwrite": true}),
        ] {
            c.err("edit_note", args).await;
        }
    }
    assert_eq!(files_with_content(&v.root), before);
}

#[tokio::test]
async fn note_tools_refuse_non_md_paths() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let before = files_with_content(&v.root);
    let e = c.err("create_note", json!({"path": "new.txt", "content": "x"})).await;
    assert!(e.contains("only notes can be"), "{e}");
    c.err("create_note", json!({"path": "sub/new.png", "content": "x"})).await;
    for tool in ["add_tag", "remove_tag"] {
        let e = c.err(tool, json!({"path": "projects/diagram.png", "tag": "x"})).await;
        assert!(e.contains("only notes can be"), "{e}");
    }
    let e = c.err("get_tags", json!({"path": "projects/diagram.png"})).await;
    assert!(e.contains("only notes can be"), "{e}");
    let e = c.err("delete_note", json!({"path": "projects/diagram.png"})).await;
    assert!(e.contains("only notes can be"), "{e}");
    assert_eq!(files_with_content(&v.root), before);
}

// ---- D39: the config file is hidden ----

#[tokio::test]
async fn config_file_is_hidden_from_every_tool() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let before = files_with_content(&v.root);
    let list = c.ok("list_notes", json!({})).await;
    assert!(!list.contains("mcp-obsidian.toml"), "{list}");
    c.err("read_note", json!({"path": ".mcp-obsidian.toml"})).await;
    c.err("move_note", json!({"from": ".mcp-obsidian.toml", "to": "cfg.md"})).await;
    c.err("move_note", json!({"from": "root.md", "to": ".mcp-obsidian.toml"})).await;
    c.err("create_note", json!({"path": ".mcp-obsidian.toml", "content": "x"})).await;
    c.err("edit_note", json!({"path": ".mcp-obsidian.toml", "append": "x"})).await;
    c.err("move_folder", json!({"from": "projects", "to": ".mcp-obsidian.toml"})).await;
    assert_eq!(files_with_content(&v.root), before);
}

// ---- D40: excluded_folders normalization ----

async fn hidden_with_config(entry: &str) -> (TestVault, String) {
    let v = TestVault::new();
    v.note("secret/s.md", "# s").note("open.md", "# o").config(&format!("excluded_folders = [\"{entry}\"]\n"));
    let c = start(&v.root).await;
    let list = c.ok("list_notes", json!({})).await;
    (v, list)
}

#[tokio::test]
async fn excluded_folder_matches_case_insensitively() {
    let (_v, list) = hidden_with_config("Secret").await;
    assert!(list.contains("open.md") && !list.contains("secret"), "{list}");
}

#[tokio::test]
async fn excluded_folder_accepts_dot_slash_and_trailing_slash() {
    let (_v, list) = hidden_with_config("./secret/").await;
    assert!(list.contains("open.md") && !list.contains("secret"), "{list}");
}

#[tokio::test]
async fn invalid_excluded_folder_entries_stop_the_server() {
    for entry in ["", "../x", "a/../b", "/", "./", "/secret"] {
        let v = TestVault::new();
        v.note("a.md", "x").config(&format!("excluded_folders = [\"{entry}\"]\n"));
        let e = try_start(&v.root, Options::default()).await.err().unwrap_or_else(|| panic!("{entry:?} should refuse"));
        assert!(e.contains(".mcp-obsidian.toml"), "{entry:?}: {e}");
    }
}

#[tokio::test]
async fn absolute_excluded_folder_stops_the_server() {
    let v = TestVault::new();
    let abs = v.outside.display().to_string();
    v.note("a.md", "x").config(&format!("excluded_folders = [{abs:?}]\n"));
    let e = try_start(&v.root, Options::default()).await.err().expect("should refuse");
    assert!(e.contains(".mcp-obsidian.toml"), "{e}");
}

// ---- D41: trash symlink, permissions, temp files ----

#[cfg(unix)]
#[tokio::test]
async fn delete_fails_when_trash_is_a_symlink_outside() {
    let v = TestVault::new();
    v.note("a.md", "# a");
    std::os::unix::fs::symlink(&v.outside, v.root.join(".trash")).unwrap();
    let c = start(&v.root).await;
    c.err("delete_note", json!({"path": "a.md"})).await;
    assert_eq!(read(&v, "a.md"), "# a");
    assert_eq!(fs::read_dir(&v.outside).unwrap().count(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn edit_keeps_file_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let v = TestVault::new();
    v.note("a.md", "# a");
    fs::set_permissions(v.root.join("a.md"), fs::Permissions::from_mode(0o640)).unwrap();
    let c = start(&v.root).await;
    c.ok("edit_note", json!({"path": "a.md", "append": "more"})).await;
    let mode = fs::metadata(v.root.join("a.md")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640);
}

#[tokio::test]
async fn temp_files_are_hidden() {
    let v = TestVault::new();
    v.note("a.md", "# a").file(".mcp-tmp-1-0", "zebratmp").file("sub/.mcp-tmp-2-0", "zebratmp");
    let c = start(&v.root).await;
    let list = c.ok("list_notes", json!({})).await;
    assert!(list.contains("a.md") && !list.contains("mcp-tmp"), "{list}");
}

// ---- D42: size limit at the boundary; visible counts ----

#[tokio::test]
async fn move_note_output_goes_through_the_size_limit() {
    let v = TestVault::new();
    v.config("max_response_bytes = 80\n").note("a/target.md", "# t");
    for i in 0..30 {
        v.note(&format!("refs/note-number-{i:02}.md"), "[[a/target]]");
    }
    let c = start(&v.root).await;
    let out = c.ok("move_note", json!({"from": "a/target.md", "to": "b/target.md"})).await;
    assert!(out.contains("[truncated"), "{out}");
    assert!(out.len() < 250, "{} bytes", out.len());
}

#[tokio::test]
async fn move_folder_output_goes_through_the_size_limit() {
    let v = TestVault::new();
    v.config("max_response_bytes = 80\n").note("dir/target.md", "# t");
    for i in 0..30 {
        v.note(&format!("refs/note-number-{i:02}.md"), "[[dir/target]]");
    }
    let c = start(&v.root).await;
    let out = c.ok("move_folder", json!({"from": "dir", "to": "moved"})).await;
    assert!(out.contains("[truncated"), "{out}");
}

#[tokio::test]
async fn get_tags_output_goes_through_the_size_limit() {
    let v = TestVault::new();
    v.config("max_response_bytes = 60\n");
    let tags: String = (0..40).map(|i| format!("#tagnumber{i} ")).collect();
    v.note("a.md", &tags);
    let c = start(&v.root).await;
    let out = c.ok("get_tags", json!({"path": "a.md"})).await;
    assert!(out.contains("[truncated"), "{out}");
}

#[tokio::test]
async fn error_messages_go_through_the_size_limit() {
    let v = TestVault::new();
    v.config("max_response_bytes = 40\n").note("a.md", "x");
    let c = start(&v.root).await;
    let long = "x".repeat(500);
    let e = c.err("read_note", json!({"path": long})).await;
    assert!(e.contains("[truncated"), "{e}");
    assert!(e.len() < 200, "{} bytes", e.len());
}

#[tokio::test]
async fn move_folder_counts_only_visible_files() {
    let v = TestVault::new();
    v.note("dir/a.md", "# a").note("dir/b.md", "# b").note("dir/priv/c.md", "# c").note("dir/priv/d.md", "# d");
    v.config("excluded_folders = [\"dir/priv\"]\n");
    let c = start(&v.root).await;
    let out = c.ok("move_folder", json!({"from": "dir", "to": "moved"})).await;
    assert!(out.contains("2 file(s) moved"), "{out}");
}

// ---- Missing tests: path escapes at every tool ----

fn escapes(v: &TestVault) -> Vec<String> {
    vec![
        "../outside/outside-file.md".into(),
        v.outside.join("outside-file.md").display().to_string(),
        "link/outside-file.md".into(),
    ]
}

#[tokio::test]
async fn escaping_paths_are_rejected_everywhere() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let vault_before = files_with_content(&v.root);
    let outside_before = files_with_content(&v.outside);
    for bad in escapes(&v) {
        let calls: Vec<(&str, Value)> = vec![
            ("move_note", json!({"from": bad, "to": "stolen.md"})),
            ("move_note", json!({"from": "root.md", "to": bad})),
            ("move_folder", json!({"from": bad, "to": "stolen"})),
            ("move_folder", json!({"from": "projects", "to": bad})),
            ("add_tag", json!({"path": bad, "tag": "x"})),
            ("remove_tag", json!({"path": bad, "tag": "x"})),
            ("get_tags", json!({"path": bad})),
            ("get_backlinks", json!({"path": bad})),
            ("delete_note", json!({"path": bad})),
        ];
        for (tool, args) in calls {
            c.err(tool, args.clone()).await;
            let _ = &args;
        }
    }
    // Folder escapes through the symlinked folder and `..`.
    for bad in ["link", "../outside", "link/sub"] {
        c.err("move_folder", json!({"from": bad, "to": "stolen"})).await;
        c.err("move_folder", json!({"from": "projects", "to": format!("{bad}/x")})).await;
    }
    assert_eq!(files_with_content(&v.root), vault_before);
    assert_eq!(files_with_content(&v.outside), outside_before);
}

#[tokio::test]
async fn read_only_hides_tag_writes_and_move_folder() {
    let v = TestVault::standard();
    let (c, _) = try_start(&v.root, Options { read_only: true }).await.unwrap();
    let names: Vec<String> =
        c.service.list_all_tools().await.unwrap().into_iter().map(|t| t.name.to_string()).collect();
    for tool in ["add_tag", "remove_tag", "move_folder"] {
        assert!(!names.contains(&tool.to_string()), "{tool} listed: {names:?}");
    }
    c.err("add_tag", json!({"path": "root.md", "tag": "x"})).await;
    c.err("remove_tag", json!({"path": "root.md", "tag": "x"})).await;
    c.err("move_folder", json!({"from": "projects", "to": "x"})).await;
    assert_eq!(read(&v, "root.md"), "# root");
    assert!(v.root.join("projects").exists());
}

// ---- Finding 9: non-UTF-8 note ----

#[tokio::test]
async fn edit_note_on_invalid_utf8_reports_read_error() {
    let v = TestVault::new();
    fs::write(v.root.join("bad.md"), [0xff, 0xfe, 0x41]).unwrap();
    let c = start(&v.root).await;
    let e = c.err("edit_note", json!({"path": "bad.md", "append": "x"})).await;
    assert!(e.contains("read failed"), "{e}");
    assert_eq!(fs::read(v.root.join("bad.md")).unwrap(), vec![0xff, 0xfe, 0x41]);
}
