//! The `tower-lsp` backend — Doc 26 §2.3 / §5 (L1 capabilities only).
//!
//! Implements the LSP lifecycle (`initialize`/`initialized`/`shutdown`),
//! full-document sync (`didOpen`/`didChange`/`didClose`), a ~200ms debounce
//! (Doc 14 §14 / Doc 26 §4.3), `publishDiagnostics`, and the L2 read
//! capabilities `documentSymbol` (Doc 14 §13) + `foldingRange` (Doc 14
//! §12). Every diagnostic AND every symbol comes from the exact `fsm
//! check` pipeline ([`crate::analysis::analyze`]) — this module never
//! re-analyses (Doc 20 §9.4 / Doc 26 §3): `documentSymbol` consumes the
//! `symbol_table` from the **same** `analyze()` the diagnostics path runs
//! (Doc 26 §8 L2: "one analysis feeds both"); `foldingRange` is a pure
//! parse-tree walk (no analysis at all).
//!
//! L2 scope boundary (Doc 26 §8): NO hover/definition/completion/rename/
//! references/semanticTokens/codeAction/inlayHint. Those are L3+ and are
//! deliberately neither implemented nor stubbed (a silent no-op handler is
//! worse than an unadvertised capability).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result as RpcResult;
use tower_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DocumentSymbolParams, DocumentSymbolResponse, FoldingRange, FoldingRangeParams,
    FoldingRangeProviderCapability, InitializeParams, InitializeResult, InitializedParams,
    MessageType, OneOf, PositionEncodingKind, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer};

use crate::analysis::analyze;
use crate::capabilities::diagnostics::to_lsp_diagnostics;
use crate::capabilities::document_symbol::document_symbols;
use crate::capabilities::folding::folding_ranges;
use crate::document_store::DocumentStore;
use crate::position::OffsetEncoding;

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
}
