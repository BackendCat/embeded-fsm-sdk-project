//! The `tower-lsp` backend — Doc 26 §2.3 / §5 (L1+L2+L3+L4+L5 caps).
//!
//! Implements the LSP lifecycle (`initialize`/`initialized`/`shutdown`),
//! full-document sync (`didOpen`/`didChange`/`didClose`), a ~200ms debounce
//! (Doc 14 §14 / Doc 26 §4.3), `publishDiagnostics`, the L2 read
//! capabilities `documentSymbol` (Doc 14 §13) + `foldingRange` (Doc 14
//! §12), the L3 capabilities `hover` (Doc 14 §5) + `definition` (Doc 14
//! §6, single-file), the L4 capability `completion` (Doc 14 §4,
//! context-aware, single-file), and the L5 capabilities `references` (Doc
//! 14 §7) + `prepareRename`/`rename` (Doc 14 §8, single-file). Every
//! diagnostic, symbol, hover, goto, completion, reference and rename comes
//! from the exact `fsm check` pipeline ([`crate::analysis::analyze`]) —
//! this module never re-analyses (Doc 20 §9.4 / Doc 26 §3):
//! `documentSymbol`/`hover`/`definition`/`completion` consume the
//! `symbol_table` (and, for hover, the additively threaded `ir`) from the
//! **same** `analyze()` the diagnostics path runs; `foldingRange` is a
//! pure parse-tree walk (no analysis). `completion` reuses L3's `resolve`
//! CST substrate for trigger-context classification. `references`/`rename`
//! consume the L5 [`ReferenceIndex`] — the ONE genuinely-new analysis,
//! itself *derived* from that same single `analyze()` (a single CST walk,
//! no second pass) and **semantic-only** (every reference proven by the
//! SAME L3 `resolve` classifier + `SymbolTable::resolve_*`, never a text
//! match); they reuse the `resolve` seam, not a parallel resolver, and
//! `LineIndex` for ranges, not a second converter (Doc 26 §8 L5).
//!
//! L6 adds `semanticTokens` (`textDocument/semanticTokens/full` + `/range`,
//! Doc 14 §10) — *more precise* than the Doc 21 TextMate grammar: it knows,
//! from the SAME single `analyze()`, whether an `Ident` is a state / event
//! / extern / context field / machine and whether it is a declaration or a
//! reference, reusing L3's `resolve` classifier (use sites) + L5's
//! `ReferenceIndex` decl-name discovery / `SymbolKey` taxonomy (declaration
//! sites) — one classifier, one `symbol_table` identity model, NO new
//! analysis, NO parallel classifier (Doc 26 §8 L6). The LSP relative delta
//! encoding measures `deltaStartChar`/`length` in the negotiated
//! `positionEncoding` via L1's one authoritative `LineIndex`.
//!
//! L6 scope boundary (Doc 26 §8): NO codeAction/inlayHint. Those are L7 and
//! are deliberately neither implemented nor stubbed (a silent no-op handler
//! is worse than an unadvertised capability — the
//! `workspaceSymbol`-left-unadvertised precedent, Doc 00 §11.33(5)).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Error as RpcError;
use tower_lsp::jsonrpc::Result as RpcResult;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentSymbolParams,
    DocumentSymbolResponse, FoldingRange, FoldingRangeParams, FoldingRangeProviderCapability,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, Location, MessageType, OneOf,
    PositionEncodingKind, PrepareRenameResponse, ReferenceParams, RenameOptions, RenameParams,
    SemanticTokensOptions, SemanticTokensParams, SemanticTokensRangeParams,
    SemanticTokensRangeResult, SemanticTokensResult, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, TextDocumentPositionParams, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url, WorkspaceEdit,
};
use tower_lsp::{Client, LanguageServer};

use crate::analysis::analyze;
use crate::capabilities::complete::completions;
use crate::capabilities::definition::goto_definition;
use crate::capabilities::diagnostics::to_lsp_diagnostics;
use crate::capabilities::document_symbol::document_symbols;
use crate::capabilities::folding::folding_ranges;
use crate::capabilities::hover::hover as build_hover;
use crate::capabilities::references::references as build_references;
use crate::capabilities::rename::{prepare_rename_handler, rename_handler};
use crate::capabilities::semantic_tokens::{
    legend as semantic_tokens_legend, semantic_tokens_full as build_semantic_tokens_full,
    semantic_tokens_range as build_semantic_tokens_range,
};
use crate::document_store::DocumentStore;
use crate::position::OffsetEncoding;
use crate::refs::ReferenceIndex;

/// Debounce window before a re-analyze fires (Doc 14 §14 / Doc 22 §8
/// `fsmLang.debounceMs` default; Doc 26 §4.3). Full re-parse+analyze of a
/// realistic embedded `.fsm` is sub-ms to low-single-digit-ms, so 200ms is
/// comfortably ahead of the analysis cost (Doc 26 §4.4).
const DEBOUNCE: Duration = Duration::from_millis(200);

/// The language-server backend. One instance per connection.
#[derive(Debug)]
pub struct Backend {
    client: Client,
    /// Open-document buffers (full-sync model, Doc 26 §4.3).
    docs: Arc<Mutex<DocumentStore>>,
    /// Negotiated `positionEncoding` (Doc 26 §4.1). Set in `initialize`;
    /// defaults to UTF-8 until then. Behind a mutex because `initialize`
    /// runs on the same `&self` as later notification handlers.
    encoding: Arc<Mutex<OffsetEncoding>>,
    /// Per-URI debounce generation counter. Each edit bumps the URI's
    /// generation; a scheduled re-analyze only publishes if its captured
    /// generation is still current — so a burst of keystrokes collapses to
    /// one analysis (Doc 26 §4.3), with no timer cancellation bookkeeping.
    debounce: Arc<Mutex<HashMap<Url, u64>>>,
}

impl Backend {
    /// Construct a backend bound to `client`.
    pub fn new(client: Client) -> Self {
        Backend {
            client,
            docs: Arc::new(Mutex::new(DocumentStore::default())),
            // Pre-negotiation default. `initialize` overrides this per the
            // client's `general.positionEncodings` capability.
            encoding: Arc::new(Mutex::new(OffsetEncoding::Utf8)),
            debounce: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Best-effort filesystem path for a document URI. The import-security
    /// containment check (Doc 26 §3) needs a path; a non-`file:` URI
    /// (untitled buffer) has none, so we fall back to a bare name — the
    /// resolver then treats the file's *parent* as the workspace root,
    /// which is the most permissive position that still rejects escapes
    /// (identical degradation to `resolve_import`'s no-parent fallback).
    fn uri_to_path(uri: &Url) -> PathBuf {
        uri.to_file_path()
            .unwrap_or_else(|_| PathBuf::from("unsaved.fsm"))
    }

    /// Schedule a debounced re-analyze + publish for `uri`. Bumps the
    /// URI's generation, then after [`DEBOUNCE`] re-checks: if a newer edit
    /// arrived (generation moved), this scheduled run is stale and does
    /// nothing — the newer edit owns the publish. This is the
    /// generation-counter debounce (no JoinHandle cancellation needed).
    async fn schedule_analyze(&self, uri: Url) {
        let generation = {
            let mut d = self.debounce.lock().await;
            let g = d.entry(uri.clone()).or_insert(0);
            *g += 1;
            *g
        };

        let docs = Arc::clone(&self.docs);
        let debounce = Arc::clone(&self.debounce);
        let encoding = Arc::clone(&self.encoding);
        let client = self.client.clone();

        tokio::spawn(async move {
            tokio::time::sleep(DEBOUNCE).await;
            // Stale-edit guard: only the most recent scheduled run for this
            // URI is allowed to publish (Doc 26 §4.3 debounce).
            {
                let d = debounce.lock().await;
                if d.get(&uri).copied() != Some(generation) {
                    return;
                }
            }
            // Snapshot the buffer under the lock, then release before the
            // (sync, fast) analyze so we don't hold the doc mutex across
            // the await-free analysis call.
            let snapshot = {
                let store = docs.lock().await;
                store
                    .get(&uri)
                    .map(|d| (d.text.clone(), d.version, d.line_index.clone()))
            };
            let Some((text, version, line_index)) = snapshot else {
                return;
            };
            let enc = *encoding.lock().await;
            let path = Backend::uri_to_path(&uri);
            // THE reuse seam — exact `fsm check` pipeline (Doc 26 §3).
            let analysis = analyze(&text, &path);
            let lsp_diags =
                to_lsp_diagnostics(&analysis.diagnostics, &uri, &text, &line_index, enc);
            client
                .publish_diagnostics(uri, lsp_diags, Some(version))
                .await;
        });
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> RpcResult<InitializeResult> {
        // Position-encoding negotiation (Doc 26 §4.1 decision): prefer
        // UTF-8 (it deletes the UTF-16 transcoding class) iff the client
        // explicitly lists it in `general.positionEncodings`. Every other
        // case -> UTF-16: that single fallback correctly covers both a
        // 3.17 client that offered only utf-16 AND a pre-3.17 client that
        // advertised no capability at all (LSP <=3.16's universal
        // default). The two non-UTF-8 outcomes are deliberately the same,
        // so this is one branch, not two.
        let client_encodings = params
            .capabilities
            .general
            .and_then(|g| g.position_encodings)
            .unwrap_or_default();
        let negotiated = if client_encodings.contains(&PositionEncodingKind::UTF8) {
            OffsetEncoding::Utf8
        } else {
            OffsetEncoding::Utf16
        };
        *self.encoding.lock().await = negotiated;

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(negotiated.to_lsp()),
                // Full-document sync (Doc 26 §4.3): the client sends the
                // whole buffer on every change; we re-parse on debounce.
                // (Incremental *wire* sync is a v1.3 optimisation, Doc 26
                // §4.3/§4.5 — NOT advertised here so the contract is
                // exactly what L1 implements.)
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                // L2 (Doc 26 §8 L2): both are honest, fully-implemented
                // providers backed by the single reused analysis / a pure
                // CST walk — NOT advertised-but-stubbed (the cardinal sin).
                document_symbol_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                // L3 (Doc 26 §8 L3): hover + single-file goto-definition.
                // Both are honest, fully-implemented providers over the
                // shared `resolve` seam (same single analysis) — advertised
                // because they genuinely work, NOT stubbed.
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                // L4 (Doc 26 §8 L4): context-aware completion. An honest,
                // fully-implemented provider whose trigger-context
                // classifier reuses L3's `resolve` CST substrate (one
                // analysis feeds it) — advertised because it genuinely
                // works, NOT stubbed. Trigger characters are the EXACT
                // Doc 14 §2 set `[".", ":", "@", "[", " "]` (verified
                // against the spec's `ServerCapabilities` block, not
                // guessed). `resolve_provider: false` — L4 returns fully
                // resolved items (no `completionItem/resolve` round-trip;
                // advertising a resolve we do not implement would be the
                // stub-a-no-op sin).
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".to_owned(),
                        ":".to_owned(),
                        "@".to_owned(),
                        "[".to_owned(),
                        " ".to_owned(),
                    ]),
                    resolve_provider: Some(false),
                    ..Default::default()
                }),
                // L5 (Doc 26 §8 L5): references + rename. Both are honest,
                // fully-implemented providers over the L5 `ReferenceIndex`
                // (the ONE new analysis, derived from the same single
                // analysis, semantic-only) — advertised because they
                // genuinely work, NOT stubbed.
                references_provider: Some(OneOf::Left(true)),
                // `prepare_provider: true` — the client MUST call
                // `prepareRename` first, which is exactly the up-front
                // "is this renameable?" contract Doc 26 risk-2 wants
                // (never silent-allow → corrupting edit). `rename` itself
                // re-validates (defence-in-depth: a client may skip
                // prepare). `RenameOptions` (not the bare `OneOf::Left`)
                // is required to express `prepare_provider`.
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: Default::default(),
                })),
                // L6 (Doc 26 §8 L6 / Doc 14 §2/§10): semantic tokens. An
                // honest, fully-implemented provider — `full` AND `range`
                // both genuinely work (Doc 26 §8 L6 says "full + range";
                // Doc 14 §2's block has `"full": true, "range": true`),
                // backed by the SAME single analysis (no second pass) +
                // the reused L3/L5 classifier (no parallel one). The
                // `legend` is declared ONCE in `semantic_tokens::legend()`
                // and reused by the encoder, so the advertised indices and
                // the encoded `tokenType`/`tokenModifiers` can never
                // diverge (the §5.4 tests assert this identity). NOT
                // advertised-but-stubbed (the cardinal sin) — it is
                // advertised because it genuinely works.
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: semantic_tokens_legend(),
                            full: Some(tower_lsp::lsp_types::SemanticTokensFullOptions::Bool(true)),
                            range: Some(true),
                            work_done_progress_options: Default::default(),
                        },
                    ),
                ),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "fsm-lang-server".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "fsm-lang-server initialized")
            .await;
    }

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        {
            let mut store = self.docs.lock().await;
            store.open(doc.uri.clone(), doc.text, doc.version);
        }
        self.schedule_analyze(doc.uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full-sync model (Doc 26 §4.3): with TextDocumentSyncKind::FULL
        // the client sends exactly one change whose `text` is the entire
        // new buffer. Take the last (defensive — spec guarantees one).
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        if let Some(change) = params.content_changes.into_iter().next_back() {
            {
                let mut store = self.docs.lock().await;
                store.replace(&uri, change.text, version);
            }
            self.schedule_analyze(uri).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        {
            let mut store = self.docs.lock().await;
            store.close(&uri);
        }
        {
            // Drop the debounce generation so a late scheduled run for a
            // now-closed doc can't publish, and the map doesn't grow.
            let mut d = self.debounce.lock().await;
            d.remove(&uri);
        }
        // Clear diagnostics for the closed document (LSP convention: the
        // server owns the squiggles only while the doc is open).
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    /// `textDocument/documentSymbol` — Doc 14 §13 hierarchical tree.
    ///
    /// Reuses the **exact** `fsm check` analysis ([`analyze`]) and consumes
    /// its `symbol_table`: this is the SAME pipeline call the debounced
    /// `publishDiagnostics` makes, so the symbol tree can no more disagree
    /// with `fsm check` than the diagnostics can (Doc 26 §3 / §8 L2 "one
    /// analysis feeds both"). The CST (recovered from the same parse) is
    /// used only to locate name tokens for `selectionRange`. A request for
    /// a not-open document returns `None` (the spec-correct empty answer —
    /// never a panic). The snapshot is released before the await-free
    /// analysis, exactly as the debounce path does.
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> RpcResult<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let snapshot = {
            let store = self.docs.lock().await;
            store
                .get(&uri)
                .map(|d| (d.text.clone(), d.line_index.clone()))
        };
        let Some((text, line_index)) = snapshot else {
            return Ok(None);
        };
        let enc = *self.encoding.lock().await;
        let path = Backend::uri_to_path(&uri);
        // THE reuse seam — the identical `fsm check` pipeline whose
        // `symbol_table` L1 discarded and L2 now threads through.
        let analysis = analyze(&text, &path);
        let cst = fsm_parser::parse(&text).syntax();
        let symbols = document_symbols(&analysis.symbol_table, &cst, &line_index, &text, enc);
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    /// `textDocument/foldingRange` — Doc 14 §12 folding regions.
    ///
    /// Pure structural projection: a single CST descendant walk of the
    /// parse tree (no analysis, no symbol table — Doc 26 §5 `foldingRange`
    /// row). Line numbers go through L1's `LineIndex` in the negotiated
    /// encoding (no second position converter). Not-open document → `None`.
    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> RpcResult<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        let snapshot = {
            let store = self.docs.lock().await;
            store
                .get(&uri)
                .map(|d| (d.text.clone(), d.line_index.clone()))
        };
        let Some((text, line_index)) = snapshot else {
            return Ok(None);
        };
        let enc = *self.encoding.lock().await;
        let cst = fsm_parser::parse(&text).syntax();
        Ok(Some(folding_ranges(&cst, &line_index, &text, enc)))
    }

    /// `textDocument/hover` — Doc 14 §5 / Doc 26 §8 L3.
    ///
    /// Resolves the identifier under the cursor via the shared `resolve`
    /// seam (the SAME `SymbolTable` resolution `documentSymbol`/`definition`
    /// and `fsm check` use) and renders Markdown enriched from the
    /// **additively threaded** `Analysis.ir` (one analysis, no second
    /// lowering — the L2 one-analysis invariant, widened to include the
    /// IR). Cursor not on a resolvable in-file symbol (whitespace, keyword,
    /// declaration site, unknown/cross-file name) → `None`: the
    /// spec-correct "no hover", never a panic and never an empty tooltip.
    /// The snapshot is released before the await-free analysis, exactly as
    /// the L2 paths do.
    async fn hover(&self, params: HoverParams) -> RpcResult<Option<Hover>> {
        let pos = params.text_document_position_params;
        let uri = pos.text_document.uri;
        let snapshot = {
            let store = self.docs.lock().await;
            store
                .get(&uri)
                .map(|d| (d.text.clone(), d.line_index.clone()))
        };
        let Some((text, line_index)) = snapshot else {
            return Ok(None);
        };
        let enc = *self.encoding.lock().await;
        // Cursor Position -> byte via the ONE authoritative LineIndex
        // inverse (no second converter — Doc 26 §4.1 / §11.32 boundary).
        let byte = line_index.offset(&text, pos.position, enc);
        let path = Backend::uri_to_path(&uri);
        // THE reuse seam — the identical `fsm check` pipeline; hover reads
        // the threaded `symbol_table` + `ir` from this single run.
        let analysis = analyze(&text, &path);
        let cst = fsm_parser::parse(&text).syntax();
        Ok(build_hover(
            &analysis.symbol_table,
            analysis.ir.as_ref(),
            &cst,
            byte,
            &line_index,
            &text,
            enc,
        ))
    }

    /// `textDocument/definition` — Doc 14 §6 / Doc 26 §8 L3 (single-file).
    ///
    /// Resolves the identifier under the cursor to its declaration via the
    /// shared `resolve` seam and returns the declaration's `Location` in
    /// **this** document (the decl span is a byte range of the same parsed
    /// buffer — never a fabricated or cross-file location). Cross-file is
    /// explicitly v1.3 (Doc 26 §4.6/§9); a symbol the single-file index
    /// cannot resolve → `None` (Doc 14 §8 graceful degradation), never a
    /// wrong jump (the silent-data-loss cardinal sin).
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> RpcResult<Option<GotoDefinitionResponse>> {
        let pos = params.text_document_position_params;
        let uri = pos.text_document.uri;
        let snapshot = {
            let store = self.docs.lock().await;
            store
                .get(&uri)
                .map(|d| (d.text.clone(), d.line_index.clone()))
        };
        let Some((text, line_index)) = snapshot else {
            return Ok(None);
        };
        let enc = *self.encoding.lock().await;
        let byte = line_index.offset(&text, pos.position, enc);
        let path = Backend::uri_to_path(&uri);
        // THE reuse seam — identical `fsm check` pipeline; resolution goes
        // through the SAME `SymbolTable::resolve_*` the diagnostics use.
        let analysis = analyze(&text, &path);
        let cst = fsm_parser::parse(&text).syntax();
        Ok(goto_definition(
            &analysis.symbol_table,
            &cst,
            &uri,
            byte,
            &line_index,
            &text,
            enc,
        )
        .map(GotoDefinitionResponse::Scalar))
    }

    /// `textDocument/completion` — Doc 14 §4 / Doc 26 §8 L4 (single-file).
    ///
    /// Classifies the trigger context at the cursor by **reusing L3's
    /// `resolve` CST substrate** (the SAME `enclosing` ancestry walk +
    /// `in_guard` predicate + the shared `prev_significant_token`
    /// primitive — Doc 26 §8 L4: reuse/extend the classifier, do NOT build
    /// a parallel detector) and returns ONLY context-correct candidates,
    /// every name sourced from the threaded `symbol_table` of the **same**
    /// single `analyze()` the diagnostics path runs (Doc 26 §3/§8 — no
    /// second analysis, no parallel context detector). The cursor
    /// `Position` → byte uses the ONE authoritative `LineIndex` inverse
    /// (no second converter — Doc 26 §4.1 / §11.32 boundary). An
    /// unclassifiable cursor → an empty list (the spec-correct "nothing
    /// meaningful here", never a dump of every symbol — the
    /// wrong-context-noise sin). The snapshot is released before the
    /// await-free analysis, exactly as the L2/L3 paths do.
    async fn completion(&self, params: CompletionParams) -> RpcResult<Option<CompletionResponse>> {
        let pos = params.text_document_position;
        let uri = pos.text_document.uri;
        let snapshot = {
            let store = self.docs.lock().await;
            store
                .get(&uri)
                .map(|d| (d.text.clone(), d.line_index.clone()))
        };
        let Some((text, line_index)) = snapshot else {
            return Ok(None);
        };
        let enc = *self.encoding.lock().await;
        // Cursor Position -> byte via the ONE authoritative LineIndex
        // inverse (the L3 seam — NOT a second converter).
        let byte = line_index.offset(&text, pos.position, enc);
        let path = Backend::uri_to_path(&uri);
        // THE reuse seam — the identical `fsm check` pipeline; completion
        // reads the threaded `symbol_table` from this single run.
        let analysis = analyze(&text, &path);
        let cst = fsm_parser::parse(&text).syntax();
        let items = completions(&analysis.symbol_table, &cst, byte);
        // An empty list is a valid LSP response meaning "no suggestions
        // here" — that is the deliberate degradation for an
        // unclassifiable / wrong context, NOT a missing capability.
        Ok(Some(CompletionResponse::Array(items)))
    }

    /// `textDocument/references` — Doc 14 §7 / Doc 26 §8 L5 (single-file).
    ///
    /// Builds the L5 [`ReferenceIndex`] from the SAME single `analyze()`
    /// the diagnostics path runs (the ONE new analysis, *derived* — a CST
    /// walk over that analysis's `symbol_table` + parsed `cst`; no second
    /// analysis pass, no parallel resolver) and returns every
    /// semantically-resolved occurrence of the symbol under the cursor,
    /// honouring `context.include_declaration`. Resolution is purely
    /// semantic (the SAME L3 `resolve` classifier); a same-spelled
    /// string/comment/different-scope token is excluded by construction
    /// (Doc 26 risk-2). Cursor not on a resolvable in-file symbol → `None`
    /// (the spec-correct empty answer, never a panic, never a textual
    /// guess). Every range is in *this* buffer (single-file; cross-file is
    /// v1.3, Doc 26 §4.6). Snapshot released before the await-free
    /// analysis, exactly as the L2/L3/L4 paths do.
    async fn references(&self, params: ReferenceParams) -> RpcResult<Option<Vec<Location>>> {
        let pos = params.text_document_position;
        let uri = pos.text_document.uri;
        let include_declaration = params.context.include_declaration;
        let snapshot = {
            let store = self.docs.lock().await;
            store
                .get(&uri)
                .map(|d| (d.text.clone(), d.line_index.clone()))
        };
        let Some((text, line_index)) = snapshot else {
            return Ok(None);
        };
        let enc = *self.encoding.lock().await;
        let byte = line_index.offset(&text, pos.position, enc);
        let path = Backend::uri_to_path(&uri);
        // THE reuse seam — identical `fsm check` pipeline; the index is
        // derived from THIS single run's symbol_table + parse.
        let analysis = analyze(&text, &path);
        let cst = fsm_parser::parse(&text).syntax();
        let index = ReferenceIndex::build(&analysis.symbol_table, &cst);
        Ok(build_references(
            &analysis.symbol_table,
            &index,
            &cst,
            &uri,
            byte,
            include_declaration,
            &line_index,
            &text,
            enc,
        ))
    }

    /// `textDocument/prepareRename` — Doc 14 §8 / Doc 26 §8 L5 / risk-2.
    ///
    /// The up-front renameability contract: returns the bare-identifier
    /// range ONLY for a safely-renameable user symbol (state / event /
    /// extern / context field declared in this file), and an **error**
    /// (telling the client up-front it is not renameable — never a silent
    /// allow that becomes a corrupting edit) for a machine name (codegen/
    /// ABI blast radius, out of v1.2 scope), an `@id`/state-id annotation
    /// string, a keyword/contextual keyword, a non-identifier cursor, or
    /// any position inside a string/comment/trivia. Resolution is purely
    /// semantic (the SAME L3 `resolve` seam via the [`ReferenceIndex`]),
    /// so a non-`Ident`/string/comment position is structurally a
    /// rejection. Snapshot released before the await-free analysis.
    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> RpcResult<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let snapshot = {
            let store = self.docs.lock().await;
            store
                .get(&uri)
                .map(|d| (d.text.clone(), d.line_index.clone()))
        };
        let Some((text, line_index)) = snapshot else {
            return Ok(None);
        };
        let enc = *self.encoding.lock().await;
        let byte = line_index.offset(&text, params.position, enc);
        let path = Backend::uri_to_path(&uri);
        let analysis = analyze(&text, &path);
        let cst = fsm_parser::parse(&text).syntax();
        // A refusal is surfaced as a JSON-RPC error so the client shows
        // the reason up-front (the LSP `prepareRename` contract) rather
        // than letting `rename` produce a dangerous edit (Doc 26 risk-2).
        match prepare_rename_handler(&analysis.symbol_table, &cst, byte, &line_index, &text, enc) {
            Ok(resp) => Ok(resp),
            Err(message) => Err(RpcError::invalid_params(message)),
        }
    }

    /// `textDocument/rename` — Doc 14 §8 / Doc 26 §8 L5 / risk-2 (the
    /// cardinal silent-data-loss risk in its most acute form).
    ///
    /// Returns a [`WorkspaceEdit`] whose text edits are **exactly** the L5
    /// [`ReferenceIndex`] occurrences (decl + semantically-resolved uses)
    /// of the target, each rewritten to the new name. The edit set is
    /// semantic-only: a same-spelled string-literal substring, comment
    /// word, or different-scope symbol never entered the index, so it can
    /// **never** be in the `WorkspaceEdit` (the headline risk-2
    /// guarantee). Rejects (a clear JSON-RPC error, **no edit**) a
    /// non-renameable cursor / machine name / cross-file-exposed symbol /
    /// invalid new identifier / in-scope name collision. Single-file
    /// only; cross-file is v1.3 (Doc 26 §4.6/§9). Snapshot released before
    /// the await-free analysis.
    async fn rename(&self, params: RenameParams) -> RpcResult<Option<WorkspaceEdit>> {
        let pos = params.text_document_position;
        let uri = pos.text_document.uri;
        let new_name = params.new_name;
        let snapshot = {
            let store = self.docs.lock().await;
            store
                .get(&uri)
                .map(|d| (d.text.clone(), d.line_index.clone()))
        };
        let Some((text, line_index)) = snapshot else {
            return Ok(None);
        };
        let enc = *self.encoding.lock().await;
        let byte = line_index.offset(&text, pos.position, enc);
        let path = Backend::uri_to_path(&uri);
        // THE reuse seam — identical pipeline; the index is derived from
        // THIS single run. NO second analysis, NO text-based matching.
        let analysis = analyze(&text, &path);
        let cst = fsm_parser::parse(&text).syntax();
        let index = ReferenceIndex::build(&analysis.symbol_table, &cst);
        match rename_handler(
            &analysis.symbol_table,
            &index,
            &cst,
            &uri,
            byte,
            &new_name,
            &line_index,
            &text,
            enc,
        ) {
            Ok(edit) => Ok(Some(edit)),
            // A clear message, NO edit — never a partial/wrong
            // WorkspaceEdit (Doc 26 risk-2).
            Err(message) => Err(RpcError::invalid_params(message)),
        }
    }

    /// `textDocument/semanticTokens/full` — Doc 14 §10 / Doc 26 §8 L6
    /// (single-file).
    ///
    /// Classifies every CST token of the SAME single `analyze()` the
    /// diagnostics/symbol/hover/completion/references path runs (Doc 26
    /// §3/§8 — NO second analysis, NO parallel classifier): an `Ident` via
    /// the reused L3 `resolve` classifier (use sites) + L5 `ReferenceIndex`
    /// decl-name discovery / `SymbolKey` taxonomy (declaration sites — the
    /// decl-vs-ref + entity-type split is decided by `symbol_table`
    /// identity, exactly Doc 26 §8 L6); every other token by its lexical
    /// `fsm-lexer` `SyntaxKind`. Emitted as the LSP relative delta array
    /// with `deltaStartChar`/`length` in the negotiated `positionEncoding`
    /// via L1's ONE authoritative `LineIndex` (no second converter — the
    /// §11.32 DRIFT-2 boundary intact). A not-open document → `None` (the
    /// spec-correct empty answer, never a panic). Snapshot released before
    /// the await-free analysis, exactly as the L2–L5 paths do.
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> RpcResult<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let snapshot = {
            let store = self.docs.lock().await;
            store
                .get(&uri)
                .map(|d| (d.text.clone(), d.line_index.clone()))
        };
        let Some((text, line_index)) = snapshot else {
            return Ok(None);
        };
        let enc = *self.encoding.lock().await;
        let path = Backend::uri_to_path(&uri);
        // THE reuse seam — the identical `fsm check` pipeline; the tokens
        // are classified from THIS single run's symbol_table + parse.
        let analysis = analyze(&text, &path);
        let cst = fsm_parser::parse(&text).syntax();
        let tokens =
            build_semantic_tokens_full(&analysis.symbol_table, &cst, &line_index, &text, enc);
        Ok(Some(SemanticTokensResult::Tokens(tokens)))
    }

    /// `textDocument/semanticTokens/range` — Doc 14 §10 / Doc 26 §8 L6
    /// ("full + range" — both explicitly specified).
    ///
    /// Identical classification (the SAME one reused analysis, the SAME
    /// reused classifier — no second pass, no parallel classifier), then
    /// the token stream is filtered to tokens overlapping the requested
    /// byte range and the relative delta encoding is recomputed for the
    /// subset (so a range response is a self-contained token stream whose
    /// first token's deltas are relative to the response start, per the LSP
    /// relative-encoding contract — not a slice of the full stream). The
    /// request `Range` is mapped to bytes via L1's ONE authoritative
    /// `LineIndex` inverse in the negotiated encoding (no second
    /// converter). Not-open document → `None`.
    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> RpcResult<Option<SemanticTokensRangeResult>> {
        let uri = params.text_document.uri;
        let snapshot = {
            let store = self.docs.lock().await;
            store
                .get(&uri)
                .map(|d| (d.text.clone(), d.line_index.clone()))
        };
        let Some((text, line_index)) = snapshot else {
            return Ok(None);
        };
        let enc = *self.encoding.lock().await;
        // Request Range -> byte offsets via the ONE authoritative LineIndex
        // inverse (NOT a second converter — Doc 26 §4.1 / §11.32 boundary).
        let start_byte = line_index.offset(&text, params.range.start, enc);
        let end_byte = line_index.offset(&text, params.range.end, enc);
        let path = Backend::uri_to_path(&uri);
        // THE reuse seam — identical pipeline; classified from THIS run.
        let analysis = analyze(&text, &path);
        let cst = fsm_parser::parse(&text).syntax();
        let tokens = build_semantic_tokens_range(
            &analysis.symbol_table,
            &cst,
            &line_index,
            &text,
            enc,
            start_byte,
            end_byte,
        );
        Ok(Some(SemanticTokensRangeResult::Tokens(tokens)))
    }
}
