//! LSP capability handlers.
//!
//! L1 ships exactly one: [`diagnostics`] (`publishDiagnostics`). The L2+
//! handlers (documentSymbol, hover, definition, completion, references,
//! rename, semanticTokens, foldingRange, inlayHint — Doc 26 §5) are out of
//! L1 scope (Doc 26 §8 L1 scope boundary) and are deliberately NOT
//! stubbed: an empty handler that silently returns nothing is worse than
//! an unadvertised capability (the client would think the feature works).

pub mod diagnostics;
