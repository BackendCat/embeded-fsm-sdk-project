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
//! ## L4 scope (Doc 26 §8 L4 — context-aware completion, no new analysis)
//!
//! `textDocument/completion` (Doc 14 §4, **single-file**), advertised in
//! `initialize` with the Doc 14 §2 trigger characters
//! `[".", ":", "@", "[", " "]`. The trigger-context classifier
//! ([`capabilities::complete`]) **reuses L3's [`capabilities::resolve`]
//! CST substrate** — the SAME `enclosing` node-ancestry walk + `in_guard`
//! predicate + a shared `prev_significant_token` primitive — to decide
//! "what may legally be typed here" (the dual of L3's "what is the
//! identifier here"); it is NOT a parallel ad-hoc context detector (Doc 26
//! §8 L4). Every name candidate is sourced from the **same** single
//! analysis's `symbol_table`; keywords are a verbatim transcription of
//! Doc 04 §1.5 (the single normative registry), pinned to it by a test.
//! Wrong-context candidates are *structurally* impossible (one context →
//! one candidate-class set); an unclassifiable cursor yields an empty
//! list, never a dump of every symbol. Still one analysis, no second
//! position converter (no `TextEdit` ranges are emitted).
//!
//! L5+ capabilities (references, rename, semanticTokens, codeAction,
//! inlayHint) are out of L4 scope and are NOT stubbed (a silent no-op
//! handler is worse than an unadvertised capability).
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
