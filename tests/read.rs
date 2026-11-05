mod common;

use common::{TestVault, start};
use serde_json::json;

fn numbered(n: usize) -> String {
    (1..=n).map(|i| format!("line {i:03}\n")).collect()
}

#[tokio::test]
async fn h4_reads_full_content() {
    let v = TestVault::standard();
    v.note("full.md", "# Title\n\nbody text\n");
    let c = start(&v.root).await;
    let out = c.ok("read_note", json!({"path": "full.md"})).await;
    assert!(out.contains("# Title") && out.contains("body text"), "{out}");
}

#[tokio::test]
async fn h5_offset_and_limit_return_range_and_total() {
    let v = TestVault::standard();
    v.note("long.md", &numbered(10));
    let c = start(&v.root).await;
    let out = c.ok("read_note", json!({"path": "long.md", "offset": 3, "limit": 2})).await;
    assert!(out.contains("line 003") && out.contains("line 004"), "{out}");
    assert!(!out.contains("line 002") && !out.contains("line 005"), "{out}");
    assert!(out.contains("10"), "total line count missing: {out}");
}

#[tokio::test]
async fn h6_excluded_note_same_error_as_missing() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let excluded = c.err("read_note", json!({"path": "secret/hidden-note.md"})).await;
    let missing = c.err("read_note", json!({"path": "nothing/there.md"})).await;
    assert!(excluded.contains("not found"), "{excluded}");
    assert_eq!(
        excluded.replace("secret/hidden-note.md", "<PATH>"),
        missing.replace("nothing/there.md", "<PATH>"),
        "excluded and missing notes must give the same error"
    );
    assert!(missing.contains("not found"), "{missing}");
}

#[tokio::test]
async fn h7_attachment_cannot_be_read() {
    let v = TestVault::standard();
    let c = start(&v.root).await;
    let e = c.err("read_note", json!({"path": "projects/diagram.png"})).await;
    assert!(e.contains("only notes can be read"), "{e}");
}

#[tokio::test]
async fn h8_over_limit_truncates_with_next_offset() {
    let v = TestVault::standard();
    v.note("big.md", &numbered(200)).config("max_response_bytes = 300\n");
    let c = start(&v.root).await;
    let out = c.ok("read_note", json!({"path": "big.md"})).await;
    assert!(out.contains("line 001"), "{out}");
    assert!(!out.contains("line 200"), "{out}");
    assert!(out.contains("truncated"), "{out}");
    let rest = out.split("offset=").nth(1).unwrap_or_else(|| panic!("no offset in notice: {out}"));
    let next: usize = rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap();
    let before = format!("line {:03}", next - 1);
    assert!(out.contains(&before), "line before next offset missing: {out}");
    let more = c.ok("read_note", json!({"path": "big.md", "offset": next})).await;
    assert!(more.contains(&format!("line {next:03}")), "{more}");
}

#[tokio::test]
async fn h9_configured_limit_is_respected() {
    let v = TestVault::standard();
    v.note("mid.md", &numbered(100));
    let c = start(&v.root).await;
    let out = c.ok("read_note", json!({"path": "mid.md"})).await;
    assert!(out.contains("line 100") && !out.contains("truncated"), "default limit should fit");

    v.config("max_response_bytes = 200\n");
    let c = start(&v.root).await;
    let out = c.ok("read_note", json!({"path": "mid.md"})).await;
    assert!(out.contains("truncated") && !out.contains("line 100"), "{out}");
}
