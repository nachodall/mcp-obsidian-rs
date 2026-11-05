mod common;

use common::*;
use serde_json::json;

fn lines(out: &str) -> Vec<&str> {
    out.lines().map(str::trim).filter(|l| !l.is_empty()).collect()
}

fn vault() -> TestVault {
    let v = TestVault::standard();
    v.note("target.md", "# target")
        .note("sub/target2.md", "# t2")
        .attachment("files/report.pdf");
    v
}

#[tokio::test]
async fn h21_every_wikilink_form_counts_as_a_backlink() {
    let v = vault();
    v.note("a-plain.md", "see [[target]]")
        .note("b-path.md", "see [[target.md]]")
        .note("c-alias.md", "see [[target|my alias]]")
        .note("d-heading.md", "see [[target#Some heading]]")
        .note("e-block.md", "see [[target#^abc123]]")
        .note("f-embed.md", "![[target]]")
        .note("g-folder.md", "see [[sub/target2]]")
        .note("h-nolink.md", "see [[other]] and target")
        .note("i-partial.md", "[[targetish]]");
    let c = start(&v.root).await;
    let out = c.ok("get_backlinks", json!({"path": "target.md"})).await;
    let got = lines(&out);
    for n in ["a-plain.md", "b-path.md", "c-alias.md", "d-heading.md", "e-block.md", "f-embed.md"] {
        assert!(got.contains(&n), "{n} missing: {out}");
    }
    for n in ["g-folder.md", "h-nolink.md", "i-partial.md", "target.md"] {
        assert!(!got.contains(&n), "{n} should not be listed: {out}");
    }
    let out2 = c.ok("get_backlinks", json!({"path": "sub/target2.md"})).await;
    assert!(lines(&out2).contains(&"g-folder.md"), "{out2}");
}

#[tokio::test]
async fn h21_links_to_attachments_are_backlinks() {
    let v = vault();
    v.note("emb.md", "![[report.pdf]]").note("emb2.md", "[[files/report.pdf|r]]");
    let c = start(&v.root).await;
    let out = c.ok("get_backlinks", json!({"path": "files/report.pdf"})).await;
    let got = lines(&out);
    assert!(got.contains(&"emb.md") && got.contains(&"emb2.md"), "{out}");
}

#[tokio::test]
async fn h21_links_resolve_by_shortest_unique_name_or_full_path() {
    let v = vault();
    // Two notes share a basename: bare name is ambiguous, full path is exact.
    v.note("x/dup.md", "# x").note("y/dup.md", "# y")
        .note("by-path.md", "[[y/dup]]")
        .note("by-name.md", "[[target]]");
    let c = start(&v.root).await;
    let out = c.ok("get_backlinks", json!({"path": "y/dup.md"})).await;
    assert!(lines(&out).contains(&"by-path.md"), "{out}");
    let out = c.ok("get_backlinks", json!({"path": "x/dup.md"})).await;
    assert!(!lines(&out).contains(&"by-path.md"), "{out}");
}

#[tokio::test]
async fn h21_links_in_code_are_ignored() {
    let v = vault();
    v.note("fenced.md", "```\n[[target]]\n```\n")
        .note("inline.md", "use `[[target]]` here")
        .note("tilde.md", "~~~md\n[[target]]\n~~~\n")
        .note("real.md", "```\ncode\n```\nthen [[target]]");
    let c = start(&v.root).await;
    let out = c.ok("get_backlinks", json!({"path": "target.md"})).await;
    let got = lines(&out);
    assert_eq!(got, vec!["real.md"], "{out}");
}

#[tokio::test]
async fn h21_excluded_and_hidden_notes_are_never_returned() {
    let v = vault();
    v.note("visible.md", "[[target]]")
        .note("secret/s.md", "[[target]]")
        .note(".trash/t.md", "[[target]]")
        .file(".obsidian/o.md", "[[target]]");
    let c = start(&v.root).await;
    let out = c.ok("get_backlinks", json!({"path": "target.md"})).await;
    assert_eq!(lines(&out), vec!["visible.md"], "{out}");
}

#[tokio::test]
async fn h21_backlinks_of_hidden_or_missing_note_is_an_error() {
    let v = vault();
    let c = start(&v.root).await;
    c.err("get_backlinks", json!({"path": "secret/hidden-note.md"})).await;
    c.err("get_backlinks", json!({"path": "nope.md"})).await;
    c.err("get_backlinks", json!({"path": "../outside/outside-file.md"})).await;
}
