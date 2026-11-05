mod common;

use common::*;
use serde_json::json;

fn fixture() -> TestVault {
    let v = TestVault::standard();
    v.note("notes/analisis.md", "# Titulo\nEl Análisis del mercado es largo.\nOtra linea sin relacion.\n")
        .note("notes/phrase-a.md", "the quick brown fox jumps\nnothing here\n")
        .note("notes/phrase-b.md", "brown quick fox is different\n")
        .note("notes/typo.md", "A note about kubernetes clusters.\n")
        .note("projects/zeta.md", "zebra stripes in projects\n")
        .note("other/zeta.md", "zebra stripes in other\n")
        .note("secret/s.md", "uniquesecretword in excluded\n")
        .note(".trash/t.md", "uniquetrashword in trash\n")
        .file(".obsidian/o.md", "uniqueobsidianword in config\n");
    v
}

#[tokio::test]
async fn h10_results_have_path_and_matching_lines_only() {
    let v = fixture();
    let c = start(&v.root).await;
    let out = c.ok("search_notes", json!({"query": "mercado"})).await;
    assert!(out.contains("notes/analisis.md"), "{out}");
    assert!(out.contains("El Análisis del mercado es largo."), "{out}");
    assert!(!out.contains("Otra linea sin relacion"), "whole note leaked: {out}");
    assert!(!out.contains("# Titulo"), "whole note leaked: {out}");
}

#[tokio::test]
async fn h10_results_are_ranked_by_relevance() {
    let v = TestVault::new();
    v.note("low.md", "fox and a lot of other filler words here to dilute the score of this note entirely\n")
        .note("high.md", "fox fox fox\n");
    let c = start(&v.root).await;
    let out = c.ok("search_notes", json!({"query": "fox"})).await;
    let hi = out.find("high.md").unwrap_or_else(|| panic!("{out}"));
    let lo = out.find("low.md").unwrap_or_else(|| panic!("{out}"));
    assert!(hi < lo, "not ranked: {out}");
}

#[tokio::test]
async fn h11_case_and_accent_insensitive() {
    let v = fixture();
    let c = start(&v.root).await;
    for q in ["analisis", "ANALISIS", "Análisis", "análisis"] {
        let out = c.ok("search_notes", json!({"query": q})).await;
        assert!(out.contains("notes/analisis.md"), "{q}: {out}");
    }
}

#[tokio::test]
async fn h12_quoted_phrase_matches_only_exact_phrase() {
    let v = fixture();
    let c = start(&v.root).await;
    let out = c.ok("search_notes", json!({"query": "\"quick brown fox\""})).await;
    assert!(out.contains("notes/phrase-a.md"), "{out}");
    assert!(!out.contains("notes/phrase-b.md"), "{out}");
    let words = c.ok("search_notes", json!({"query": "quick brown fox"})).await;
    assert!(words.contains("notes/phrase-b.md"), "{words}");
}

#[tokio::test]
async fn h13_fuzzy_finds_term_with_typo() {
    let v = fixture();
    let c = start(&v.root).await;
    let off = c.ok("search_notes", json!({"query": "kubernetse"})).await;
    assert!(!off.contains("notes/typo.md"), "{off}");
    let on = c.ok("search_notes", json!({"query": "kubernetse", "fuzzy": true})).await;
    assert!(on.contains("notes/typo.md"), "{on}");
    assert!(on.contains("kubernetes clusters"), "{on}");
}

#[tokio::test]
async fn h14_scope_limits_results_to_folder() {
    let v = fixture();
    let c = start(&v.root).await;
    let all = c.ok("search_notes", json!({"query": "zebra"})).await;
    assert!(all.contains("projects/zeta.md") && all.contains("other/zeta.md"), "{all}");
    let scoped = c.ok("search_notes", json!({"query": "zebra", "folder": "projects"})).await;
    assert!(scoped.contains("projects/zeta.md"), "{scoped}");
    assert!(!scoped.contains("other/zeta.md"), "{scoped}");
}

#[tokio::test]
async fn h15_hidden_folders_are_never_searched() {
    let v = fixture();
    let c = start(&v.root).await;
    for word in ["uniquesecretword", "uniquetrashword", "uniqueobsidianword"] {
        let out = c.ok("search_notes", json!({"query": word})).await;
        assert!(!out.contains(word) && !out.contains("s.md") && !out.contains("t.md"), "{word}: {out}");
    }
    for scope in ["secret", ".trash", ".obsidian"] {
        let e = c.err("search_notes", json!({"query": "uniquesecretword", "folder": scope})).await;
        let missing = c.err("search_notes", json!({"query": "uniquesecretword", "folder": "nope"})).await;
        assert_eq!(e.replace(scope, "X"), missing.replace("nope", "X"));
    }
}

fn snapshot(root: &std::path::Path) -> Vec<String> {
    fn go(dir: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            out.push(p.display().to_string());
            if p.is_dir() {
                go(&p, out);
            }
        }
    }
    let mut out = Vec::new();
    go(root, &mut out);
    out.sort();
    out
}

#[tokio::test]
async fn h18_building_the_index_creates_no_file_in_the_vault() {
    let v = fixture();
    let before = snapshot(&v.root);
    let c = start(&v.root).await;
    c.ok("search_notes", json!({"query": "mercado"})).await;
    assert_eq!(before, snapshot(&v.root));
}

#[tokio::test]
async fn index_upsert_and_remove_keep_it_in_sync() {
    use mcp_obsidian::index::SearchIndex;
    use mcp_obsidian::vault::Vault;
    use std::sync::Arc;
    let v = TestVault::new();
    v.note("a.md", "alphaword here\n");
    let cfg = mcp_obsidian::config::Config::load(&v.root).unwrap();
    let vault = Arc::new(Vault::new(&v.root, cfg).unwrap());
    let idx = SearchIndex::build(vault).unwrap();
    assert_eq!(idx.search("alphaword", false, None).unwrap().len(), 1);
    v.note("b.md", "betaword here\n");
    idx.upsert("b.md").unwrap();
    assert_eq!(idx.search("betaword", false, None).unwrap().len(), 1);
    v.note("a.md", "gammaword now\n");
    idx.upsert("a.md").unwrap();
    assert!(idx.search("alphaword", false, None).unwrap().is_empty());
    assert_eq!(idx.search("gammaword", false, None).unwrap().len(), 1);
    idx.remove("b.md").unwrap();
    assert!(idx.search("betaword", false, None).unwrap().is_empty());
}

#[tokio::test]
async fn results_respect_the_size_limit() {
    let v = TestVault::new();
    v.config("max_response_bytes = 300\n");
    for i in 0..40 {
        v.note(&format!("n{i:02}.md"), "needle in a haystack line that is fairly long to fill space\n");
    }
    let c = start(&v.root).await;
    let out = c.ok("search_notes", json!({"query": "needle"})).await;
    assert!(out.contains("truncated"), "{out}");
    assert!(out.len() < 300 + 200, "{} bytes", out.len());
}
