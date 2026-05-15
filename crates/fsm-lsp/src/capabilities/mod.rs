//! LSP capability handlers.
//!
//! L1 shipped [`diagnostics`] (`publishDiagnostics`). L2 adds
//! [`document_symbol`] (`textDocument/documentSymbol`, Doc 14 §13) and
//! [`folding`] (`textDocument/foldingRange`, Doc 14 §12) — both pure
//! projections of the *single* analysis L1 already runs (Doc 26 §8 L2:
//! "one analysis feeds both"): `document_symbol` consumes the threaded
//! `symbol_table`, `folding` walks the parse-tree CST. Neither re-analyses
//! and neither adds a second position converter (Doc 26 §3/§4.1).
//!
//! The remaining L3+ handlers (hover, definition, completion, references,
//! rename, semanticTokens, codeAction, inlayHint — Doc 26 §5) are out of
//! L2 scope (Doc 26 §8 L2 scope boundary) and are deliberately NOT
//! stubbed: an empty handler that silently returns nothing is worse than
//! an unadvertised capability (the client would think the feature works).

pub mod diagnostics;
pub mod document_symbol;
pub mod folding;
