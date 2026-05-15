//! In-memory open-document buffers — Doc 26 §2.4 / §4.3.
//!
//! L1 stores only what `publishDiagnostics` needs: the current full text +
//! version + a precomputed [`LineIndex`]. The richer `AnalyzedDocument`
//! (parse tree, symbol table, IR, reference index — Doc 26 §2.4) is L2+
//! and is intentionally NOT built here (Doc 26 §8 L1 scope boundary). The
//! sync model is **full-buffer** (Doc 26 §4.3): `didChange` replaces the
//! whole text; re-parse/re-analyze happens on the debounced trigger, not
//! per keystroke.

use std::collections::HashMap;

use tower_lsp::lsp_types::Url;

use crate::position::LineIndex;

/// One open text document.
#[derive(Clone, Debug)]
pub struct Document {
    /// Authoritative buffer content while the document is open.
    pub text: String,
    /// LSP document version (monotonic per the client).
    pub version: i32,
    /// Precomputed line-start table for `text` (Doc 26 §4.1) — rebuilt on
    /// every content replace so it never drifts from `text`.
    pub line_index: LineIndex,
}

impl Document {
    fn new(text: String, version: i32) -> Self {
        let line_index = LineIndex::new(&text);
        Document {
            text,
            version,
            line_index,
        }
    }
}

/// Map of open documents keyed by URI. Single-threaded-logical (guarded by
/// the server's async mutex) — no interior locking here.
#[derive(Default, Debug)]
pub struct DocumentStore {
    docs: HashMap<Url, Document>,
}

impl DocumentStore {
    /// `textDocument/didOpen` — the client now owns this document; its
    /// content supersedes anything on disk until `didClose`.
    pub fn open(&mut self, uri: Url, text: String, version: i32) {
        self.docs.insert(uri, Document::new(text, version));
    }

    /// `textDocument/didChange` with **full** content sync (Doc 26 §4.3):
    /// the buffer is replaced wholesale and the line index rebuilt. No-op
    /// if the document is not open (a spec-conformant client never sends
    /// this before `didOpen`, but tolerate it rather than panic).
    pub fn replace(&mut self, uri: &Url, text: String, version: i32) {
        if let Some(doc) = self.docs.get_mut(uri) {
            doc.line_index = LineIndex::new(&text);
            doc.text = text;
            doc.version = version;
        }
    }

    /// `textDocument/didClose` — the client no longer owns this document;
    /// disk content (if any) becomes authoritative again. Returns whether
    /// a document was actually removed.
    pub fn close(&mut self, uri: &Url) -> bool {
        self.docs.remove(uri).is_some()
    }

    /// Read access to an open document.
    pub fn get(&self, uri: &Url) -> Option<&Document> {
        self.docs.get(uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri() -> Url {
        Url::parse("file:///tmp/m.fsm").unwrap()
    }

    #[test]
    fn open_then_replace_updates_text_and_version() {
        let mut s = DocumentStore::default();
        s.open(uri(), "a".into(), 1);
        assert_eq!(s.get(&uri()).unwrap().text, "a");
        assert_eq!(s.get(&uri()).unwrap().version, 1);
        s.replace(&uri(), "machine M {}".into(), 2);
        let d = s.get(&uri()).unwrap();
        assert_eq!(d.text, "machine M {}");
        assert_eq!(d.version, 2);
    }

    #[test]
    fn close_removes_document() {
        let mut s = DocumentStore::default();
        s.open(uri(), "a".into(), 1);
        assert!(s.close(&uri()));
        assert!(s.get(&uri()).is_none());
        assert!(!s.close(&uri()), "second close is a no-op");
    }

    #[test]
    fn replace_before_open_is_a_noop_not_a_panic() {
        let mut s = DocumentStore::default();
        s.replace(&uri(), "x".into(), 9);
        assert!(s.get(&uri()).is_none());
    }
}
