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
//! ## L5 scope (Doc 26 §8 L5 — references + rename, the ONE new analysis)
//!
//! `textDocument/references` (Doc 14 §7) + `textDocument/prepareRename` +
//! `textDocument/rename` (Doc 14 §8, **single-file** — cross-file is v1.3,
//! Doc 26 §4.6/§9), advertised in `initialize`. These are powered by
//! [`refs::ReferenceIndex`] — the **one genuinely-new analysis** of the
//! LSP epic, and the only place L5 adds analysis. It is *derived* from the
//! single `analyze()` (it runs no `analyze()`/lowering of its own — a
//! single CST walk over that one analysis's `symbol_table` + parsed `cst`)
//! and **semantic-only**: a token is a reference to a symbol iff the SAME
//! L3 [`capabilities::resolve`] classifier (which mirrors the analyzer's
//! `checks::name_resolution` and resolves through the SAME
//! `SymbolTable::resolve_*` `fsm check` uses) resolves it to *exactly that
//! declaration* — never an identifier-string/text match. A token inside a
//! string literal, comment, or trivia is not a CST `Ident` in a classified
//! position, and a same-spelled token in a different scope resolves to a
//! different declaration `Span`; both are excluded **by construction**, so
//! the `WorkspaceEdit` can never silently corrupt the user's source (Doc
//! 26 risk-2, the cardinal sin in its most acute form). `prepareRename`
//! hard-rejects (up-front, per the LSP contract — never silent-allow)
//! machine names (codegen/ABI blast radius, out of v1.2 scope),
//! `@id`/state-id annotation strings, keywords/contextual keywords,
//! non-identifier cursors, and any string/comment/trivia position;
//! `rename` additionally rejects an invalid new identifier and an in-scope
//! name collision with a clear message and **no edit**. Still one
//! analysis, one position converter, one resolver — reused, not duplicated.
//!
//! ## L6 scope (Doc 26 §8 L6 — `semanticTokens`, no new analysis)
//!
//! `textDocument/semanticTokens/full` + `textDocument/semanticTokens/range`
//! (Doc 14 §10, **single-file**), advertised in `initialize` with the Doc
//! 14 §2/§10 `legend` declared **once** in
//! [`capabilities::semantic_tokens::legend`] and reused by the encoder (the
//! advertised indices and the encoded `tokenType`/`tokenModifiers` can
//! never diverge). Semantic tokens are *more precise* than the Doc 21
//! TextMate grammar: a bare `Ident` is one TextMate scope everywhere, but
//! this layer knows — from the **same** single `analyze()` — whether it is
//! a state / event / extern / context field / machine and whether it is a
//! *declaration* or a *reference*. The decl-vs-ref + entity-type split
//! **reuses** L3's [`capabilities::resolve`] (use sites) and L5's
//! [`refs::collect_decl_name_tokens`] / [`refs::SymbolKey`] taxonomy
//! (declaration sites) — one classifier, one `symbol_table` identity
//! model, **no new analysis, no parallel classifier** (Doc 26 §8 L6).
//! Non-`Ident` tokens get their lexical `fsm-lexer` `SyntaxKind` type
//! (keyword / operator / number / string / comment / `@id` decorator);
//! structural punctuation Doc 14 §10 has no legend slot for is not emitted
//! (the client uses the Doc 21 TextMate scope — Doc 21 §6 coexistence). The
//! LSP relative delta encoding measures `deltaStartChar`/`length` in the
//! negotiated `positionEncoding` via L1's ONE authoritative `LineIndex`
//! (no second converter); multi-line comments are split per line (LSP
//! `multilineTokenSupport` defaults off). Still one analysis, one position
//! converter, one resolver — reused, not duplicated.
//!
//! ## L7 scope (Doc 26 §8 L7 — `codeAction` + `inlayHint`, feature-complete)
//!
//! `textDocument/codeAction` (Doc 14 §9) + `textDocument/inlayHint` (Doc 14
//! §11), advertised in `initialize` — the final v1.2 LSP capabilities.
//! `codeAction` is **edit-producing**, so it inherits L5's risk-2
//! silent-corruption discipline: a `quickfix` is offered ONLY when the fix
//! is provably mechanical — `FSM-E0107` (insert `initial <FirstState>`
//! after the machine's `{`) and `FSM-E0022` (delete the duplicate event
//! declaration, withheld if it carries its own `@id`). The other five Doc
//! 14 §9 codes (E0100/E0106/W0200/W0500/E0300) and **both**
//! `refactor.extract` actions are deliberately **scoped OUT** (W0200/W0500
//! are never emitted by the toolchain at all; E0100/E0106/E0300/extract
//! cannot be mechanized without a heuristic or a parallel re-analysis) and
//! flagged in Doc 00 §11.38 — a missing quick-fix is a minor UX gap, a
//! wrong edit is the cardinal sin. `inlayHint` is **read-only display**
//! from the threaded `Analysis.ir` (the Doc-26-§5 trio: non-default
//! transition priority, timer durations, composite/parallel substate
//! count), each gated by its Doc 22 §8 toggle plus the master
//! `enableInlayHints`; Doc 14 §11's extern-param-name row is scoped out
//! (no Doc 22 §8 toggle, not in Doc 26 §5's IR-sourced set) and flagged.
//! Still one analysis, one position converter — reused, not duplicated;
//! L1–L6 behaviour byte-unchanged.
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
pub mod config;
pub mod document_store;
pub mod position;
pub mod refs;
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
