use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Context, anyhow};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{
    Field, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions, Value,
};
use tantivy::tokenizer::{AsciiFoldingFilter, LowerCaser, SimpleTokenizer, TextAnalyzer};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

use crate::vault::{Vault, join_rel};

const TOKENIZER: &str = "fold";
const WRITER_HEAP_BYTES: usize = 50_000_000;

/// One search hit: vault-relative path and the matching lines only.
#[derive(Debug, Clone)]
pub struct Hit {
    pub path: String,
    /// `(line number starting at 1, line text)`.
    pub lines: Vec<(usize, String)>,
}

/// In-memory full-text index of the visible notes (never written to disk).
/// Cheap to share: wrap it in an `Arc`. All methods take `&self`.
pub struct SearchIndex {
    vault: Arc<Vault>,
    index: Index,
    reader: IndexReader,
    writer: Mutex<IndexWriter>,
    path: Field,
    body: Field,
    /// Frontmatter and inline tags of each note.
    tags: Field,
}

impl fmt::Debug for SearchIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SearchIndex").finish_non_exhaustive()
    }
}

fn analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(LowerCaser)
        .filter(AsciiFoldingFilter)
        .build()
}

fn tokens(text: &str) -> Vec<String> {
    let mut analyzer = analyzer();
    let mut stream = analyzer.token_stream(text);
    let mut out = Vec::new();
    while stream.advance() {
        out.push(stream.token().text.clone());
    }
    out
}

/// Optimal string alignment distance, true when it is at most 1.
fn within_one_edit(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let mut i = 0;
    while i < a.len() && i < b.len() && a[i] == b[i] {
        i += 1;
    }
    let (ra, rb) = (&a[i..], &b[i..]);
    match (ra.len(), rb.len()) {
        (0, 0) => true,
        (_, 0) | (0, _) => ra.len() + rb.len() == 1,
        (x, y) if x == y => ra[1..] == rb[1..] || (x >= 2 && ra[0] == rb[1] && ra[1] == rb[0] && ra[2..] == rb[2..]),
        (_, _) => ra[1..] == rb[..] || ra[..] == rb[1..],
    }
}

impl SearchIndex {
    /// Builds the index from every visible note of the vault.
    pub fn build(vault: Arc<Vault>) -> anyhow::Result<SearchIndex> {
        let mut builder = Schema::builder();
        let path = builder.add_text_field("path", STRING | STORED);
        let text_options = |stored: bool| {
            let indexing = TextFieldIndexing::default()
                .set_tokenizer(TOKENIZER)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions);
            let opts = TextOptions::default().set_indexing_options(indexing);
            if stored { opts.set_stored() } else { opts }
        };
        let body = builder.add_text_field("body", text_options(true));
        let tags = builder.add_text_field("tags", text_options(false));
        let index = Index::create_in_ram(builder.build());
        index.tokenizers().register(TOKENIZER, analyzer());
        let writer: IndexWriter = index.writer_with_num_threads(1, WRITER_HEAP_BYTES)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        let this = SearchIndex {
            vault,
            index,
            reader,
            writer: Mutex::new(writer),
            path,
            body,
            tags,
        };
        {
            let mut writer = this.writer()?;
            for entry in this.vault.walk(this.vault.root(), false) {
                if entry.is_dir || entry.abs.extension().is_none_or(|e| e != "md") {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&entry.abs) {
                    writer.add_document(this.document(&entry.rel, &text))?;
                }
            }
            writer.commit()?;
        }
        this.reader.reload()?;
        Ok(this)
    }

    fn document(&self, rel: &str, text: &str) -> TantivyDocument {
        let mut doc = TantivyDocument::default();
        doc.add_text(self.path, rel);
        doc.add_text(self.body, text);
        for tag in crate::frontmatter::all_tags(text) {
            doc.add_text(self.tags, tag);
        }
        doc
    }

    /// Locks the single index writer.
    fn writer(&self) -> anyhow::Result<MutexGuard<'_, IndexWriter>> {
        self.writer.lock().map_err(|_| anyhow!("index lock poisoned"))
    }

    /// Re-indexes one vault-relative note (adds or replaces it). A note that is
    /// missing, hidden, excluded or not valid UTF-8 text is removed instead.
    pub fn upsert(&self, path: &str) -> anyhow::Result<()> {
        let rel = normalize(path);
        let text = self
            .vault
            .resolve(&rel)
            .ok()
            .filter(|p| p.is_file() && rel.ends_with(".md"))
            .and_then(|p| std::fs::read_to_string(p).ok());
        let mut writer = self.writer()?;
        writer.delete_term(Term::from_field_text(self.path, &rel));
        if let Some(text) = text {
            writer.add_document(self.document(&rel, &text))?;
        }
        writer.commit()?;
        drop(writer);
        self.reader.reload()?;
        Ok(())
    }

    /// Re-indexes many notes with ONE commit and reload. Same rules as `upsert`.
    pub fn update_many(&self, paths: &[String]) -> anyhow::Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut writer = self.writer()?;
        for path in paths {
            let rel = normalize(path);
            let text = self
                .vault
                .resolve(&rel)
                .ok()
                .filter(|p| p.is_file() && rel.ends_with(".md"))
                .and_then(|p| std::fs::read_to_string(p).ok());
            writer.delete_term(Term::from_field_text(self.path, &rel));
            if let Some(text) = text {
                writer.add_document(self.document(&rel, &text))?;
            }
        }
        writer.commit()?;
        drop(writer);
        self.reader.reload()?;
        Ok(())
    }

    /// Removes one vault-relative note from the index.
    pub fn remove(&self, path: &str) -> anyhow::Result<()> {
        let rel = normalize(path);
        let mut writer = self.writer()?;
        writer.delete_term(Term::from_field_text(self.path, &rel));
        writer.commit()?;
        drop(writer);
        self.reader.reload()?;
        Ok(())
    }

    /// Ranked (BM25) search. `folder` is a vault-relative scope (already validated).
    /// Hits carry only the matching lines, never the whole note.
    pub fn search(&self, query: &str, fuzzy: bool, folder: Option<&str>) -> anyhow::Result<Vec<Hit>> {
        let mut parser = QueryParser::for_index(&self.index, vec![self.body, self.tags]);
        if fuzzy {
            parser.set_field_fuzzy(self.body, false, 1, true);
        }
        let (parsed, _ignored) = parser.parse_query_lenient(query);
        let searcher = self.reader.searcher();
        let limit = (searcher.num_docs() as usize).max(1);
        let top = searcher
            .search(&parsed, &TopDocs::with_limit(limit).order_by_score())
            .context("search failed")?;
        let scope = folder.map(normalize).filter(|f| !f.is_empty()).map(|f| format!("{f}/"));
        let wanted: HashSet<String> = tokens(query).into_iter().collect();
        let mut hits = Vec::new();
        for (_score, addr) in top {
            let doc: TantivyDocument = searcher.doc(addr)?;
            let Some(path) = doc.get_first(self.path).and_then(|v| v.as_str()) else { continue };
            if scope.as_ref().is_some_and(|s| !path.starts_with(s.as_str())) {
                continue;
            }
            let text = doc.get_first(self.body).and_then(|v| v.as_str()).unwrap_or("");
            let lines = text
                .lines()
                .enumerate()
                .filter(|(_, line)| {
                    tokens(line)
                        .iter()
                        .any(|t| wanted.contains(t) || (fuzzy && wanted.iter().any(|w| within_one_edit(t, w))))
                })
                .map(|(i, line)| (i + 1, line.trim().to_string()))
                .collect();
            hits.push(Hit { path: path.to_string(), lines });
        }
        Ok(hits)
    }
}

fn normalize(path: &str) -> String {
    join_rel(Path::new(path.trim_matches('/')))
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect::<Vec<_>>()
        .join("/")
}
