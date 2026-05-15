//! LSP capability handlers.
//!
//! L1 shipped [`diagnostics`] (`publishDiagnostics`). L2 added
//! [`document_symbol`] (`textDocument/documentSymbol`, Doc 14 §13) and
//! [`folding`] (`textDocument/foldingRange`, Doc 14 §12). L3 added
//! [`hover`] (`textDocument/hover`, Doc 14 §5) and [`definition`]
//! (`textDocument/definition`, Doc 14 §6, single-file), both on the shared
//! [`resolve`] token-at-cursor seam. L4 adds [`complete`]
//! (`textDocument/completion`, Doc 14 §4) — a context-aware completion
//! whose trigger-context classifier **reuses L3's [`resolve`] CST
//! substrate** (the SAME `enclosing` ancestry walk + `in_guard` predicate
//! + a shared `prev_significant_token` primitive), NOT a parallel ad-hoc
//! detector (Doc 26 §8 L4). Every handler is a pure projection of the
//! *single* analysis L1 already runs (Doc 26 §8 L2/L3/L4: "one analysis
//! feeds all"): `document_symbol`/`hover`/`definition`/`complete` consume
//! the threaded `symbol_table` (+ for hover, the additively threaded
//! `ir`), `folding` walks the parse-tree CST. No handler re-analyses,
//! none adds a second position converter, and `resolve` mirrors the
//! analyzer's own name-resolution dispatch so a goto/hover/completion can
//! never disagree with a diagnostic (Doc 26 §3/§4.1/§5).
//!
//! L5 adds [`references`] (`textDocument/references`, Doc 14 §7) and
//! [`rename`] (`textDocument/prepareRename` + `textDocument/rename`, Doc
//! 14 §8), both pure projections of the L5 [`crate::refs::ReferenceIndex`]
//! — the ONE genuinely-new analysis, itself *derived* from the single
//! `analyze()` (no second pass) and **semantic-only** (every reference is
//! proven by the SAME L3 [`resolve`] classifier + `SymbolTable::resolve_*`,
//! never a text match). They reuse the [`resolve`] seam, not a parallel
//! resolver; their ranges go through L1's `LineIndex` (no second position
//! converter). `rename` is conservative-by-construction per Doc 26 risk-2:
//! a same-spelled string/comment/different-scope token can never be in the
//! `WorkspaceEdit` because it never entered the index.
//!
//! L6 adds [`semantic_tokens`] (`textDocument/semanticTokens/full` +
//! `/range`, Doc 14 §10) — semantic tokens are *more precise* than the Doc
//! 21 TextMate grammar (a bare `Ident` is one TextMate scope everywhere;
//! semantic tokens know, from the one analysis, whether it is a state /
//! event / extern / context field / machine and decl-vs-ref). It is **not**
//! a new analysis or a parallel classifier: the decl-vs-ref + entity-type
//! split **reuses** L3's [`resolve`] (use sites) and L5's
//! [`crate::refs::collect_decl_name_tokens`] / [`crate::refs::SymbolKey`]
//! taxonomy (declaration sites) — one classifier, one `symbol_table`
//! identity model (Doc 26 §8 L6). The LSP relative delta encoding measures
//! `deltaStartChar`/`length` in the negotiated `positionEncoding` via L1's
//! one authoritative `LineIndex` (no second converter).
//!
//! The remaining L7 handlers (codeAction, inlayHint — Doc 26 §5/§8) are out
//! of L6 scope (Doc 26 §8 L6 scope boundary) and are deliberately NOT
//! stubbed: an empty handler that silently returns nothing is worse than an
//! unadvertised capability (the client would think the feature works) — the
//! `workspaceSymbol`-left-unadvertised precedent (Doc 00 §11.33(5)).

pub mod complete;
pub mod definition;
pub mod diagnostics;
pub mod document_symbol;
pub mod folding;
pub mod hover;
pub mod references;
pub mod rename;
pub mod resolve;
pub mod semantic_tokens;
