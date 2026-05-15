//! `fsm-lsp` — the FSM Studio Language Server (Doc 26).
//!
//! Wraps the **exact** `fsm check` analysis pipeline behind LSP. This
//! crate is a developer-host tool: it pulls `tokio`/`tower-lsp` (Doc 26
//! §2.1) and is *never* compiled into firmware, so the embedded G2
//! heap-free promise is unaffected.
//!
//! ## L1 scope (Doc 26 §8 L1 — the spine)
//!
//! `initialize` (with UTF-8/UTF-16 `positionEncoding` negotiation, Doc 26
//! §4.1) · `initialized` · `shutdown` · full-document sync
//! (`didOpen`/`didChange`/`didClose`) · ~200ms debounce ·
//! `textDocument/publishDiagnostics` via the reused pipeline · the one
//! correct byte-`Span` ↔ LSP-`Position` converter ([`position::LineIndex`]).
//!
//! ## L2 scope (Doc 26 §8 L2 — read capabilities, no new analysis)
//!
//! `textDocument/documentSymbol` (Doc 14 §13 hierarchical tree) +
//! `textDocument/foldingRange` (Doc 14 §12), advertised in `initialize`.
//! Both are pure projections of the **single** analysis the spine already
//! runs (Doc 26 §8 L2 "one analysis feeds both"): `documentSymbol`
//! consumes the `symbol_table` threaded through [`analysis::Analysis`];
//! `foldingRange` is a structural parse-tree walk. No second analysis
//! pass, no second position converter.
//!
//! ## L3 scope (Doc 26 §8 L3 — hover + single-file goto, no new analysis)
//!
//! `textDocument/hover` (Doc 14 §5 Markdown) + `textDocument/definition`
//! (Doc 14 §6, **single-file** — cross-file is v1.3, Doc 26 §4.6),
//! advertised in `initialize`. Both project the shared
//! [`capabilities::resolve`] token-at-cursor seam, whose resolution
//! mirrors the analyzer's own name-resolution dispatch and goes through
//! the SAME `SymbolTable::resolve_*` `fsm check` uses (so a goto/hover can
//! never disagree with a squiggle). Hover's structured detail (payload
//! types, extern signatures, transition counts) comes from the **`ir`**
//! field threaded **additively** through [`analysis::Analysis`] — the
//! exact behaviour-neutral pattern L2 used for `symbol_table`; still one
//! analysis, no second lowering, no second position converter.
//!
//! L4+ capabilities (completion, references, rename, semanticTokens,
//! codeAction, inlayHint) are out of L3 scope and are NOT stubbed (a
//! silent no-op handler is worse than an unadvertised capability).
//!
//! ## The reuse seam
//!
//! [`analysis::analyze`] calls `fsm_parser::parse` +
//! `fsm_analyzer::analyze_with_source` + the import-security pass
//! (`resolve_import`) in `fsm check`'s exact order — so an editor squiggle
//! can never disagree with `fsm check --json`. The LSP re-implements no
//! analysis and shells out to no binary (Doc 20 §9.4 / Doc 26 §3).

// The project invariant is `#![forbid(unsafe_code)]` in every crate.
// `tower-lsp`/`tokio` require NO consumer-side `unsafe`, so `fsm-lsp`
// upholds it — taking the workspace to 10/10 forbid-unsafe crates.
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

pub mod analysis;
pub mod capabilities;
pub mod document_store;
pub mod position;
pub mod server;

use tower_lsp::{LspService, Server};

pub use server::Backend;

/// Run the language server over stdio (the Doc 14 §1 default transport).
///
/// Blocks until the client closes stdin / the LSP `exit` notification is
/// received. `tower-lsp` drives the JSON-RPC framing; [`Backend`] supplies
/// the L1 capability behaviour.
pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
