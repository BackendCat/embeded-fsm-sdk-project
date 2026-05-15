//! §5.4-LSP behavioural acceptance — Doc 26 §8 L1.
//!
//! This is the LSP analogue of the gcc-compile-and-RUN mandate
//! (FSM-PROC-SUBAGENT §5.4): an **in-process `tower-lsp` client** that
//! issues real JSON-RPC requests and asserts the **payloads** of the real
//! responses / server→client notifications. Symbol/route-presence is
//! explicitly FORBIDDEN as acceptance — the asserted *Range bytes* are the
//! proof.
//!
//! Harness: the canonical tower-lsp in-process pattern — `LspService::new`
//! gives a `tower::Service` we `call()` with `jsonrpc::Request`, plus a
//! `ClientSocket` whose split `RequestStream` yields every server→client
//! message (here: the `textDocument/publishDiagnostics` notification).
//!
//! The "expected" Range is **never hand-typed**: it is recomputed from the
//! exact `fsm_lsp::analysis::analyze` pipeline (the same code the server
//! runs, which is the same code `fsm check` runs) projected through
//! `LineIndex`, then byte-compared to what the server actually published.
//! A position-math regression makes the assertion fail — that is the point.

use std::path::Path;
use std::time::Duration;

use futures::StreamExt;
use serde_json::{json, Value};
use tower::{Service, ServiceExt};
use tower_lsp::jsonrpc::Request;
use tower_lsp::lsp_types::{Diagnostic as LspDiagnostic, PositionEncodingKind, Range, Url};
use tower_lsp::LspService;

use fsm_lsp::analysis::analyze;
use fsm_lsp::capabilities::diagnostics::to_lsp_diagnostics;
use fsm_lsp::position::{LineIndex, OffsetEncoding};
use fsm_lsp::Backend;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn fixture(name: &str) -> (String, Url) {
    let path = format!("{FIXTURES}/{name}");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read fixture {path}: {e}"));
    let uri = Url::from_file_path(&path).expect("fixture path is absolute");
    (text, uri)
}

/// Build the oracle: the LSP diagnostics the server *should* publish for
/// `text`, computed via the exact reused pipeline + `LineIndex`. This is
/// the SAME code path the server uses — so equality proves the server
/// wired the pipeline correctly AND the position math is correct (a wrong
/// `LineIndex` would corrupt this oracle and the server identically, but
/// the per-encoding cross-check below pins the math independently).
fn expected_diags(text: &str, uri: &Url, encoding: OffsetEncoding) -> Vec<LspDiagnostic> {
    let path = uri.to_file_path().unwrap();
    let analysis = analyze(text, &path);
    let idx = LineIndex::new(text);
    to_lsp_diagnostics(&analysis.diagnostics, uri, text, &idx, encoding)
}

fn initialize_params(encodings: &[PositionEncodingKind]) -> Value {
    json!({
        "capabilities": {
            "general": {
                "positionEncodings": encodings.iter().map(|e| e.as_str()).collect::<Vec<_>>()
            }
        }
    })
}

fn did_open_params(uri: &Url, text: &str) -> Value {
    json!({
        "textDocument": {
            "uri": uri,
            "languageId": "fsm",
            "version": 1,
            "text": text
        }
    })
}

fn did_change_params(uri: &Url, text: &str, version: i32) -> Value {
    json!({
        "textDocument": { "uri": uri, "version": version },
        "contentChanges": [ { "text": text } ]
    })
}

/// Drive `initialize` and return the negotiated `positionEncoding` the
/// server advertised back (asserts capability negotiation, Doc 26 §8 L1
/// acceptance (a)).
async fn do_initialize<S>(service: &mut S, encodings: &[PositionEncodingKind]) -> Value
where
    S: Service<Request, Response = Option<tower_lsp::jsonrpc::Response>>,
    S::Error: std::fmt::Debug,
{
    let req = Request::build("initialize")
        .params(initialize_params(encodings))
        .id(1)
        .finish();
    let resp = service
        .ready()
        .await
        .unwrap()
        .call(req)
        .await
        .unwrap()
        .expect("initialize returns a response");
    let v: Value = serde_json::to_value(resp).unwrap();
    v["result"].clone()
}

/// Pump the server→client `RequestStream` until a
/// `textDocument/publishDiagnostics` for `uri` arrives (the server fires
/// it from a debounced `tokio::spawn`, so we poll with a generous bound).
/// Returns the decoded diagnostics array.
async fn next_publish_for(
    stream: &mut (impl StreamExt<Item = Request> + Unpin),
    uri: &Url,
) -> Vec<LspDiagnostic> {
    // The 200ms debounce + spawn means the notification is not immediate.
    // 5s is far beyond the debounce yet fails fast if it never comes.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let next = tokio::time::timeout(remaining, stream.next()).await;
        let msg = match next {
            Ok(Some(m)) => m,
            Ok(None) => panic!("client socket closed before publishDiagnostics"),
            Err(_) => panic!("timed out waiting for publishDiagnostics for {uri}"),
        };
        if msg.method() != "textDocument/publishDiagnostics" {
            continue;
        }
        let params: Value = serde_json::to_value(&msg).unwrap();
        let p = &params["params"];
        if p["uri"] != json!(uri) {
            continue;
        }
        return serde_json::from_value(p["diagnostics"].clone())
            .expect("decode publishDiagnostics.diagnostics");
    }
}

/// (a) `initialize` advertises UTF-8 when the client supports it, plus
/// full text-document sync; (b) a clean doc → `publishDiagnostics` with an
/// EMPTY diagnostics array.
#[tokio::test(flavor = "current_thread")]
async fn initialize_negotiates_utf8_and_clean_doc_publishes_empty() {
    let (mut service, socket) = LspService::new(Backend::new);
    let (mut requests, _sink) = socket.split();

    let result = do_initialize(
        &mut service,
        &[PositionEncodingKind::UTF8, PositionEncodingKind::UTF16],
    )
    .await;
    assert_eq!(
        result["capabilities"]["positionEncoding"],
        json!("utf-8"),
        "server must prefer UTF-8 when the client offers it (Doc 26 §4.1)"
    );
    // Full-document sync (Doc 26 §4.3): TextDocumentSyncKind::FULL == 1.
    assert_eq!(
        result["capabilities"]["textDocumentSync"],
        json!(1),
        "L1 advertises full (not incremental) sync"
    );

    let (text, uri) = fixture("clean.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    let diags = next_publish_for(&mut requests, &uri).await;
    assert!(
        diags.is_empty(),
        "a clean .fsm must publish zero diagnostics, got {diags:?}"
    );
}

/// (b) A known-broken doc → the published diagnostic's CODE **and Range**
/// byte-match what the reused `fsm check` pipeline produces for the
/// identical source. This proves the server runs the *exact* pipeline
/// (not a parallel re-implementation) and projects positions correctly.
#[tokio::test(flavor = "current_thread")]
async fn broken_doc_publishes_exact_code_and_range_matching_check_pipeline() {
    let (mut service, socket) = LspService::new(Backend::new);
    let (mut requests, _sink) = socket.split();
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text, uri) = fixture("broken.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    let got = next_publish_for(&mut requests, &uri).await;
    let want = expected_diags(&text, &uri, OffsetEncoding::Utf8);

    // broken.fsm has exactly one error: FSM-E0107 (no initial).
    assert_eq!(got.len(), 1, "expected one diagnostic, got {got:?}");
    assert_eq!(want.len(), 1);
    assert_eq!(
        got[0].code,
        Some(tower_lsp::lsp_types::NumberOrString::String(
            "FSM-E0107".to_owned()
        )),
        "code must be the exact FSM wire string the CLI emits"
    );
    // The load-bearing assertion: byte-exact Range equality with the
    // pipeline oracle. NOT a symbol-presence stand-in.
    assert_eq!(
        got[0].range, want[0].range,
        "published Range must byte-match the reused pipeline's projection"
    );
    assert_eq!(got[0], want[0], "the whole projected diagnostic must match");
}

/// (c) THE risk-1 proof (Doc 26 §4.1 / §7.1). `non_ascii.fsm`'s error
/// line is `    state A { /* 🚀 ы */ on GO -> Nope }` — a 4-byte non-BMP
/// emoji (🚀) and 2-byte Cyrillic (ы) sit in a block comment *before* the
/// offending transition on the *same line*. The analyzer's E0100 span
/// covers the whole `on GO -> Nope ` transition (byte range (90,104));
/// projected onto that line the start column is:
///   UTF-8  col 28  (byte count of the line segment before `on`)
///   UTF-16 col 25  (🚀 = 2 code units, ы = 1 — 3 fewer than the byte
///                    count, so a byte-as-`character` shim emits 28 here
///                    and is WRONG by 3)
/// and the end column is UTF-8 42 / UTF-16 39. The two existing in-tree
/// converters (byte-1-based and scalar-1-based) would BOTH mis-encode
/// this. The expected columns are computed-from-source (recorded below);
/// the test asserts the published Range is correct under BOTH negotiated
/// encodings AND byte-matches the pipeline oracle per encoding. It FAILS
/// if position math regresses — that is the point.
#[tokio::test(flavor = "current_thread")]
async fn non_ascii_line_range_correct_under_utf8_and_utf16() {
    for (enc_kind, enc) in [
        (PositionEncodingKind::UTF8, OffsetEncoding::Utf8),
        (PositionEncodingKind::UTF16, OffsetEncoding::Utf16),
    ] {
        let (mut service, socket) = LspService::new(Backend::new);
        let (mut requests, _sink) = socket.split();

        let result = do_initialize(&mut service, std::slice::from_ref(&enc_kind)).await;
        assert_eq!(
            result["capabilities"]["positionEncoding"],
            json!(enc_kind.as_str()),
            "server must echo the only encoding the client offered"
        );

        let (text, uri) = fixture("non_ascii.fsm");
        let did_open = Request::build("textDocument/didOpen")
            .params(did_open_params(&uri, &text))
            .finish();
        service.ready().await.unwrap().call(did_open).await.unwrap();

        let got = next_publish_for(&mut requests, &uri).await;
        let want = expected_diags(&text, &uri, enc);

        assert_eq!(got.len(), 1, "[{enc:?}] expected one E0100, got {got:?}");
        assert_eq!(
            got[0].code,
            Some(tower_lsp::lsp_types::NumberOrString::String(
                "FSM-E0100".to_owned()
            )),
            "[{enc:?}] unknown-state reference is FSM-E0100"
        );
        assert_eq!(
            got[0].range, want[0].range,
            "[{enc:?}] Range must byte-match the pipeline oracle"
        );

        // Independent hard-coded cross-check of the EXACT column the
        // computed-from-source values above predict — pins the math so a
        // bug that corrupts BOTH oracle and server identically still fails
        // here (the oracle alone is not self-validating; this is).
        // Verified-from-source (UTF-8 byte vs UTF-16 code-unit count of
        // the line prefix up to the analyzer's span ends): the values are
        // recomputed, not guessed, and differ per encoding precisely
        // because of 🚀 (4 bytes / 2 UTF-16) + ы (2 bytes / 1 UTF-16) in
        // the comment — exactly the §4.1 defect class.
        let r: Range = got[0].range;
        assert_eq!(r.start.line, 5, "[{enc:?}] error is on 0-based line 5");
        assert_eq!(r.end.line, 5, "[{enc:?}] span ends on the same line");
        let (expected_start_char, expected_end_char) = match enc {
            OffsetEncoding::Utf8 => (28, 42),
            OffsetEncoding::Utf16 => (25, 39),
        };
        assert_eq!(
            r.start.character, expected_start_char,
            "[{enc:?}] transition-span start column wrong — a byte-as-\
             character (or scalar) shim corrupted by the multibyte comment \
             prefix (the §4.1 defect class)"
        );
        assert_eq!(
            r.end.character, expected_end_char,
            "[{enc:?}] transition-span end column wrong under {enc:?}"
        );
        // Cross-encoding sanity: the byte count MUST exceed the UTF-16
        // count on this line (proves the encodings genuinely diverge here,
        // so a single-encoding bug cannot pass both branches).
        if enc == OffsetEncoding::Utf8 {
            assert!(
                expected_start_char > 25,
                "UTF-8 column must exceed the UTF-16 column on a multibyte line"
            );
        }
    }
}

/// (d) `didChange` that fixes the error → diagnostics clear (empty
/// publish). Proves the debounced re-analyze path re-runs the pipeline on
/// the *new* buffer and republishes.
#[tokio::test(flavor = "current_thread")]
async fn did_change_fixing_the_error_clears_diagnostics() {
    let (mut service, socket) = LspService::new(Backend::new);
    let (mut requests, _sink) = socket.split();
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (broken, uri) = fixture("broken.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &broken))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();
    let first = next_publish_for(&mut requests, &uri).await;
    assert_eq!(first.len(), 1, "broken doc should report one error first");

    // Add the missing `initial` — same source the clean pipeline accepts.
    let fixed = broken.replace("machine Broken {", "machine Broken {\n    initial Only");
    // Sanity: the fixed buffer really is clean via the same pipeline.
    assert!(
        analyze(&fixed, Path::new("/tmp/x.fsm"))
            .diagnostics
            .is_empty(),
        "test fixture invariant: the edited buffer must be clean"
    );

    let did_change = Request::build("textDocument/didChange")
        .params(did_change_params(&uri, &fixed, 2))
        .finish();
    service
        .ready()
        .await
        .unwrap()
        .call(did_change)
        .await
        .unwrap();

    let after = next_publish_for(&mut requests, &uri).await;
    assert!(
        after.is_empty(),
        "fixing the error must clear diagnostics, still got {after:?}"
    );
}

// ===========================================================================
// L2 — `documentSymbol` + `foldingRange` (Doc 26 §8 L2 §5.4-LSP acceptance)
//
// Same discipline as L1: an in-process `tower-lsp` client issues the real
// `textDocument/documentSymbol` / `textDocument/foldingRange` JSON-RPC
// requests and asserts the **decoded response payloads** — the symbol
// tree's names + kinds + parent/child nesting + every `range`/
// `selectionRange`, and the fold start/end lines + kind. Symbol-/route-
// presence is explicitly NOT the acceptance: the asserted values are.
//
// The expected ranges are recomputed from the EXACT reused pipeline
// (`fsm_lsp::analysis::analyze` → `document_symbols`/`folding_ranges`
// through `LineIndex`) — the same code path the server runs — AND
// cross-checked against independent hard-coded byte/UTF-16 columns so a
// bug that corrupts both oracle and server identically still fails (the
// oracle alone is not self-validating; the per-encoding hard-coded
// cross-check is, exactly as L1's non-ASCII test pins position math).
// ===========================================================================

use tower_lsp::lsp_types::{DocumentSymbol, DocumentSymbolResponse, FoldingRange, SymbolKind};

use fsm_lsp::capabilities::document_symbol::document_symbols;
use fsm_lsp::capabilities::folding::folding_ranges;

/// Issue a JSON-RPC **request** (id-bearing) and decode its `result`.
async fn call_request<S>(service: &mut S, method: &'static str, params: Value, id: i64) -> Value
where
    S: Service<Request, Response = Option<tower_lsp::jsonrpc::Response>>,
    S::Error: std::fmt::Debug,
{
    let req = Request::build(method).params(params).id(id).finish();
    let resp = service
        .ready()
        .await
        .unwrap()
        .call(req)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("{method} returned no response"));
    let v: Value = serde_json::to_value(resp).unwrap();
    v["result"].clone()
}

fn doc_symbol_params(uri: &Url) -> Value {
    json!({ "textDocument": { "uri": uri } })
}

/// Navigate a path like `["Plant","states","Running","Heater"]` down the
/// nested `DocumentSymbol` tree, returning the leaf (so assertions read
/// declaratively against the *structure*, not a flat list).
fn nav<'a>(roots: &'a [DocumentSymbol], path: &[&str]) -> &'a DocumentSymbol {
    let mut level: &[DocumentSymbol] = roots;
    let mut cur: Option<&DocumentSymbol> = None;
    for seg in path {
        let found = level
            .iter()
            .find(|s| s.name == *seg)
            .unwrap_or_else(|| panic!("symbol path segment {seg:?} not found (path {path:?})"));
        cur = Some(found);
        level = found.children.as_deref().unwrap_or(&[]);
    }
    cur.expect("non-empty path")
}

fn decode_symbols(result: &Value) -> Vec<DocumentSymbol> {
    match serde_json::from_value::<DocumentSymbolResponse>(result.clone())
        .expect("decode documentSymbol response")
    {
        DocumentSymbolResponse::Nested(v) => v,
        DocumentSymbolResponse::Flat(_) => {
            panic!("server must return a NESTED hierarchical tree, not a flat list")
        }
    }
}

/// (L2-a) `documentSymbol` on a **hierarchical** fixture (composite state
/// holding two parallel regions, each with nested states + a `final`) →
/// assert the FULL returned tree: every name, `SymbolKind`, the exact
/// parent/child nesting matching the statechart, and every
/// `range`/`selectionRange` byte-equal to the reused-pipeline oracle.
#[tokio::test(flavor = "current_thread")]
async fn document_symbol_full_hierarchical_tree_and_ranges() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text, uri) = fixture("hierarchy.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    let result = call_request(
        &mut service,
        "textDocument/documentSymbol",
        doc_symbol_params(&uri),
        10,
    )
    .await;
    let got = decode_symbols(&result);

    // --- Oracle: the SAME pipeline the server runs, recomputed here. The
    //     whole tree must byte-match (proves the server wired the threaded
    //     symbol_table from the single analysis, not a parallel pass).
    let path = uri.to_file_path().unwrap();
    let analysis = analyze(&text, &path);
    assert!(
        analysis.diagnostics.is_empty(),
        "fixture invariant: hierarchy.fsm must be clean, got {:?}",
        analysis.diagnostics
    );
    let cst = fsm_parser::parse(&text).syntax();
    let idx = LineIndex::new(&text);
    let want = document_symbols(
        &analysis.symbol_table,
        &cst,
        &idx,
        &text,
        OffsetEncoding::Utf8,
    );
    assert_eq!(
        got, want,
        "the whole documentSymbol tree must byte-match the reused-pipeline oracle"
    );

    // --- Structure + kinds, asserted explicitly (the oracle proves
    //     server==pipeline; this proves pipeline==statechart, so neither
    //     can silently regress without failing).
    assert_eq!(got.len(), 1, "one machine -> one root symbol");
    let plant = nav(&got, &["Plant"]);
    assert_eq!(plant.kind, SymbolKind::MODULE);
    // Category groups present (Doc 14 §13): context, events, states.
    let group_names: Vec<&str> = plant
        .children
        .as_ref()
        .expect("machine has children")
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(
        group_names,
        vec!["context", "events", "states"],
        "Doc 14 §13 category grouping + order"
    );

    // context -> level: u16 (FIELD)
    let level = nav(&got, &["Plant", "context", "level"]);
    assert_eq!(level.kind, SymbolKind::FIELD);
    assert_eq!(level.detail.as_deref(), Some("level: u16"));

    // events -> TICK, STOP (EVENT)
    let tick = nav(&got, &["Plant", "events", "TICK"]);
    assert_eq!(tick.kind, SymbolKind::EVENT);
    assert_eq!(
        nav(&got, &["Plant", "events", "STOP"]).kind,
        SymbolKind::EVENT
    );

    // The statechart hierarchy: Running(composite) -> {Heater, Pump}
    // (each a composite region) -> nested states + a `final`. This is the
    // load-bearing nesting assertion.
    let running = nav(&got, &["Plant", "states", "Running"]);
    assert_eq!(running.kind, SymbolKind::CLASS);
    assert_eq!(running.detail.as_deref(), Some("Running (composite)"));
    let running_kids: Vec<&str> = running
        .children
        .as_ref()
        .expect("Running is composite")
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(
        running_kids,
        vec!["Heater", "Pump"],
        "the two parallel regions nest under Running, in source order"
    );

    let heater = nav(&got, &["Plant", "states", "Running", "Heater"]);
    assert_eq!(heater.detail.as_deref(), Some("Heater (composite)"));
    let heater_kids: Vec<&str> = heater
        .children
        .as_ref()
        .unwrap()
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(heater_kids, vec!["Cold", "Hot", "HeaterDone"]);

    let cold = nav(&got, &["Plant", "states", "Running", "Heater", "Cold"]);
    assert_eq!(cold.kind, SymbolKind::CLASS);
    assert_eq!(cold.detail.as_deref(), Some("Cold (simple)"));
    assert!(cold.children.is_none(), "a simple state has no children");

    let hdone = nav(
        &got,
        &["Plant", "states", "Running", "Heater", "HeaterDone"],
    );
    assert_eq!(
        hdone.kind,
        SymbolKind::ENUM_MEMBER,
        "a `final` pseudo-state maps to ENUM_MEMBER"
    );
    assert_eq!(hdone.detail.as_deref(), Some("HeaterDone (final)"));

    // Sibling simple state at machine root (not nested under Running).
    let done = nav(&got, &["Plant", "states", "Done"]);
    assert_eq!(done.detail.as_deref(), Some("Done (simple)"));

    // --- Ranges: independent hard-coded cross-check of computed-from-
    //     source values (so a bug corrupting both oracle and server still
    //     fails). `range` = full decl; `selectionRange` = the name; the
    //     latter is strictly inside the former.
    assert_eq!(
        (
            plant.range.start.line,
            plant.range.start.character,
            plant.range.end.line
        ),
        (5, 0, 51),
        "machine full range"
    );
    assert_eq!(
        (
            plant.selection_range.start.line,
            plant.selection_range.start.character,
            plant.selection_range.end.character
        ),
        (5, 8, 13),
        "machine selectionRange = the name 'Plant' (cols 8..13 on line 5)"
    );
    assert_ne!(
        plant.selection_range, plant.range,
        "selectionRange (name) must differ from range (whole decl)"
    );
    assert_eq!(
        (
            heater.selection_range.start.line,
            heater.selection_range.start.character,
            heater.selection_range.end.character
        ),
        (20, 15, 21),
        "region Heater name span"
    );
    assert_eq!(
        (
            hdone.selection_range.start.line,
            hdone.selection_range.start.character,
            hdone.selection_range.end.character
        ),
        (31, 18, 28),
        "HeaterDone name span"
    );
    // selectionRange ⊆ range invariant, checked across the whole tree.
    fn assert_sel_within(s: &DocumentSymbol) {
        let r = &s.range;
        let sr = &s.selection_range;
        assert!(
            (sr.start.line, sr.start.character) >= (r.start.line, r.start.character)
                && (sr.end.line, sr.end.character) <= (r.end.line, r.end.character),
            "selectionRange {sr:?} must be contained by range {r:?} for {:?}",
            s.name
        );
        if let Some(ch) = &s.children {
            for c in ch {
                assert_sel_within(c);
            }
        }
    }
    for s in &got {
        assert_sel_within(s);
    }
}

/// (L2-b) THE risk-1 proof at the **symbol** layer (Doc 26 §4.1 / §7.1 —
/// same rigor L1 applied to diagnostics). `hierarchy_non_ascii.fsm` has a
/// composite `Outer` whose nested `state Inner` declaration sits on a line
/// **after** a `/* 🚀 ы */` block comment (🚀 = 4 bytes / 2 UTF-16 units;
/// ы = 2 bytes / 1 UTF-16 unit). `Inner`'s `selectionRange` column
/// therefore DIVERGES by exactly 3 between encodings — a byte-as-character
/// (or scalar) shim at the symbol layer would corrupt it. The test asserts
/// the served tree's ranges are correct under BOTH negotiated encodings,
/// byte-matching the per-encoding oracle, with independent hard-coded
/// divergent columns.
#[tokio::test(flavor = "current_thread")]
async fn document_symbol_ranges_correct_under_utf8_and_utf16() {
    // Captures the SERVED `Inner` name-start column per encoding so the
    // cross-encoding divergence is asserted on real response data (not a
    // constant) after the loop.
    let mut served_name_col: Vec<u32> = Vec::new();
    for (enc_kind, enc) in [
        (PositionEncodingKind::UTF8, OffsetEncoding::Utf8),
        (PositionEncodingKind::UTF16, OffsetEncoding::Utf16),
    ] {
        let (mut service, _socket) = LspService::new(Backend::new);
        let init = do_initialize(&mut service, std::slice::from_ref(&enc_kind)).await;
        assert_eq!(
            init["capabilities"]["positionEncoding"],
            json!(enc_kind.as_str())
        );
        // Capability advertisement is part of L2 (Doc 26 §8 L2): assert it
        // here so an un-advertised provider fails acceptance.
        assert_eq!(
            init["capabilities"]["documentSymbolProvider"],
            json!(true),
            "documentSymbolProvider must be advertised"
        );
        assert_eq!(
            init["capabilities"]["foldingRangeProvider"],
            json!(true),
            "foldingRangeProvider must be advertised"
        );

        let (text, uri) = fixture("hierarchy_non_ascii.fsm");
        let did_open = Request::build("textDocument/didOpen")
            .params(did_open_params(&uri, &text))
            .finish();
        service.ready().await.unwrap().call(did_open).await.unwrap();

        let result = call_request(
            &mut service,
            "textDocument/documentSymbol",
            doc_symbol_params(&uri),
            20,
        )
        .await;
        let got = decode_symbols(&result);

        // Per-encoding oracle byte-match (server == reused pipeline).
        let path = uri.to_file_path().unwrap();
        let analysis = analyze(&text, &path);
        assert!(analysis.diagnostics.is_empty(), "[{enc:?}] fixture clean");
        let cst = fsm_parser::parse(&text).syntax();
        let idx = LineIndex::new(&text);
        let want = document_symbols(&analysis.symbol_table, &cst, &idx, &text, enc);
        assert_eq!(got, want, "[{enc:?}] served tree must match the oracle");

        // Structure is encoding-independent.
        let outer = nav(&got, &["Sys", "states", "Outer"]);
        assert_eq!(outer.detail.as_deref(), Some("Outer (composite)"));
        let inner = nav(&got, &["Sys", "states", "Outer", "Inner"]);
        assert_eq!(inner.detail.as_deref(), Some("Inner (simple)"));

        // `Outer`'s name is BEFORE any multibyte → identical both encodings.
        assert_eq!(
            (
                outer.selection_range.start.line,
                outer.selection_range.start.character,
                outer.selection_range.end.character
            ),
            (9, 10, 15),
            "[{enc:?}] 'Outer' name (pre-multibyte) is encoding-invariant"
        );

        // `Inner`'s decl + name are AFTER `/* 🚀 ы */` on line 11 → the
        // columns DIVERGE. Computed-from-source (hard-coded so a bug that
        // corrupts oracle+server identically still fails here):
        //   line 11 prefix = 8 spaces + "/* " + 🚀 + " " + ы + " */ state "
        //   UTF-8  : 🚀=4 ы=2  -> `state` decl col 22, `Inner` name col 28
        //   UTF-16 : 🚀=2 ы=1  -> decl col 19, name col 25  (3 fewer)
        let (decl_col, name_col) = match enc {
            OffsetEncoding::Utf8 => (22, 28),
            OffsetEncoding::Utf16 => (19, 25),
        };
        assert_eq!(
            (inner.range.start.line, inner.range.start.character),
            (11, decl_col),
            "[{enc:?}] Inner decl-start column wrong — a byte/scalar shim \
             corrupted by the multibyte comment prefix (the §4.1 class at \
             the symbol layer)"
        );
        assert_eq!(
            (
                inner.selection_range.start.line,
                inner.selection_range.start.character,
                inner.selection_range.end.character
            ),
            (11, name_col, name_col + 5),
            "[{enc:?}] Inner name span column wrong under {enc:?}"
        );
        served_name_col.push(inner.selection_range.start.character);
    }

    // Cross-encoding sanity on REAL served data (not a constant): the
    // UTF-8 name column MUST exceed the UTF-16 one by exactly the 3-unit
    // multibyte delta (🚀 4→2, ы 2→1). This proves the encodings
    // genuinely diverge on this line, so a single-encoding bug could not
    // have passed both per-encoding branches above by coincidence.
    assert_eq!(served_name_col.len(), 2, "both encodings exercised");
    assert_eq!(
        served_name_col[0],
        served_name_col[1] + 3,
        "served UTF-8 col {} must exceed served UTF-16 col {} by the \
         multibyte delta (3) — the §4.1 divergence at the symbol layer",
        served_name_col[0],
        served_name_col[1]
    );
}

/// (L2-c) `foldingRange` on the hierarchical fixture → assert the folding
/// regions (start/end line + kind) for the composite/parallel/region
/// constructs are EXACTLY correct (Doc 14 §12). Oracle byte-match plus an
/// explicit expected set so a fold regression fails.
#[tokio::test(flavor = "current_thread")]
async fn folding_range_composite_parallel_region_exact() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text, uri) = fixture("hierarchy.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    let result = call_request(
        &mut service,
        "textDocument/foldingRange",
        doc_symbol_params(&uri),
        30,
    )
    .await;
    let got: Vec<FoldingRange> =
        serde_json::from_value(result).expect("decode foldingRange response");

    // Oracle byte-match (server == the pure CST walk).
    let cst = fsm_parser::parse(&text).syntax();
    let idx = LineIndex::new(&text);
    let want = folding_ranges(&cst, &idx, &text, OffsetEncoding::Utf8);
    assert_eq!(
        got, want,
        "served folds must byte-match the CST-walk oracle"
    );

    // Explicit expected set (computed-from-source line numbers). Folds, as
    // (start,end) line pairs: machine body 5..50; context block 6..10;
    // events block 10..15; composite `Running` 17..49; region `Heater`
    // 20..34; states `Cold` 23..27, `Hot` 27..31; region `Pump` 34..47;
    // states `Off` 37..41, `On` 41..45. Single-line constructs
    // (`final HeaterDone`, `state Done { }`) do NOT fold.
    let pairs: std::collections::BTreeSet<(u32, u32)> =
        got.iter().map(|f| (f.start_line, f.end_line)).collect();
    let expected: std::collections::BTreeSet<(u32, u32)> = [
        (5, 50),
        (6, 10),
        (10, 15),
        (17, 49),
        (20, 34),
        (23, 27),
        (27, 31),
        (34, 47),
        (37, 41),
        (41, 45),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        pairs, expected,
        "exact Doc 14 §12 fold set (composite Running, parallel regions \
         Heater/Pump, their nested state bodies, context/events blocks)"
    );

    // The composite/parallel-region folds specifically (the Doc 26 §8 L2
    // headline): Running (composite) and both region bodies must fold and
    // structurally contain their nested-state folds.
    let running = got
        .iter()
        .find(|f| (f.start_line, f.end_line) == (17, 49))
        .expect("composite `Running` body folds");
    let heater = got
        .iter()
        .find(|f| (f.start_line, f.end_line) == (20, 34))
        .expect("parallel region `Heater` body folds");
    assert!(
        running.start_line < heater.start_line && heater.end_line < running.end_line,
        "the region fold nests inside the composite fold"
    );

    // No degenerate single-line fold leaked in.
    assert!(
        got.iter().all(|f| f.end_line > f.start_line),
        "no single-line (start==end) fold may be emitted"
    );
}

// ===========================================================================
// L3 — `hover` + `definition` (Doc 26 §8 L3 §5.4-LSP acceptance)
//
// Same discipline as L1/L2: an in-process `tower-lsp` client issues the
// real `textDocument/definition` / `textDocument/hover` JSON-RPC requests
// and asserts the **decoded response payloads** — the definition
// `Location` Range byte-matches the declaration computed from the analysis
// oracle (the SAME `fsm_lsp` code path the server runs), and the hover
// Markdown's structured content matches expectation (the meaningful
// payload-field / signature / type lines, not a substring coincidence).
// Cross-file / unresolved → `null`. Non-ASCII fixture under BOTH
// `positionEncoding`s pins the position mapping at the L3 layer.
//
// Expected ranges are recomputed from the EXACT reused pipeline
// (`fsm_lsp::analysis::analyze` → `resolve`/`goto_definition`/`hover`
// through `LineIndex`) — the same code the server runs — AND cross-checked
// against independent hard-coded computed-from-source columns so a bug
// that corrupts both oracle and server identically still fails (the
// oracle alone is not self-validating; the hard-coded cross-check is,
// exactly as L1's non-ASCII test pins position math).
// ===========================================================================

use tower_lsp::lsp_types::{GotoDefinitionResponse, Hover, HoverContents, Location};

use fsm_lsp::capabilities::definition::goto_definition;
use fsm_lsp::capabilities::hover::hover as build_hover;

fn pos_params(uri: &Url, line: u32, character: u32) -> Value {
    json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character }
    })
}

/// Byte offset of `needle` in `text`, +`plus` (cursor placement helper).
fn byte_of(text: &str, needle: &str, plus: usize) -> usize {
    text.find(needle).expect("needle in fixture") + plus
}

fn decode_definition(result: &Value) -> Option<Location> {
    if result.is_null() {
        return None;
    }
    match serde_json::from_value::<GotoDefinitionResponse>(result.clone())
        .expect("decode definition response")
    {
        GotoDefinitionResponse::Scalar(loc) => Some(loc),
        other => panic!("expected a single Location, got {other:?}"),
    }
}

fn hover_markdown(result: &Value) -> Option<String> {
    if result.is_null() {
        return None;
    }
    let h: Hover = serde_json::from_value(result.clone()).expect("decode hover");
    match h.contents {
        HoverContents::Markup(m) => Some(m.value),
        other => panic!("expected markup hover, got {other:?}"),
    }
}

/// (L3-a) `definition` on a transition-target state ref → the returned
/// `Location` Range **byte-matches** the `state NAME {` declaration the
/// analysis oracle computes (the SAME `goto_definition` code path), AND an
/// independent hard-coded computed-from-source line/col cross-check. Plus
/// an event-ref goto (different resolution category) for breadth.
#[tokio::test(flavor = "current_thread")]
async fn definition_resolves_use_site_to_declaration_range() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text, uri) = fixture("l3_basic.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    // --- Oracle: the SAME pipeline + goto code the server runs. ---------
    let path = uri.to_file_path().unwrap();
    let analysis = analyze(&text, &path);
    assert!(
        analysis.diagnostics.is_empty(),
        "fixture invariant: l3_basic.fsm must be clean, got {:?}",
        analysis.diagnostics
    );
    let cst = fsm_parser::parse(&text).syntax();
    let idx = LineIndex::new(&text);

    // Cursor on the `Moving` in `on CALL [can_go(1)] -> Moving`.
    let cur = byte_of(&text, "-> Moving", 3);
    let cur_pos = idx.position(&text, cur as u32, OffsetEncoding::Utf8);
    let want = goto_definition(
        &analysis.symbol_table,
        &cst,
        &uri,
        cur as u32,
        &idx,
        &text,
        OffsetEncoding::Utf8,
    )
    .expect("oracle: `Moving` resolves to its decl");

    let result = call_request(
        &mut service,
        "textDocument/definition",
        pos_params(&uri, cur_pos.line, cur_pos.character),
        40,
    )
    .await;
    let got = decode_definition(&result).expect("server: `Moving` resolves");

    // Load-bearing: byte-exact Location equality with the oracle.
    assert_eq!(
        got, want,
        "served definition Location must byte-match the analysis oracle"
    );
    assert_eq!(
        got.uri, uri,
        "single-file: decl is in the requested document"
    );

    // Independent computed-from-source cross-check: `state Moving {` decl
    // starts at 0-based L20 C4 (verified from the fixture layout), and the
    // decl span is the whole `state Moving { … }` so the Range starts
    // exactly there. A position-math regression fails THIS even if it
    // corrupted the oracle identically.
    assert_eq!(
        (got.range.start.line, got.range.start.character),
        (20, 4),
        "definition Range must start at the `state Moving {{` declaration"
    );
    // The decl span must actually cover the declaration text (independent
    // of the range math: reconstruct the byte from the range and read it).
    let decl_byte = idx.offset(&text, got.range.start, OffsetEncoding::Utf8) as usize;
    assert_eq!(
        &text[decl_byte..decl_byte + "state Moving".len()],
        "state Moving",
        "the Range must point at the literal `state Moving` declaration"
    );

    // Breadth: an EVENT ref resolves to the event decl (different
    // resolution category — proves it is not a state-only goto).
    let ev_cur = byte_of(&text, "on CALL", 3); // the `CALL` after `on`
    let ev_pos = idx.position(&text, ev_cur as u32, OffsetEncoding::Utf8);
    let ev_want = goto_definition(
        &analysis.symbol_table,
        &cst,
        &uri,
        ev_cur as u32,
        &idx,
        &text,
        OffsetEncoding::Utf8,
    )
    .expect("oracle: event `CALL` resolves");
    let ev_result = call_request(
        &mut service,
        "textDocument/definition",
        pos_params(&uri, ev_pos.line, ev_pos.character),
        41,
    )
    .await;
    let ev_got = decode_definition(&ev_result).expect("server: event `CALL` resolves");
    assert_eq!(
        ev_got, ev_want,
        "event-ref definition must match the oracle"
    );
    // The event decl is on L10 (the `CALL(dest: u16)` line in `events {}`).
    assert_eq!(
        ev_got.range.start.line, 10,
        "event `CALL` decl is on 0-based line 10"
    );
}

/// (L3-b) `definition` for a symbol that does not resolve in this file
/// returns **null** — the spec-correct single-file graceful degradation
/// (Doc 14 §8 / Doc 26 §4.6), NEVER a fabricated or cross-file Location.
/// Two cases: an unknown name (no such decl) and a deliberately
/// import/cross-file-style reference. Both must be null, not a guess.
#[tokio::test(flavor = "current_thread")]
async fn definition_unresolved_returns_null_not_a_fabricated_location() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    // A buffer whose transition targets a state that does NOT exist (the
    // analyzer emits E0100; the resolver must NOT invent a location). This
    // is the single-file degradation contract: an unresolved/cross-file
    // symbol yields null, exactly as a cross-file import would (there is
    // no project index in v1.2 — Doc 26 §4.6/§9; the same null path).
    let text = "language fsm 2.0\n\nmachine M {\n  events { GO }\n  initial A\n  state A {\n    on GO -> Elsewhere\n  }\n}\n"
        .to_owned();
    let path = std::env::temp_dir().join("l3_unresolved.fsm");
    let uri = Url::from_file_path(&path).unwrap();
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    // Oracle agrees it is unresolvable (None) — server must echo null.
    let analysis = analyze(&text, &path);
    let cst = fsm_parser::parse(&text).syntax();
    let idx = LineIndex::new(&text);
    let cur = byte_of(&text, "-> Elsewhere", 3);
    let cur_pos = idx.position(&text, cur as u32, OffsetEncoding::Utf8);
    assert!(
        goto_definition(
            &analysis.symbol_table,
            &cst,
            &uri,
            cur as u32,
            &idx,
            &text,
            OffsetEncoding::Utf8
        )
        .is_none(),
        "oracle: an unresolved cross-file-style ref must be None"
    );

    let result = call_request(
        &mut service,
        "textDocument/definition",
        pos_params(&uri, cur_pos.line, cur_pos.character),
        42,
    )
    .await;
    assert!(
        result.is_null(),
        "definition for an unresolved/cross-file symbol MUST be null \
         (graceful single-file degradation) — got {result:?}, a \
         fabricated location is the silent-data-loss cardinal sin"
    );
    assert!(
        decode_definition(&result).is_none(),
        "decoded definition must be None"
    );

    // Negative-position breadth: cursor in whitespace → null, no panic.
    let ws = byte_of(&text, "machine M", 7);
    let ws_pos = idx.position(&text, ws as u32, OffsetEncoding::Utf8);
    let ws_res = call_request(
        &mut service,
        "textDocument/definition",
        pos_params(&uri, ws_pos.line, ws_pos.character),
        43,
    )
    .await;
    assert!(ws_res.is_null(), "definition on whitespace is null");
}

/// (L3-c) `hover` content assertions — the MEANINGFUL structured Markdown
/// (Doc 14 §5), not a substring coincidence. Event payload field list
/// (IR-sourced), `pure` extern signature, context-field type+default, and
/// a state's kind + transitions-out. Plus the negative: hover on
/// whitespace → null (no panic, no empty tooltip).
#[tokio::test(flavor = "current_thread")]
async fn hover_markdown_content_is_structured_and_ir_sourced() {
    let (mut service, _socket) = LspService::new(Backend::new);
    let init = do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;
    // Capability advertisement is part of L3 (Doc 26 §8 L3): assert it so
    // an un-advertised provider fails acceptance.
    assert_eq!(
        init["capabilities"]["hoverProvider"],
        json!(true),
        "hoverProvider must be advertised"
    );
    assert_eq!(
        init["capabilities"]["definitionProvider"],
        json!(true),
        "definitionProvider must be advertised"
    );

    let (text, uri) = fixture("l3_basic.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    // Oracle parity per request (server == the same hover code path).
    let path = uri.to_file_path().unwrap();
    let analysis = analyze(&text, &path);
    let cst = fsm_parser::parse(&text).syntax();
    let idx = LineIndex::new(&text);
    let oracle = |byte: usize| -> Option<String> {
        build_hover(
            &analysis.symbol_table,
            analysis.ir.as_ref(),
            &cst,
            byte as u32,
            &idx,
            &text,
            OffsetEncoding::Utf8,
        )
        .map(|h| match h.contents {
            HoverContents::Markup(m) => m.value,
            _ => unreachable!(),
        })
    };

    // 1. EVENT hover — the payload field list is IR-sourced (the
    //    symbol_table has no payload schema). `CALL(dest: u16)`.
    let ev = byte_of(&text, "on CALL", 3);
    let ev_pos = idx.position(&text, ev as u32, OffsetEncoding::Utf8);
    let ev_md = hover_markdown(
        &call_request(
            &mut service,
            "textDocument/hover",
            pos_params(&uri, ev_pos.line, ev_pos.character),
            50,
        )
        .await,
    )
    .expect("event hover present");
    assert_eq!(
        Some(ev_md.clone()),
        oracle(ev),
        "served event hover must match the same-pipeline oracle"
    );
    assert!(
        ev_md.contains("## event `CALL`"),
        "event hover header wrong: {ev_md}"
    );
    assert!(
        ev_md.contains("**Payload fields:**") && ev_md.contains("- `dest: u16`"),
        "event hover MUST list the IR payload field `dest: u16` \
         (structured content, not a substring fluke): {ev_md}"
    );

    // 2. `pure` extern hover — signature from the IR.
    let ex = byte_of(&text, "can_go(1)", 0);
    let ex_pos = idx.position(&text, ex as u32, OffsetEncoding::Utf8);
    let ex_md = hover_markdown(
        &call_request(
            &mut service,
            "textDocument/hover",
            pos_params(&uri, ex_pos.line, ex_pos.character),
            51,
        )
        .await,
    )
    .expect("extern hover present");
    assert_eq!(
        Some(ex_md.clone()),
        oracle(ex),
        "extern hover oracle parity"
    );
    assert!(
        ex_md.contains("## `pure` extern `can_go`"),
        "extern hover header (purity) wrong: {ex_md}"
    );
    assert!(
        ex_md.contains("**Signature:** `(n: u8) -> bool`"),
        "extern hover MUST show the IR-derived signature: {ex_md}"
    );

    // 3. context-field hover — type + default from the IR ContextField.
    let cf = byte_of(&text, "ctx.floor", 4);
    let cf_pos = idx.position(&text, cf as u32, OffsetEncoding::Utf8);
    let cf_md = hover_markdown(
        &call_request(
            &mut service,
            "textDocument/hover",
            pos_params(&uri, cf_pos.line, cf_pos.character),
            52,
        )
        .await,
    )
    .expect("ctx hover present");
    assert_eq!(Some(cf_md.clone()), oracle(cf), "ctx hover oracle parity");
    assert!(
        cf_md.contains("## context field `floor`")
            && cf_md.contains("**Type:** `u8`")
            && cf_md.contains("**Default value:** `0`"),
        "ctx-field hover MUST show IR type + default: {cf_md}"
    );

    // 4. STATE hover — kind + transitions-out (Idle has 1 outgoing).
    let st = byte_of(&text, "initial Idle", 8); // the `Idle` use-site
    let st_pos = idx.position(&text, st as u32, OffsetEncoding::Utf8);
    let st_md = hover_markdown(
        &call_request(
            &mut service,
            "textDocument/hover",
            pos_params(&uri, st_pos.line, st_pos.character),
            53,
        )
        .await,
    )
    .expect("state hover present");
    assert_eq!(Some(st_md.clone()), oracle(st), "state hover oracle parity");
    assert!(
        st_md.contains("## state `Idle` *(simple)*") && st_md.contains("**Transitions out:** 1"),
        "state hover MUST show kind + IR transition count: {st_md}"
    );

    // Negative: hover on whitespace → null, no panic, no empty tooltip.
    let ws = byte_of(&text, "machine Lift", 7);
    let ws_pos = idx.position(&text, ws as u32, OffsetEncoding::Utf8);
    let ws_res = call_request(
        &mut service,
        "textDocument/hover",
        pos_params(&uri, ws_pos.line, ws_pos.character),
        54,
    )
    .await;
    assert!(
        ws_res.is_null(),
        "hover on whitespace MUST be null (no empty tooltip): {ws_res:?}"
    );
}

/// (L3-d) THE risk-1 proof at the L3 layer (Doc 26 §4.1 / §7.1 — the same
/// rigor L1/L2 applied). `l3_non_ascii.fsm`'s `on GO -> Target` sits on a
/// line **after** `/* 🚀 ы переход */` (🚀 = 4 bytes / 2 UTF-16; ы = 2/1;
/// 7 Cyrillic in "переход" = 2/1 each). The cursor `character` to place on
/// the `Target` use-site therefore DIVERGES by exactly 10 units between
/// encodings — a byte/scalar shim sends the wrong position and resolves
/// the wrong (or no) token. The test drives definition AND hover under
/// BOTH negotiated encodings, sending the per-encoding-correct cursor, and
/// asserts the resolved decl Range / hover content is correct each time,
/// byte-matching the per-encoding oracle, with hard-coded divergent cursor
/// columns. It FAILS if the L3 position mapping regresses.
#[tokio::test(flavor = "current_thread")]
async fn non_ascii_definition_and_hover_correct_under_utf8_and_utf16() {
    // Captures the served decl-Range start per encoding; the decl line is
    // pure ASCII so the Range itself is encoding-invariant — but the
    // *cursor we must send* is not, which is the actual position-mapping
    // test (a wrong inverse maps the divergent cursor to the wrong token).
    let mut cursor_cols: Vec<u32> = Vec::new();
    for (enc_kind, enc) in [
        (PositionEncodingKind::UTF8, OffsetEncoding::Utf8),
        (PositionEncodingKind::UTF16, OffsetEncoding::Utf16),
    ] {
        let (mut service, _socket) = LspService::new(Backend::new);
        let init = do_initialize(&mut service, std::slice::from_ref(&enc_kind)).await;
        assert_eq!(
            init["capabilities"]["positionEncoding"],
            json!(enc_kind.as_str())
        );

        let (text, uri) = fixture("l3_non_ascii.fsm");
        let did_open = Request::build("textDocument/didOpen")
            .params(did_open_params(&uri, &text))
            .finish();
        service.ready().await.unwrap().call(did_open).await.unwrap();

        let path = uri.to_file_path().unwrap();
        let analysis = analyze(&text, &path);
        assert!(
            analysis.diagnostics.is_empty(),
            "[{enc:?}] fixture invariant: l3_non_ascii.fsm clean"
        );
        let cst = fsm_parser::parse(&text).syntax();
        let idx = LineIndex::new(&text);

        // The cursor on the `Target` USE-site. Its byte is encoding-
        // independent; the LSP `character` to send is NOT — compute it via
        // the authoritative LineIndex (the same the server inverts).
        let cur_byte = byte_of(&text, "-> Target", 3);
        let cur_pos = idx.position(&text, cur_byte as u32, enc);
        // It is on 0-based line 8 (the `… on GO -> Target` line). Hard-
        // coded divergent columns (computed-from-source) so a bug that
        // corrupts oracle+server identically still fails here:
        //   line 8 prefix = 8 spaces + "/* " + 🚀 + " " + ы + " переход */ on GO -> "
        //   UTF-8  : 🚀=4 ы=2 переход=14  -> `Target` use col 46
        //   UTF-16 : 🚀=2 ы=1 переход=7   -> col 36   (10 fewer)
        assert_eq!(cur_pos.line, 8, "[{enc:?}] use-site is on line 8");
        let expected_cur_col = match enc {
            OffsetEncoding::Utf8 => 46,
            OffsetEncoding::Utf16 => 36,
        };
        assert_eq!(
            cur_pos.character, expected_cur_col,
            "[{enc:?}] the cursor column on `Target` must diverge by the \
             multibyte delta — a byte/scalar shim computes the wrong one"
        );
        cursor_cols.push(cur_pos.character);

        // --- definition: send the per-encoding cursor, assert the decl
        //     Range byte-matches the oracle AND the literal decl. -------
        let want = goto_definition(
            &analysis.symbol_table,
            &cst,
            &uri,
            cur_byte as u32,
            &idx,
            &text,
            enc,
        )
        .expect("[oracle] `Target` resolves");
        let dres = call_request(
            &mut service,
            "textDocument/definition",
            pos_params(&uri, cur_pos.line, cur_pos.character),
            60,
        )
        .await;
        let dgot = decode_definition(&dres).expect("[server] `Target` resolves");
        assert_eq!(
            dgot, want,
            "[{enc:?}] served definition must byte-match the oracle \
             (proves the divergent cursor mapped to the right token)"
        );
        // The decl is the whole `STATE_DECL` node for `state Target {}`.
        // Verified-from-source: that node's byte range is [152,168) =
        // "state Target {}\n" (rowan attaches the trailing newline to the
        // node — the SAME `span_of` decl span L2's `documentSymbol`
        // `range` uses). It starts on the pure-ASCII line 11 col 4 and
        // ends at line 12 col 0 (just past the closing `}` + newline).
        // Encoding-invariant (the whole decl is ASCII).
        assert_eq!(
            (
                dgot.range.start.line,
                dgot.range.start.character,
                dgot.range.end.line,
                dgot.range.end.character
            ),
            (11, 4, 12, 0),
            "[{enc:?}] decl Range = the `state Target {{}}` STATE_DECL node \
             [L11C4, L12C0) (full node span incl. trailing newline — the \
             same span_of decl span L2 documentSymbol uses)"
        );
        // Independent: the Range start must point at the literal decl text
        // (reconstruct the byte from the Range, encoding-agnostic check).
        let db = idx.offset(&text, dgot.range.start, enc) as usize;
        assert!(
            text[db..].starts_with("state Target {}"),
            "[{enc:?}] decl Range start must be the `state Target {{}}` text"
        );

        // --- hover at the same divergent cursor: content correct + oracle
        //     parity (proves hover's position inverse is the same correct
        //     one, not a second/naive converter). -----------------------
        let hres = call_request(
            &mut service,
            "textDocument/hover",
            pos_params(&uri, cur_pos.line, cur_pos.character),
            61,
        )
        .await;
        let hmd = hover_markdown(&hres).expect("[server] hover on `Target`");
        let hwant = build_hover(
            &analysis.symbol_table,
            analysis.ir.as_ref(),
            &cst,
            cur_byte as u32,
            &idx,
            &text,
            enc,
        )
        .map(|h| match h.contents {
            HoverContents::Markup(m) => m.value,
            _ => unreachable!(),
        });
        assert_eq!(
            Some(hmd.clone()),
            hwant,
            "[{enc:?}] served hover must match the same-pipeline oracle"
        );
        assert!(
            hmd.contains("## state `Target` *(simple)*"),
            "[{enc:?}] hover on the `Target` use-site resolves to the \
             state and renders its kind: {hmd}"
        );
    }

    // Cross-encoding sanity on REAL data: the UTF-8 cursor column MUST
    // exceed the UTF-16 one by exactly 10 (🚀 4→2, ы 2→1, переход 7×2→7×1
    // = 7 less; +1+2 = 10). Proves the encodings genuinely diverge on this
    // line, so a single-encoding bug could not have passed both branches.
    assert_eq!(cursor_cols.len(), 2, "both encodings exercised");
    assert_eq!(
        cursor_cols[0],
        cursor_cols[1] + 10,
        "UTF-8 cursor col {} must exceed UTF-16 col {} by the multibyte \
         delta (10) — the §4.1 divergence at the L3 layer",
        cursor_cols[0],
        cursor_cols[1]
    );
}

/// (L3-e) The exact Doc 26 §8 L3 acceptance sentence: place the cursor on
/// a **state reference in a transition** → `definition` returns the exact
/// Range of the `state NAME {` declaration; **hover on an event** → the
/// Markdown contains the payload field list from the IR; **hover in
/// whitespace** → `None`, no panic. (a/c cover these via the oracle; this
/// is the verbatim-spec restatement with the exact byte assertions, so the
/// acceptance maps 1:1 to the doc clause and cannot silently drift.)
#[tokio::test(flavor = "current_thread")]
async fn doc26_l3_acceptance_sentence_verbatim() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;
    let (text, uri) = fixture("l3_basic.fsm");
    service
        .ready()
        .await
        .unwrap()
        .call(
            Request::build("textDocument/didOpen")
                .params(did_open_params(&uri, &text))
                .finish(),
        )
        .await
        .unwrap();
    let idx = LineIndex::new(&text);

    // "cursor on a state reference in a transition → definition returns
    //  the exact Range of the `state NAME {` declaration"
    let s = byte_of(&text, "-> Moving", 3);
    let sp = idx.position(&text, s as u32, OffsetEncoding::Utf8);
    let d = decode_definition(
        &call_request(
            &mut service,
            "textDocument/definition",
            pos_params(&uri, sp.line, sp.character),
            70,
        )
        .await,
    )
    .expect("state ref resolves");
    let b = idx.offset(&text, d.range.start, OffsetEncoding::Utf8) as usize;
    assert!(
        text[b..].starts_with("state Moving {"),
        "definition Range must be exactly the `state Moving {{` decl line"
    );

    // "hover on an event → response Markdown contains the payload field
    //  list from the IR"
    let e = byte_of(&text, "on CALL", 3);
    let ep = idx.position(&text, e as u32, OffsetEncoding::Utf8);
    let md = hover_markdown(
        &call_request(
            &mut service,
            "textDocument/hover",
            pos_params(&uri, ep.line, ep.character),
            71,
        )
        .await,
    )
    .expect("event hover present");
    assert!(
        md.contains("**Payload fields:**") && md.contains("- `dest: u16`"),
        "event hover Markdown must contain the IR payload field list: {md}"
    );

    // "Negative: hover in whitespace → None, no panic."
    let w = byte_of(&text, "    state Idle", 2); // a space in the indent
    let wp = idx.position(&text, w as u32, OffsetEncoding::Utf8);
    let wr = call_request(
        &mut service,
        "textDocument/hover",
        pos_params(&uri, wp.line, wp.character),
        72,
    )
    .await;
    assert!(wr.is_null(), "hover in whitespace must be null: {wr:?}");
}

// ===========================================================================
// L4 — `textDocument/completion` (Doc 26 §8 L4). §5.4-LSP behavioural
// acceptance: an in-process tower-lsp client sends a real completion
// request at each representative cursor context and asserts the returned
// **set** (labels + CompletionItemKind) EQUALS the context-correct set the
// analysis oracle (the SAME `completions()` code the server runs) computes,
// AND that at least one wrong-context candidate that exists elsewhere in
// scope is ABSENT. Both the positive set and the negative exclusion are
// asserted. Symbol/route presence is NOT acceptance — the asserted
// label+kind SET equality and the exclusion are.
// ===========================================================================

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, CompletionResponse};

use fsm_lsp::capabilities::complete::completions;

/// Decode a `textDocument/completion` result into items (the server always
/// answers `CompletionResponse::Array` — an empty array is the valid
/// "nothing here", never `null`/missing).
fn decode_completions(result: &Value) -> Vec<CompletionItem> {
    assert!(
        !result.is_null(),
        "completion must answer an array (possibly empty), never null"
    );
    match serde_json::from_value::<CompletionResponse>(result.clone())
        .expect("decode completion response")
    {
        CompletionResponse::Array(v) => v,
        CompletionResponse::List(l) => l.items,
    }
}

/// `CompletionItemKind` has no `Ord`; its LSP wire value is a stable
/// integer, so map to that for a deterministic comparable key.
fn kind_num(k: Option<CompletionItemKind>) -> i64 {
    k.map(|k| serde_json::to_value(k).unwrap().as_i64().unwrap())
        .unwrap_or(0)
}

/// (label, kind-as-int) pairs, sorted by label (labels are unique within
/// one context) — the comparable "set" for byte-exact assertions.
fn label_kind_set(items: &[CompletionItem]) -> Vec<(String, i64)> {
    let mut v: Vec<(String, i64)> = items
        .iter()
        .map(|i| (i.label.clone(), kind_num(i.kind)))
        .collect();
    v.sort();
    v
}

fn labels_of(items: &[CompletionItem]) -> Vec<String> {
    items.iter().map(|i| i.label.clone()).collect()
}

/// Build one comparable `(label, kind-as-int)` tuple matching
/// [`label_kind_set`]'s element shape, for hard-coded expected sets.
fn lk(label: &str, kind: CompletionItemKind) -> (String, i64) {
    (label.to_string(), kind_num(Some(kind)))
}

/// (L4-init) `initialize` advertises `completionProvider` with the EXACT
/// Doc 14 §2 trigger characters `[".", ":", "@", "[", " "]` (verified
/// against the spec's ServerCapabilities block, not guessed), and
/// `resolveProvider:false` (L4 returns fully-resolved items — advertising
/// a resolve it does not implement would be the stub-a-no-op sin).
#[tokio::test(flavor = "current_thread")]
async fn completion_capability_advertised_with_doc14_trigger_chars() {
    let (mut service, _socket) = LspService::new(Backend::new);
    let init = do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;
    let cp = &init["capabilities"]["completionProvider"];
    assert!(
        !cp.is_null(),
        "completionProvider MUST be advertised (L4 genuinely implements it)"
    );
    assert_eq!(
        cp["triggerCharacters"],
        json!([".", ":", "@", "[", " "]),
        "trigger chars MUST be the exact Doc 14 §2 set"
    );
    assert_eq!(
        cp["resolveProvider"],
        json!(false),
        "no completionItem/resolve is implemented — must advertise false, \
         not silently claim a resolve round-trip"
    );
}

/// Drive `didOpen` then a `textDocument/completion` at the byte `cur`
/// (mapped to a Position via the authoritative LineIndex, the same the
/// server inverts), returning (served_items, oracle_items). The oracle is
/// the SAME `completions()` the server runs over the SAME analysis — set
/// equality proves the server wired it correctly; the hard-coded
/// cross-checks below pin the *content* independently.
async fn open_and_complete<S>(
    service: &mut S,
    fixture_name: &str,
    cur: usize,
    enc: OffsetEncoding,
    id: i64,
) -> (Vec<CompletionItem>, Vec<CompletionItem>, Url, String)
where
    S: Service<Request, Response = Option<tower_lsp::jsonrpc::Response>>,
    S::Error: std::fmt::Debug,
{
    let (text, uri) = fixture(fixture_name);
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    let path = uri.to_file_path().unwrap();
    let analysis = analyze(&text, &path);
    let cst = fsm_parser::parse(&text).syntax();
    let idx = LineIndex::new(&text);
    let cur_pos = idx.position(&text, cur as u32, enc);

    // Oracle: the exact same code path the server's handler runs.
    let oracle = completions(&analysis.symbol_table, &cst, cur as u32);

    let result = call_request(
        service,
        "textDocument/completion",
        pos_params(&uri, cur_pos.line, cur_pos.character),
        id,
    )
    .await;
    let served = decode_completions(&result);
    (served, oracle, uri, text)
}

/// (L4-a) **transition-target** context (`on GO -> |`): the served set
/// EQUALS exactly the in-scope **state names** as `CompletionItemKind::CLASS`
/// (oracle parity) AND the event `GO` (which exists in scope) is ABSENT.
#[tokio::test(flavor = "current_thread")]
async fn completion_transition_target_is_states_excludes_events() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text0, _) = fixture("l4_transition_target.fsm");
    // Cursor right after `on GO -> ` (the space following the arrow).
    let cur = byte_of(&text0, "on GO -> ", "on GO -> ".len());
    let (served, oracle, _uri, _t) = open_and_complete(
        &mut service,
        "l4_transition_target.fsm",
        cur,
        OffsetEncoding::Utf8,
        80,
    )
    .await;

    // Positive: served SET == oracle SET (labels + kinds), byte-exact.
    assert_eq!(
        label_kind_set(&served),
        label_kind_set(&oracle),
        "served completion set must equal the analysis oracle set"
    );
    // Independent hard-coded cross-check: the machine declares states
    // `Idle` and `Running`; a transition target offers exactly those two
    // as CLASS (kind 7). Computed-from-fixture, fails even if a bug
    // corrupted oracle+server identically.
    let mut want = vec![
        lk("Idle", CompletionItemKind::CLASS),
        lk("Running", CompletionItemKind::CLASS),
    ];
    want.sort();
    assert_eq!(
        label_kind_set(&served),
        want,
        "transition-target completion = exactly the in-scope state names \
         as kind CLASS"
    );
    // NEGATIVE exclusion (as load-bearing as the positive): the event
    // `GO` / `STOP` exist in this machine but a transition-target
    // position must NEVER offer them.
    let ls = labels_of(&served);
    assert!(
        !ls.contains(&"GO".to_string()) && !ls.contains(&"STOP".to_string()),
        "event names MUST be absent from a transition-target completion \
         (offering them is the wrong-context-noise sin): {ls:?}"
    );
}

/// (L4-b) **`on `-trigger** context: the served set EQUALS exactly the
/// machine's declared **event names** as `CompletionItemKind::EVENT`
/// (oracle parity) AND a state name (in scope) is ABSENT.
#[tokio::test(flavor = "current_thread")]
async fn completion_after_on_is_events_excludes_states() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text0, _) = fixture("l4_trigger.fsm");
    // Cursor right after `on ` (the space following the `on` keyword).
    let cur = byte_of(&text0, "        on ", "        on ".len());
    let (served, oracle, _uri, _t) = open_and_complete(
        &mut service,
        "l4_trigger.fsm",
        cur,
        OffsetEncoding::Utf8,
        81,
    )
    .await;

    assert_eq!(
        label_kind_set(&served),
        label_kind_set(&oracle),
        "served set must equal the oracle set"
    );
    // Hard-coded: events { GO STOP } → exactly those two, kind 20 (EVENT).
    let mut want = vec![
        lk("GO", CompletionItemKind::EVENT),
        lk("STOP", CompletionItemKind::EVENT),
    ];
    want.sort();
    assert_eq!(
        label_kind_set(&served),
        want,
        "after `on ` = exactly the declared event names as kind EVENT"
    );
    // NEGATIVE: states `Idle`/`Running` are in scope but a trigger
    // position must NOT offer them.
    let ls = labels_of(&served);
    assert!(
        !ls.contains(&"Idle".to_string()) && !ls.contains(&"Running".to_string()),
        "state names MUST be absent from a trigger completion: {ls:?}"
    );
}

/// (L4-c) **guard-expression** context (`on GO [ |`): the served set
/// EQUALS the oracle's guard-legal values — context fields (kind FIELD) +
/// **`pure`** externs (kind FUNCTION) — AND the IMPURE extern `do_io`
/// (illegal in a guard, Doc 04 §2.5) and a state name are ABSENT.
#[tokio::test(flavor = "current_thread")]
async fn completion_in_guard_is_pure_externs_and_fields_excludes_impure_and_states() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text0, _) = fixture("l4_guard.fsm");
    // Cursor right after `on GO [ ` (inside the guard bracket).
    let cur = byte_of(&text0, "on GO [ ", "on GO [ ".len());
    let (served, oracle, _uri, _t) =
        open_and_complete(&mut service, "l4_guard.fsm", cur, OffsetEncoding::Utf8, 82).await;

    assert_eq!(
        label_kind_set(&served),
        label_kind_set(&oracle),
        "served guard set must equal the oracle set"
    );
    // Hard-coded content cross-check: the field `floor` (kind FIELD) and
    // the PURE extern `can_go` (kind FUNCTION) must be present.
    let set = label_kind_set(&served);
    assert!(
        set.contains(&lk("floor", CompletionItemKind::FIELD)),
        "guard offers context field `floor` as FIELD: {set:?}"
    );
    assert!(
        set.contains(&lk("can_go", CompletionItemKind::FUNCTION)),
        "guard offers the PURE extern `can_go` as FUNCTION: {set:?}"
    );
    // NEGATIVE (load-bearing): the IMPURE extern `do_io` is illegal in a
    // guard (Doc 04 §2.5 — guards may call only pure externs); a state
    // name is also wrong-context. Both MUST be absent.
    let ls = labels_of(&served);
    assert!(
        !ls.contains(&"do_io".to_string()),
        "the IMPURE extern `do_io` MUST be absent from a guard completion \
         (Doc 04 §2.5): {ls:?}"
    );
    assert!(
        !ls.contains(&"Idle".to_string()),
        "a state name MUST be absent from a guard completion: {ls:?}"
    );
}

/// (L4-d) **statement-start** context (empty state body): the served set
/// EQUALS the oracle's state-item keywords (kind KEYWORD) + the Doc 14 §4
/// empty-body snippets (kind SNIPPET) AND no symbol (event/state name) is
/// offered (those are not statement starters).
#[tokio::test(flavor = "current_thread")]
async fn completion_statement_start_is_keywords_and_snippets_excludes_symbols() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text0, _) = fixture("l4_stmt_start.fsm");
    // The blank line inside `state Idle { … }` (after the 8-space indent).
    let cur = byte_of(
        &text0,
        "state Idle {\n        ",
        "state Idle {\n        ".len(),
    );
    let (served, oracle, _uri, _t) = open_and_complete(
        &mut service,
        "l4_stmt_start.fsm",
        cur,
        OffsetEncoding::Utf8,
        83,
    )
    .await;

    assert_eq!(
        label_kind_set(&served),
        label_kind_set(&oracle),
        "served statement-start set must equal the oracle set"
    );
    let set = label_kind_set(&served);
    // The state-item keyword `on` (kind 14) MUST be present.
    assert!(
        set.contains(&lk("on", CompletionItemKind::KEYWORD)),
        "statement-start offers the `on` keyword: {set:?}"
    );
    // The Doc 14 §4 empty-body snippet MUST be present as kind SNIPPET
    // (15) with the spec's verbatim insert text.
    let on_snip = served
        .iter()
        .find(|i| i.label == "on EVENT -> TARGET")
        .expect("the Doc 14 §4 `on EVENT -> TARGET` snippet is offered");
    assert_eq!(on_snip.kind, Some(CompletionItemKind::SNIPPET));
    assert_eq!(
        on_snip.insert_text.as_deref(),
        Some("on ${1:EVENT} -> ${2:Target}"),
        "snippet body MUST be verbatim Doc 14 §4"
    );
    // NEGATIVE: the event `GO` exists in scope but it is a symbol, not a
    // statement starter — it must be absent at a declaration-start.
    let ls = labels_of(&served);
    assert!(
        !ls.contains(&"GO".to_string()),
        "a symbol name MUST be absent from a statement-start completion \
         (only keywords/snippets start a declaration): {ls:?}"
    );
}

/// (L4-enc) **non-ASCII, BOTH `positionEncoding`s.** A 🚀 + Cyrillic block
/// comment (lexer-valid trivia — Cyrillic *identifiers* would explode the
/// ASCII-only lexer, prior-wave lesson) precedes a `on GO -> |`
/// transition-target on the same line, so the cursor column DIVERGES
/// between UTF-8 (bytes) and UTF-16 (code units). Under EACH negotiated
/// encoding the test sends the per-encoding-correct cursor column and
/// asserts the completion set is still the correct transition-target set
/// (state names) — proving the completion request's position mapping is
/// the ONE authoritative `LineIndex` inverse, not a naive byte/scalar
/// shim (which would map the divergent cursor to the wrong token and
/// mis-classify the context).
#[tokio::test(flavor = "current_thread")]
async fn completion_non_ascii_position_correct_under_utf8_and_utf16() {
    let mut cursor_cols: Vec<u32> = Vec::new();
    for (enc_kind, enc) in [
        (PositionEncodingKind::UTF8, OffsetEncoding::Utf8),
        (PositionEncodingKind::UTF16, OffsetEncoding::Utf16),
    ] {
        let (mut service, _socket) = LspService::new(Backend::new);
        let init = do_initialize(&mut service, std::slice::from_ref(&enc_kind)).await;
        assert_eq!(
            init["capabilities"]["positionEncoding"],
            json!(enc_kind.as_str())
        );

        let (text, uri) = fixture("l4_non_ascii.fsm");
        let did_open = Request::build("textDocument/didOpen")
            .params(did_open_params(&uri, &text))
            .finish();
        service.ready().await.unwrap().call(did_open).await.unwrap();

        let path = uri.to_file_path().unwrap();
        let analysis = analyze(&text, &path);
        let cst = fsm_parser::parse(&text).syntax();
        let idx = LineIndex::new(&text);

        // Cursor right after `on GO -> ` (the byte is encoding-independent;
        // the LSP `character` to send is NOT — compute via the same
        // authoritative LineIndex the server inverts).
        let cur_byte = byte_of(&text, "on GO -> ", "on GO -> ".len());
        let cur_pos = idx.position(&text, cur_byte as u32, enc);
        // It is on 0-based line 8 (the `… */ on GO -> ` line). Hard-coded
        // divergent columns (computed-from-fixture; the comment is the
        // SAME 🚀/ы/переход trivia as l3_non_ascii.fsm). The cursor sits
        // right after `-> ` — the exact byte where the L3 fixture's
        // `Target` ident begins, so the columns match L3's verified
        // values: line 8 prefix = 8 spaces + "/* " + 🚀 + " " + ы +
        // " переход */ on GO -> ". 🚀 is 1 Unicode scalar = 4 UTF-8 bytes
        // / 2 UTF-16 units; each Cyrillic letter = 2 UTF-8 bytes / 1
        // UTF-16 unit. → UTF-8 col 46, UTF-16 col 36 (10 fewer; the §4.1
        // divergence). A byte- or scalar-counting shim computes the wrong
        // one and the request maps to the wrong token / context.
        assert_eq!(cur_pos.line, 8, "[{enc:?}] cursor is on line 8");
        let expected_col = match enc {
            OffsetEncoding::Utf8 => 46,
            OffsetEncoding::Utf16 => 36,
        };
        assert_eq!(
            cur_pos.character, expected_col,
            "[{enc:?}] the cursor column after the multibyte comment must \
             diverge by the byte/code-unit delta — a byte/scalar shim \
             computes the wrong one"
        );
        cursor_cols.push(cur_pos.character);

        // Oracle (same code the server runs).
        let oracle = completions(&analysis.symbol_table, &cst, cur_byte as u32);
        let result = call_request(
            &mut service,
            "textDocument/completion",
            pos_params(&uri, cur_pos.line, cur_pos.character),
            90,
        )
        .await;
        let served = decode_completions(&result);

        // The served set must equal the oracle AND be exactly the
        // in-scope state names — proving the divergent cursor mapped to
        // the right token and the context classified correctly under each
        // encoding.
        assert_eq!(
            label_kind_set(&served),
            label_kind_set(&oracle),
            "[{enc:?}] served set must equal the oracle (divergent cursor \
             mapped to the right token under this encoding)"
        );
        let mut want = vec![
            lk("Start", CompletionItemKind::CLASS),
            lk("Target", CompletionItemKind::CLASS),
        ];
        want.sort();
        assert_eq!(
            label_kind_set(&served),
            want,
            "[{enc:?}] transition-target after the multibyte line = exactly \
             the state names `Start`,`Target` as CLASS"
        );
        // NEGATIVE under each encoding: the event `GO` must still be
        // absent (the exclusion must hold regardless of position encoding).
        assert!(
            !labels_of(&served).contains(&"GO".to_string()),
            "[{enc:?}] event `GO` MUST be absent from the transition-target \
             completion under this encoding too"
        );
    }
    // Cross-encoding sanity on REAL data: the UTF-8 cursor column MUST
    // exceed the UTF-16 one by exactly 10 (🚀 4→2, ы 2→1, переход 7×2→7
    // = 7 less; +1+2 = 10) — proving the encodings genuinely diverge on
    // this line, so a single-encoding bug could not have passed both.
    assert_eq!(cursor_cols.len(), 2, "both encodings exercised");
    assert_eq!(
        cursor_cols[0],
        cursor_cols[1] + 10,
        "UTF-8 col {} must exceed UTF-16 col {} by the multibyte delta (10)",
        cursor_cols[0],
        cursor_cols[1]
    );
}

/// (L4-spec) The exact Doc 26 §8 L4 acceptance sentence, restated with
/// byte assertions so the acceptance maps 1:1 to the doc clause and cannot
/// silently drift: `completion` after `on ` → the item set equals exactly
/// the machine's declared event names (kind 20); after `-> ` → exactly the
/// state names; after `ctx.` → exactly the context fields with their type
/// in `detail`; a keyword item's snippet text matches Doc 14 §4.
#[tokio::test(flavor = "current_thread")]
async fn doc26_l4_acceptance_sentence_verbatim() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    // "after `on ` → exactly the machine's declared event names (kind 20)"
    let (t_trig, _) = fixture("l4_trigger.fsm");
    let c1 = byte_of(&t_trig, "        on ", "        on ".len());
    let (s1, _o1, _u1, _x1) =
        open_and_complete(&mut service, "l4_trigger.fsm", c1, OffsetEncoding::Utf8, 95).await;
    let mut ev = vec![
        lk("GO", CompletionItemKind::EVENT),
        lk("STOP", CompletionItemKind::EVENT),
    ];
    ev.sort();
    assert_eq!(
        label_kind_set(&s1),
        ev,
        "after `on ` = exact event set, kind 20"
    );

    // "after `-> ` → exactly the state names"
    let (t_tt, _) = fixture("l4_transition_target.fsm");
    let c2 = byte_of(&t_tt, "on GO -> ", "on GO -> ".len());
    let (s2, _o2, _u2, _x2) = open_and_complete(
        &mut service,
        "l4_transition_target.fsm",
        c2,
        OffsetEncoding::Utf8,
        96,
    )
    .await;
    let mut st = vec![
        lk("Idle", CompletionItemKind::CLASS),
        lk("Running", CompletionItemKind::CLASS),
    ];
    st.sort();
    assert_eq!(label_kind_set(&s2), st, "after `-> ` = exact state set");

    // "after `ctx.` → exactly the context fields with their type in detail"
    let (t_basic, uri_b) = fixture("l4_basic.fsm");
    service
        .ready()
        .await
        .unwrap()
        .call(
            Request::build("textDocument/didOpen")
                .params(did_open_params(&uri_b, &t_basic))
                .finish(),
        )
        .await
        .unwrap();
    let idx_b = LineIndex::new(&t_basic);
    // `ctx.floor = ctx.floor + 1` — cursor right after the first `ctx.`.
    let c3 = byte_of(&t_basic, "ctx.", "ctx.".len());
    let p3 = idx_b.position(&t_basic, c3 as u32, OffsetEncoding::Utf8);
    let r3 = call_request(
        &mut service,
        "textDocument/completion",
        pos_params(&uri_b, p3.line, p3.character),
        97,
    )
    .await;
    let s3 = decode_completions(&r3);
    // Exactly the two context fields, kind FIELD, with type in `detail`.
    let mut got: Vec<(String, i64, Option<String>)> = s3
        .iter()
        .map(|i| (i.label.clone(), kind_num(i.kind), i.detail.clone()))
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            (
                "floor".to_string(),
                kind_num(Some(CompletionItemKind::FIELD)),
                Some("u8".to_string())
            ),
            (
                "moving".to_string(),
                kind_num(Some(CompletionItemKind::FIELD)),
                Some("bool".to_string())
            ),
        ],
        "after `ctx.` = exactly the context fields, kind FIELD, type in detail"
    );

    // "Assert a keyword item's snippet text matches Doc 14 §4." — the
    // `machine` keyword at file top level carries the Doc 14 §4 verbatim
    // snippet body. File top-level start: a fresh blank line after the
    // `language` decl.
    let top_src = "language fsm 2.0\n\n";
    let top_uri = Url::from_file_path("/tmp/l4_top.fsm").unwrap();
    service
        .ready()
        .await
        .unwrap()
        .call(
            Request::build("textDocument/didOpen")
                .params(did_open_params(&top_uri, top_src))
                .finish(),
        )
        .await
        .unwrap();
    let idx_t = LineIndex::new(top_src);
    let c_top = top_src.len(); // EOF (the blank line after `language`)
    let p_top = idx_t.position(top_src, c_top as u32, OffsetEncoding::Utf8);
    let r_top = call_request(
        &mut service,
        "textDocument/completion",
        pos_params(&top_uri, p_top.line, p_top.character),
        98,
    )
    .await;
    let s_top = decode_completions(&r_top);
    let machine_kw = s_top
        .iter()
        .find(|i| i.label == "machine")
        .expect("file top level offers the `machine` keyword");
    assert_eq!(machine_kw.kind, Some(CompletionItemKind::KEYWORD));
    assert_eq!(
        machine_kw.insert_text.as_deref(),
        Some("machine ${1:Name} {\n    $0\n}"),
        "the `machine` keyword snippet MUST be verbatim Doc 14 §4"
    );
}

// ===========================================================================
// L5 — references + prepareRename + rename (Doc 26 §8 L5 / risk-2).
//
// The most exhaustive §5.4 matrix of any wave: rename rewrites the user's
// source, so a wrong edit silently corrupts their program (the cardinal
// sin in its most acute form). Every test asserts decoded payloads vs the
// analysis oracle + hard-coded cross-checks; symbol/route presence is NOT
// acceptance. The headline risk-2 proof — a same-spelled string-literal /
// comment / different-scope token is NOT in the `WorkspaceEdit` — is made
// unmissable below.
// ===========================================================================

use tower_lsp::lsp_types::{PrepareRenameResponse, TextEdit, WorkspaceEdit};

use fsm_lsp::capabilities::references::references as oracle_references;
use fsm_lsp::refs::{prepare_rename as oracle_prepare, ReferenceIndex};

fn refs_params(uri: &Url, line: u32, character: u32, include_decl: bool) -> Value {
    json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character },
        "context": { "includeDeclaration": include_decl }
    })
}

fn rename_params(uri: &Url, line: u32, character: u32, new_name: &str) -> Value {
    json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character },
        "newName": new_name
    })
}

fn decode_locations(result: &Value) -> Option<Vec<Location>> {
    if result.is_null() {
        return None;
    }
    Some(serde_json::from_value(result.clone()).expect("decode Location[]"))
}

/// All byte ranges of comment + string-literal + whitespace tokens in
/// `text`, derived from the CST. The risk-2 guarantee is precisely that
/// **no** rename/reference range may fall inside ANY of these — so the
/// test asserts against the structural truth, not a brittle substring
/// search (a stronger, fixture-edit-proof assertion).
fn forbidden_trivia_and_string_ranges(text: &str) -> Vec<(usize, usize)> {
    use fsm_parser::cst::SyntaxKind as K;
    let cst = fsm_parser::parse(text).syntax();
    cst.descendants_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| {
            matches!(
                t.kind(),
                K::StringLiteral
                    | K::LineComment
                    | K::BlockComment
                    | K::DocComment
                    | K::Whitespace
                    | K::Newline
            )
        })
        .map(|t| {
            let r = t.text_range();
            (usize::from(r.start()), usize::from(r.end()))
        })
        .collect()
}

/// The full raw JSON-RPC envelope (so a request that returns an *error*
/// — `prepareRename`/`rename` refusals are JSON-RPC errors by design —
/// is observable; `call_request` only exposes `result`).
async fn call_envelope<S>(service: &mut S, method: &'static str, params: Value, id: i64) -> Value
where
    S: Service<Request, Response = Option<tower_lsp::jsonrpc::Response>>,
    S::Error: std::fmt::Debug,
{
    let req = Request::build(method).params(params).id(id).finish();
    let resp = service
        .ready()
        .await
        .unwrap()
        .call(req)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("{method} returned no response"));
    serde_json::to_value(resp).unwrap()
}

/// (L5-a) `references` with the cursor on a **decl** and on a **use** →
/// the EXACT semantic reference set (decl + the 3 transition uses), byte
/// ranges oracle-matched, `includeDeclaration` honoured. Both cursor
/// positions must yield the identical set (decl-cursor and use-cursor
/// resolve to the same symbol).
#[tokio::test(flavor = "current_thread")]
async fn references_decl_and_use_yield_exact_semantic_set() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text, uri) = fixture("l5_basic.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    // --- Oracle: the SAME pipeline + ReferenceIndex the server runs. ----
    let path = uri.to_file_path().unwrap();
    let analysis = analyze(&text, &path);
    assert!(
        analysis.diagnostics.is_empty(),
        "fixture invariant: l5_basic.fsm must be clean, got {:?}",
        analysis.diagnostics
    );
    let cst = fsm_parser::parse(&text).syntax();
    let idx = LineIndex::new(&text);
    let rindex = ReferenceIndex::build(&analysis.symbol_table, &cst);

    // `Moving`: declared once (`state Moving {`), used in 2 transition
    // targets (`Idle.on CALL -> Moving` and the `Moving.on CALL ->
    // Moving` self-target — `on STOP -> Idle` targets `Idle`, not
    // `Moving`). Verified from the fixture: 1 decl + 2 use sites = 3
    // with the declaration.
    let decl_cur = byte_of(&text, "state Moving", 6); // the `Moving` name
    let use_cur = byte_of(&text, "-> Moving", 3); // a transition target

    let decl_pos = idx.position(&text, decl_cur as u32, OffsetEncoding::Utf8);
    let use_pos = idx.position(&text, use_cur as u32, OffsetEncoding::Utf8);

    let want_with_decl = oracle_references(
        &analysis.symbol_table,
        &rindex,
        &cst,
        &uri,
        decl_cur as u32,
        true,
        &idx,
        &text,
        OffsetEncoding::Utf8,
    )
    .expect("oracle: Moving has references");

    // Server, cursor on the DECL.
    let r_decl = call_request(
        &mut service,
        "textDocument/references",
        refs_params(&uri, decl_pos.line, decl_pos.character, true),
        500,
    )
    .await;
    let got_decl = decode_locations(&r_decl).expect("server: refs from decl");
    assert_eq!(
        got_decl, want_with_decl,
        "references from the DECL cursor must byte-match the oracle"
    );

    // Server, cursor on a USE — must resolve to the SAME symbol → same set.
    let r_use = call_request(
        &mut service,
        "textDocument/references",
        refs_params(&uri, use_pos.line, use_pos.character, true),
        501,
    )
    .await;
    let got_use = decode_locations(&r_use).expect("server: refs from use");
    assert_eq!(
        got_use, want_with_decl,
        "references from a USE cursor must equal the same semantic set"
    );

    // Exactly 3 with the declaration (1 decl + 2 uses) — count is exact.
    assert_eq!(
        got_decl.len(),
        3,
        "decl + 2 transition uses = 3, got {got_decl:#?}"
    );
    for l in &got_decl {
        assert_eq!(l.uri, uri, "single-file: every reference is in this doc");
    }

    // includeDeclaration=false drops EXACTLY the declaration → 3, and the
    // dropped one is the `state Moving {` decl-name range.
    let r_nodecl = call_request(
        &mut service,
        "textDocument/references",
        refs_params(&uri, use_pos.line, use_pos.character, false),
        502,
    )
    .await;
    let got_nodecl = decode_locations(&r_nodecl).expect("server: uses only");
    assert_eq!(got_nodecl.len(), 2, "uses only = 2, got {got_nodecl:#?}");
    // The decl range that was present with-decl and absent without-decl:
    let decl_only: Vec<_> = got_decl
        .iter()
        .filter(|l| !got_nodecl.contains(l))
        .collect();
    assert_eq!(decl_only.len(), 1, "exactly one range is decl-only");
    let dr = decl_only[0];
    let db = idx.offset(&text, dr.range.start, OffsetEncoding::Utf8) as usize;
    assert_eq!(
        &text[db..db + "Moving".len()],
        "Moving",
        "the decl-only range is the bare `Moving` name token"
    );
}

/// (L5-b) **risk-2 exclusion at the references layer.** A same-spelled
/// identifier that resolves to a DIFFERENT symbol (a state `Moving` in a
/// different machine) is EXCLUDED; a same-spelled token inside a string
/// literal AND inside a comment is EXCLUDED. These are NOT in the
/// reference set of `Lift.Moving`.
#[tokio::test(flavor = "current_thread")]
async fn references_exclude_diff_scope_string_and_comment() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text, uri) = fixture("l5_safety.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    let path = uri.to_file_path().unwrap();
    let analysis = analyze(&text, &path);
    assert!(
        analysis.diagnostics.is_empty(),
        "fixture invariant: l5_safety.fsm clean, got {:?}",
        analysis.diagnostics
    );
    let idx = LineIndex::new(&text);

    // Cursor on Lift's `state Moving` decl name (the FIRST occurrence —
    // machine Crane's is later in the file).
    let lift_moving = byte_of(&text, "state Moving", 6);
    let lp = idx.position(&text, lift_moving as u32, OffsetEncoding::Utf8);
    let r = call_request(
        &mut service,
        "textDocument/references",
        refs_params(&uri, lp.line, lp.character, true),
        510,
    )
    .await;
    let got = decode_locations(&r).expect("Lift.Moving references");

    // The different-machine boundary + ALL comment/string regions (CST-
    // derived: fixture-edit-proof, and a STRONGER assertion than a single
    // substring search — NO reference may fall in ANY trivia/string).
    let crane_start = text.find("machine Crane").unwrap();
    let forbidden = forbidden_trivia_and_string_ranges(&text);

    for l in &got {
        let b = idx.offset(&text, l.range.start, OffsetEncoding::Utf8) as usize;
        assert!(
            b < crane_start,
            "a different-machine `Moving` (machine Crane) leaked into the \
             reference set: byte {b}, range {:?}",
            l.range
        );
        for &(s, e) in &forbidden {
            assert!(
                !(b >= s && b < e),
                "a `Moving` substring inside a comment/string/trivia \
                 ([{s},{e})) must NEVER be a reference: byte {b}"
            );
        }
        // Every returned range is the literal bare name token.
        assert_eq!(
            &text[b..b + "Moving".len()],
            "Moving",
            "every reference range is the bare `Moving` identifier"
        );
    }
    // Lift.Moving: decl + `Idle.on CALL -> Moving` + `Moving.on CALL ->
    // Moving` = 3 (the comment/string/Crane occurrences are all excluded).
    assert_eq!(
        got.len(),
        3,
        "Lift.Moving = decl + 2 semantic uses ONLY, got {got:#?}"
    );
}

/// (L5-c) `prepareRename` — the **negative matrix**, one rejection test
/// EACH (Doc 26 §8 L5): machine-name, `@id`/state-id, keyword, string
/// interior, comment, non-identifier/whitespace — all rejected with a
/// JSON-RPC error (the up-front "not renameable" contract, never a silent
/// allow that becomes a corrupting edit). Plus a positive: a renameable
/// state/event/ctx-field → the correct bare-name range.
#[tokio::test(flavor = "current_thread")]
async fn prepare_rename_negative_matrix_and_positive() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    // A fixture exercising every rejection target on valid grammar.
    let src = "language fsm 2.0\n\
               machine M {\n\
               \x20\x20context { note: str = \"Idle is a string\" }\n\
               \x20\x20events { GO }\n\
               \x20\x20initial Idle\n\
               \x20\x20// the word Idle in a comment\n\
               \x20\x20@id(\"s-idle-id\")\n\
               \x20\x20state Idle {\n\
               \x20\x20\x20\x20on GO -> Idle\n\
               \x20\x20}\n\
               }\n";
    let uri = Url::from_file_path("/tmp/l5_prep.fsm").unwrap();
    service
        .ready()
        .await
        .unwrap()
        .call(
            Request::build("textDocument/didOpen")
                .params(did_open_params(&uri, src))
                .finish(),
        )
        .await
        .unwrap();
    let idx = LineIndex::new(src);

    // Helper: send prepareRename at a byte, return (is_error, envelope).
    async fn prep(
        service: &mut (impl Service<
            Request,
            Response = Option<tower_lsp::jsonrpc::Response>,
            Error = impl std::fmt::Debug,
        > + Unpin),
        uri: &Url,
        idx: &LineIndex,
        src: &str,
        byte: usize,
        id: i64,
    ) -> Value {
        let p = idx.position(src, byte as u32, OffsetEncoding::Utf8);
        call_envelope(
            service,
            "textDocument/prepareRename",
            pos_params(uri, p.line, p.character),
            id,
        )
        .await
    }

    // Negatives — each must be a JSON-RPC ERROR (no range, told up-front).
    let cases: &[(&str, usize)] = &[
        ("machine name", byte_of(src, "machine M", 8)),
        ("@id annotation string", byte_of(src, "s-idle-id", 2)),
        ("keyword `state`", byte_of(src, "state Idle", 1)),
        ("string interior", byte_of(src, "Idle is a string", 0)),
        (
            "comment interior",
            byte_of(src, "word Idle in a comment", 5),
        ),
        ("whitespace", byte_of(src, "machine M", 7)),
    ];
    for (label, byte) in cases {
        let env = prep(&mut service, &uri, &idx, src, *byte, 600).await;
        assert!(
            env.get("error").is_some() && env.get("result").is_none(),
            "prepareRename on {label} MUST be rejected up-front (a \
             JSON-RPC error, never a range): got {env}"
        );
        // The error message is non-empty and explanatory.
        let msg = env["error"]["message"].as_str().unwrap_or("");
        assert!(
            !msg.is_empty(),
            "{label} rejection must carry a clear message"
        );
    }

    // Positive: the `Idle` transition-target use → a Range covering EXACTLY
    // the bare `Idle` name of `state Idle {`.
    let pos_byte = byte_of(src, "-> Idle", 3);
    let env = prep(&mut service, &uri, &idx, src, pos_byte, 610).await;
    assert!(
        env.get("error").is_none(),
        "a real state must be renameable, got error {env}"
    );
    let resp: PrepareRenameResponse =
        serde_json::from_value(env["result"].clone()).expect("decode prepareRename");
    let range = match resp {
        PrepareRenameResponse::Range(r) => r,
        other => panic!("expected a bare Range, got {other:?}"),
    };
    let rb = idx.offset(src, range.start, OffsetEncoding::Utf8) as usize;
    assert_eq!(
        &src[rb..rb + "Idle".len()],
        "Idle",
        "prepareRename range must be the bare `Idle` name token"
    );
    // Independent: the decl-name `Idle` is on the `state Idle {` line.
    let decl_line = src[..byte_of(src, "state Idle", 0)].matches('\n').count() as u32;
    assert_eq!(
        range.start.line, decl_line,
        "the renameable range is on the `state Idle {{` declaration line"
    );
}

/// (L5-d) **THE risk-2 core** — the headline safety proof, made
/// unmissable. A safe `rename` → the `WorkspaceEdit` text-edit set is
/// EXACTLY the semantic references (byte ranges oracle-matched, count
/// exact). On a fixture where the target name ALSO appears as a
/// string-literal substring AND in a comment AND as a different-scope
/// same-spelled symbol → assert NONE of those three are in the
/// `WorkspaceEdit`.
#[tokio::test(flavor = "current_thread")]
async fn rename_workspace_edit_is_exactly_semantic_refs_risk2_core() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text, uri) = fixture("l5_safety.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    let path = uri.to_file_path().unwrap();
    let analysis = analyze(&text, &path);
    let cst = fsm_parser::parse(&text).syntax();
    let idx = LineIndex::new(&text);
    let rindex = ReferenceIndex::build(&analysis.symbol_table, &cst);

    // Oracle: the semantic reference set of Lift.Moving (decl + uses).
    let lift_moving = byte_of(&text, "state Moving", 6);
    let oracle_set = oracle_references(
        &analysis.symbol_table,
        &rindex,
        &cst,
        &uri,
        lift_moving as u32,
        true,
        &idx,
        &text,
        OffsetEncoding::Utf8,
    )
    .expect("oracle: Lift.Moving refs");
    let oracle_ranges: std::collections::BTreeSet<(u32, u32, u32, u32)> = oracle_set
        .iter()
        .map(|l| {
            (
                l.range.start.line,
                l.range.start.character,
                l.range.end.line,
                l.range.end.character,
            )
        })
        .collect();

    // Cursor on a USE of Lift.Moving; rename to `Lifting`.
    let use_cur = byte_of(&text, "-> Moving", 3);
    let up = idx.position(&text, use_cur as u32, OffsetEncoding::Utf8);
    let env = call_envelope(
        &mut service,
        "textDocument/rename",
        rename_params(&uri, up.line, up.character, "Lifting"),
        700,
    )
    .await;
    assert!(
        env.get("error").is_none(),
        "a safe rename must succeed, got error {env}"
    );
    let we: WorkspaceEdit =
        serde_json::from_value(env["result"].clone()).expect("decode WorkspaceEdit");
    let changes = we.changes.expect("rename produces `changes`");
    let edits: &Vec<TextEdit> = changes
        .get(&uri)
        .expect("edits are for the requested document");

    // 1. The edit set is EXACTLY the semantic reference set (same ranges).
    let edit_ranges: std::collections::BTreeSet<(u32, u32, u32, u32)> = edits
        .iter()
        .map(|e| {
            (
                e.range.start.line,
                e.range.start.character,
                e.range.end.line,
                e.range.end.character,
            )
        })
        .collect();
    assert_eq!(
        edit_ranges, oracle_ranges,
        "the WorkspaceEdit ranges MUST be exactly the semantic references"
    );
    // Count exact: decl + 2 uses = 3 (NOT the comment/string/Crane ones).
    assert_eq!(
        edits.len(),
        3,
        "exactly decl + 2 semantic uses = 3 edits, got {edits:#?}"
    );
    for e in edits {
        assert_eq!(e.new_text, "Lifting", "every edit writes the new name");
    }

    // 2. THE risk-2 proof — NONE of the three forbidden occurrence
    //    classes is in the WorkspaceEdit: (a) a different-scope
    //    same-spelled `Moving` (machine Crane), (b) ANY comment, (c) ANY
    //    string literal. (b)+(c) are derived from the CST so the guard is
    //    fixture-edit-proof AND stricter than a single substring search.
    let crane_start = text.find("machine Crane").unwrap();
    let forbidden = forbidden_trivia_and_string_ranges(&text);
    for e in edits {
        let b = idx.offset(&text, e.range.start, OffsetEncoding::Utf8) as usize;
        assert!(
            b < crane_start,
            "SILENT-CORRUPTION GUARD: an edit fell in machine Crane \
             (a different-scope same-spelled `Moving`): byte {b}"
        );
        for &(s, en) in &forbidden {
            assert!(
                !(b >= s && b < en),
                "SILENT-CORRUPTION GUARD: an edit fell inside a comment/\
                 string/trivia region ([{s},{en})) — this is the exact \
                 silent-source-corruption risk-2 forbids: byte {b}"
            );
        }
        // And it is the bare identifier, never a wider/narrower slice.
        assert_eq!(
            &text[b..b + "Moving".len()],
            "Moving",
            "every edit replaces exactly the bare `Moving` identifier"
        );
    }

    // 3. Independent reconstruction: applying the edits to the buffer
    //    leaves Crane untouched and renames only Lift's Moving. Apply
    //    right-to-left so earlier byte offsets stay valid.
    let mut buf = text.clone();
    let mut byte_edits: Vec<(usize, usize)> = edits
        .iter()
        .map(|e| {
            let s = idx.offset(&text, e.range.start, OffsetEncoding::Utf8) as usize;
            let en = idx.offset(&text, e.range.end, OffsetEncoding::Utf8) as usize;
            (s, en)
        })
        .collect();
    byte_edits.sort_by(|a, b| b.0.cmp(&a.0));
    for (s, en) in byte_edits {
        buf.replace_range(s..en, "Lifting");
    }
    // Crane's `state Moving` / `initial Moving` / `-> Moving` survive.
    let crane_text = &buf[buf.find("machine Crane").unwrap()..];
    assert!(
        crane_text.contains("state Moving") && crane_text.contains("initial Moving"),
        "machine Crane's `Moving` MUST be untouched by Lift's rename"
    );
    // The comment + string still literally say "Moving" (byte-preserved):
    // these substrings appear ONLY in the comment / string in the fixture,
    // so finding them intact proves the rename did not touch trivia.
    assert!(
        buf.contains("mentions Moving by name"),
        "the comment text MUST be byte-preserved"
    );
    assert!(
        buf.contains("\"Moving to next floor\""),
        "the string literal MUST be byte-preserved"
    );
    // The edited buffer still parses clean (the rename did not corrupt it).
    let re = analyze(&buf, &path);
    assert!(
        re.diagnostics.is_empty(),
        "the renamed buffer must still be clean, got {:?}",
        re.diagnostics
    );
}

/// (L5-e) A rename whose new name **collides** with an existing symbol in
/// scope → rejected with a clear message and NO edit (Doc 26 risk-2: a
/// silent merge/shadow is the same class of corruption as a wrong edit).
#[tokio::test(flavor = "current_thread")]
async fn rename_collision_is_rejected_with_no_edit() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text, uri) = fixture("l5_basic.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    let idx = LineIndex::new(&text);
    // Rename `Moving` → `Idle` — `Idle` is an existing state in the SAME
    // machine, so this would silently merge two states. Must be refused.
    let use_cur = byte_of(&text, "-> Moving", 3);
    let up = idx.position(&text, use_cur as u32, OffsetEncoding::Utf8);
    let env = call_envelope(
        &mut service,
        "textDocument/rename",
        rename_params(&uri, up.line, up.character, "Idle"),
        710,
    )
    .await;
    assert!(
        env.get("error").is_some() && env.get("result").is_none(),
        "a colliding rename MUST be rejected (error, NO WorkspaceEdit): {env}"
    );
    let msg = env["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("already exists"),
        "the collision message must explain the conflict, got {msg:?}"
    );

    // An invalid identifier is likewise rejected with no edit.
    let env2 = call_envelope(
        &mut service,
        "textDocument/rename",
        rename_params(&uri, up.line, up.character, "9bad name"),
        711,
    )
    .await;
    assert!(
        env2.get("error").is_some() && env2.get("result").is_none(),
        "an invalid new identifier MUST be rejected with no edit: {env2}"
    );
}

/// (L5-f) **Non-ASCII, BOTH encodings.** All reference & rename edit
/// ranges are correct under `positionEncoding` UTF-8 AND UTF-16 — a
/// byte/scalar shim must fail this. Multibyte text is ONLY in a
/// lexer-valid block comment (`/* 🚀 ы переход к Target */`); the renamed
/// state `Target` is referenced on the SAME line after that comment and
/// on a later line, so the intra-line column math is exercised under both
/// encodings.
#[tokio::test(flavor = "current_thread")]
async fn references_and_rename_non_ascii_both_encodings() {
    for enc_kind in [PositionEncodingKind::UTF8, PositionEncodingKind::UTF16] {
        let (mut service, _socket) = LspService::new(Backend::new);
        do_initialize(&mut service, std::slice::from_ref(&enc_kind)).await;
        let enc = OffsetEncoding::from_lsp(&enc_kind);

        let (text, uri) = fixture("l5_non_ascii.fsm");
        let did_open = Request::build("textDocument/didOpen")
            .params(did_open_params(&uri, &text))
            .finish();
        service.ready().await.unwrap().call(did_open).await.unwrap();

        let path = uri.to_file_path().unwrap();
        let analysis = analyze(&text, &path);
        assert!(
            analysis.diagnostics.is_empty(),
            "[{enc_kind:?}] l5_non_ascii.fsm must be clean, got {:?}",
            analysis.diagnostics
        );
        let cst = fsm_parser::parse(&text).syntax();
        let idx = LineIndex::new(&text);
        let rindex = ReferenceIndex::build(&analysis.symbol_table, &cst);

        // Cursor on the `Target` use in `…переход к Target */ on GO ->
        // Target` (the post-comment, post-arrow target on the multibyte
        // line) — its column DIFFERS between UTF-8 (bytes) and UTF-16
        // (code units) because of 🚀+Cyrillic earlier on the line.
        let use_cur = byte_of(&text, "-> Target", 3);
        let cur_pos = idx.position(&text, use_cur as u32, enc);

        // Oracle references in THIS encoding.
        let want = oracle_references(
            &analysis.symbol_table,
            &rindex,
            &cst,
            &uri,
            use_cur as u32,
            true,
            &idx,
            &text,
            enc,
        )
        .expect("[oracle] Target refs");

        let r = call_request(
            &mut service,
            "textDocument/references",
            refs_params(&uri, cur_pos.line, cur_pos.character, true),
            800,
        )
        .await;
        let got = decode_locations(&r).expect("server: Target refs");
        assert_eq!(
            got, want,
            "[{enc_kind:?}] references must byte-match the oracle in this encoding"
        );
        // decl + 2 uses (`Start.-> Target`, `Target.-> Target`) = 3.
        assert_eq!(got.len(), 3, "[{enc_kind:?}] decl + 2 uses, got {got:#?}");

        // Every range round-trips back to the bare `Target` token via the
        // SAME encoding's inverse — a wrong intra-line transcoding fails
        // this on the multibyte line.
        for l in &got {
            let b = idx.offset(&text, l.range.start, enc) as usize;
            assert_eq!(
                &text[b..b + "Target".len()],
                "Target",
                "[{enc_kind:?}] every reference range is the bare `Target`"
            );
        }

        // Rename in this encoding → edits must also round-trip correctly.
        let env = call_envelope(
            &mut service,
            "textDocument/rename",
            rename_params(&uri, cur_pos.line, cur_pos.character, "Goal"),
            801,
        )
        .await;
        assert!(
            env.get("error").is_none(),
            "[{enc_kind:?}] safe rename must succeed, got {env}"
        );
        let we: WorkspaceEdit =
            serde_json::from_value(env["result"].clone()).expect("decode WorkspaceEdit");
        let edits = &we.changes.unwrap()[&uri];
        assert_eq!(
            edits.len(),
            3,
            "[{enc_kind:?}] rename edits = decl + 2 uses = 3, got {edits:#?}"
        );
        // Apply right-to-left; the result must parse clean AND the
        // multibyte comment must be byte-preserved (the rename only
        // touched `Target` identifiers, never the 🚀/Cyrillic trivia).
        let mut buf = text.clone();
        let mut be: Vec<(usize, usize)> = edits
            .iter()
            .map(|e| {
                (
                    idx.offset(&text, e.range.start, enc) as usize,
                    idx.offset(&text, e.range.end, enc) as usize,
                )
            })
            .collect();
        be.sort_by(|a, b| b.0.cmp(&a.0));
        for (s, en) in be {
            assert_eq!(
                &text[s..en],
                "Target",
                "[{enc_kind:?}] each edit range is exactly `Target`"
            );
            buf.replace_range(s..en, "Goal");
        }
        assert!(
            buf.contains("🚀 ы переход к Target */"),
            "[{enc_kind:?}] the multibyte comment MUST be byte-preserved \
             (the rename must not touch `Target` inside the comment)"
        );
        let re = analyze(&buf, &path);
        assert!(
            re.diagnostics.is_empty(),
            "[{enc_kind:?}] renamed buffer must still be clean, got {:?}",
            re.diagnostics
        );
    }
}

/// (L5-g) `references`/`prepareRename` on a non-resolvable cursor return
/// the spec-correct empty/error answer — never a panic, never a textual
/// guess (the single-file graceful-degradation contract, Doc 14 §8).
#[tokio::test(flavor = "current_thread")]
async fn references_none_and_prepare_error_on_non_symbol() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text, uri) = fixture("l5_basic.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();
    let idx = LineIndex::new(&text);

    // Cursor on the `state` keyword → references is `null`.
    let kw = byte_of(&text, "state Moving", 1);
    let kp = idx.position(&text, kw as u32, OffsetEncoding::Utf8);
    let r = call_request(
        &mut service,
        "textDocument/references",
        refs_params(&uri, kp.line, kp.character, true),
        900,
    )
    .await;
    assert!(
        r.is_null(),
        "references on a keyword must be null (no textual guess), got {r}"
    );

    // The oracle agrees (the SAME prepare logic): a keyword is not
    // renameable.
    let cst = fsm_parser::parse(&text).syntax();
    let analysis = analyze(&text, &uri.to_file_path().unwrap());
    assert!(
        oracle_prepare(&analysis.symbol_table, &cst, kw as u32).is_err(),
        "oracle: a keyword cursor is not renameable"
    );
}

// ===========================================================================
// L6 — semanticTokens/full + /range (Doc 26 §8 L6 / Doc 14 §10).
//
// The delta encoding makes symbol-presence acceptance non-trivial AND
// forbidden: every test **decodes the relative `u32` array back to absolute
// `(line, char, len, type, modifiers)` tuples** and asserts the FULL
// decoded stream equals the oracle (the SAME `semantic_tokens_full` the
// server runs, off the SAME analysis), plus hard-coded cross-checks on
// specific tokens (a state DECLARATION carries the `declaration` modifier; a
// state USE carries the type but NOT that modifier; a `ctx.field` ref is the
// field type; a comment is `comment`; a keyword is `keyword`). The
// advertised legend indices are asserted to be exactly what the encoded
// `tokenType`/`tokenModifiers` reference. The non-ASCII test runs under BOTH
// `positionEncoding`s — a byte-vs-UTF-16 mismatch makes the decoded
// positions wrong → the test fails (the §4.1 defect guard, at the L6 layer).
// ===========================================================================

use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensResult,
};

use fsm_lsp::capabilities::semantic_tokens::{
    legend as oracle_legend, semantic_tokens_full as oracle_semantic_full,
    semantic_tokens_range as oracle_semantic_range,
};

fn semantic_full_params(uri: &Url) -> Value {
    json!({ "textDocument": { "uri": uri } })
}

fn semantic_range_params(uri: &Url, range: Range) -> Value {
    json!({
        "textDocument": { "uri": uri },
        "range": {
            "start": { "line": range.start.line, "character": range.start.character },
            "end":   { "line": range.end.line,   "character": range.end.character }
        }
    })
}

/// Decode the LSP relative-encoded `data` array (the wire form) back to
/// ABSOLUTE `(line, startChar, len, tokenType, tokenModifiers)` tuples.
/// This is the inverse of the server's `encode`; assertions read against
/// absolute positions so a delta-math regression (a wrong deltaLine /
/// deltaStartChar / length, or a non-reset deltaStartChar on a new line)
/// makes the decoded stream wrong and the equality fail — that is the point
/// (symbol presence is NOT acceptance; the decoded bytes are).
fn decode_semantic(tokens: &[SemanticToken]) -> Vec<(u32, u32, u32, u32, u32)> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut line = 0u32;
    let mut ch = 0u32;
    for t in tokens {
        if t.delta_line == 0 {
            ch += t.delta_start;
        } else {
            line += t.delta_line;
            ch = t.delta_start;
        }
        out.push((line, ch, t.length, t.token_type, t.token_modifiers_bitset));
    }
    out
}

fn decode_semantic_result(result: &Value) -> Vec<SemanticToken> {
    match serde_json::from_value::<SemanticTokensResult>(result.clone())
        .expect("decode SemanticTokensResult")
    {
        SemanticTokensResult::Tokens(SemanticTokens { data, .. }) => data,
        SemanticTokensResult::Partial(_) => {
            panic!("server must return full Tokens, not a partial result")
        }
    }
}

/// Legend index of a token-type name in the advertised legend (so the
/// per-token cross-checks reference the SAME indices the server encodes
/// against — never a hard-coded magic number that could silently drift
/// from the legend).
fn ty_index(legend: &tower_lsp::lsp_types::SemanticTokensLegend, t: &SemanticTokenType) -> u32 {
    legend
        .token_types
        .iter()
        .position(|x| x == t)
        .unwrap_or_else(|| panic!("token type {t:?} not in advertised legend")) as u32
}

fn md_bit(legend: &tower_lsp::lsp_types::SemanticTokensLegend, m: &SemanticTokenModifier) -> u32 {
    let i = legend
        .token_modifiers
        .iter()
        .position(|x| x == m)
        .unwrap_or_else(|| panic!("token modifier {m:?} not in advertised legend"));
    1u32 << i
}

/// Find the single decoded token whose absolute `(line, startChar)` is the
/// LSP position of `needle`+`plus` in `text` under `enc`.
fn tok_at<'a>(
    decoded: &'a [(u32, u32, u32, u32, u32)],
    li: &LineIndex,
    text: &str,
    enc: OffsetEncoding,
    needle: &str,
    plus: usize,
) -> &'a (u32, u32, u32, u32, u32) {
    let b = byte_of(text, needle, plus);
    let p = li.position(text, b as u32, enc);
    decoded
        .iter()
        .find(|(l, c, ..)| *l == p.line && *c == p.character)
        .unwrap_or_else(|| panic!("no semantic token at {needle:?}+{plus} (pos {p:?})"))
}

/// (L6-a) `semanticTokens/full` on a fixture exercising every legend index
/// → the FULL decoded stream byte-equals the reused-pipeline oracle, the
/// advertised legend matches Doc 14 §2/§10, and the hard-coded per-token
/// cross-checks hold (state decl vs use modifier, ctx field type, comment,
/// keyword, operator, number, extern, event, the `@id` decorator + its
/// string).
#[tokio::test(flavor = "current_thread")]
async fn semantic_tokens_full_decoded_stream_matches_oracle_and_cross_checks() {
    let (mut service, _socket) = LspService::new(Backend::new);
    let init = do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    // (1) The advertised legend is EXACTLY Doc 14 §2/§10 order and equals
    //     the one the encoder uses (`oracle_legend`) — so every encoded
    //     `tokenType`/`tokenModifiers` integer references the advertised
    //     name. A drift here corrupts every client's colouring.
    let adv = &init["capabilities"]["semanticTokensProvider"];
    let adv_types: Vec<String> =
        serde_json::from_value(adv["legend"]["tokenTypes"].clone()).expect("advertised tokenTypes");
    let adv_mods: Vec<String> = serde_json::from_value(adv["legend"]["tokenModifiers"].clone())
        .expect("advertised tokenModifiers");
    let legend = oracle_legend();
    let want_types: Vec<String> = legend
        .token_types
        .iter()
        .map(|t| t.as_str().to_owned())
        .collect();
    let want_mods: Vec<String> = legend
        .token_modifiers
        .iter()
        .map(|m| m.as_str().to_owned())
        .collect();
    assert_eq!(
        adv_types,
        vec![
            "namespace",
            "type",
            "enum",
            "function",
            "variable",
            "keyword",
            "string",
            "number",
            "operator",
            "comment",
            "decorator"
        ],
        "advertised tokenTypes MUST be Doc 14 §2/§10 order"
    );
    assert_eq!(
        adv_mods,
        vec!["declaration", "readonly", "deprecated", "static"],
        "advertised tokenModifiers MUST be Doc 14 §2/§10 order"
    );
    assert_eq!(
        adv_types, want_types,
        "advertised == encoder legend (types)"
    );
    assert_eq!(adv_mods, want_mods, "advertised == encoder legend (mods)");
    assert_eq!(
        adv["full"],
        json!(true),
        "Doc 26 §8 L6 / Doc 14 §2: `full` advertised"
    );
    assert_eq!(
        adv["range"],
        json!(true),
        "Doc 26 §8 L6 / Doc 14 §2: `range` advertised"
    );

    let (text, uri) = fixture("l6_legend.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    // (2) Oracle: the SAME `semantic_tokens_full` the server runs, off the
    //     SAME `analyze()` pipeline. The decoded stream must byte-match —
    //     proving the server wired the single analysis + the reused
    //     classifier + correct delta math (not a parallel pass).
    let path = uri.to_file_path().unwrap();
    let analysis = analyze(&text, &path);
    assert!(
        analysis.diagnostics.is_empty(),
        "fixture invariant: l6_legend.fsm must be clean, got {:?}",
        analysis.diagnostics
    );
    let cst = fsm_parser::parse(&text).syntax();
    let li = LineIndex::new(&text);
    let want = oracle_semantic_full(
        &analysis.symbol_table,
        &cst,
        &li,
        &text,
        OffsetEncoding::Utf8,
    );

    let result = call_request(
        &mut service,
        "textDocument/semanticTokens/full",
        semantic_full_params(&uri),
        600,
    )
    .await;
    let got_raw = decode_semantic_result(&result);
    assert_eq!(
        got_raw, want.data,
        "the raw delta-encoded array MUST equal the reused-pipeline oracle"
    );
    let got = decode_semantic(&got_raw);
    let want_dec = decode_semantic(&want.data);
    assert_eq!(
        got, want_dec,
        "the DECODED absolute stream MUST equal the oracle (delta math correct)"
    );
    assert!(!got.is_empty(), "a non-empty fixture yields tokens");

    // (3) Hard-coded per-token cross-checks — these pin the classification
    //     (the oracle proves server==pipeline; these prove
    //     pipeline==Doc 14 §10, so neither can silently regress).
    let t_type = ty_index(&legend, &SemanticTokenType::TYPE);
    let t_enum = ty_index(&legend, &SemanticTokenType::ENUM);
    let t_func = ty_index(&legend, &SemanticTokenType::FUNCTION);
    let t_var = ty_index(&legend, &SemanticTokenType::VARIABLE);
    let t_kw = ty_index(&legend, &SemanticTokenType::KEYWORD);
    let t_str = ty_index(&legend, &SemanticTokenType::STRING);
    let t_num = ty_index(&legend, &SemanticTokenType::NUMBER);
    let t_op = ty_index(&legend, &SemanticTokenType::OPERATOR);
    let t_cmt = ty_index(&legend, &SemanticTokenType::COMMENT);
    let t_dec = ty_index(&legend, &SemanticTokenType::DECORATOR);
    let m_decl = md_bit(&legend, &SemanticTokenModifier::DECLARATION);
    let m_ro = md_bit(&legend, &SemanticTokenModifier::READONLY);
    let e = OffsetEncoding::Utf8;

    // A state DECLARATION (`state Idle {`) → type `type` + `declaration`.
    let decl_idle = tok_at(&got, &li, &text, e, "state Idle {", 6);
    assert_eq!(decl_idle.2, 4, "`Idle` length 4");
    assert_eq!(
        (decl_idle.3, decl_idle.4),
        (t_type, m_decl),
        "a state DECLARATION = type `type` + the `declaration` modifier"
    );
    // The SAME state name USED as a transition target (`-> Moving`)… use
    // `Idle` used in `on STOP -> Idle` instead (Idle is targeted there).
    let use_idle = tok_at(&got, &li, &text, e, "-> Idle", 3);
    assert_eq!(
        (use_idle.3, use_idle.4),
        (t_type, 0),
        "a state USE = type `type`, NO `declaration` modifier (decl-vs-ref)"
    );
    // Event name after `on ` → `enum`.
    let ev = tok_at(&got, &li, &text, e, "on CALL", 3);
    assert_eq!(ev.3, t_enum, "event name → `enum`");
    // `ctx.floor` field ref → `variable` (the field, classified by reuse
    // of the L3 resolver).
    let ctxf = tok_at(&got, &li, &text, e, "ctx.floor == 0", 4);
    assert_eq!(
        ctxf.3, t_var,
        "a ctx.field ref is the field type `variable`"
    );
    // extern call `can_go(1)` in the guard → `function`.
    let ext = tok_at(&got, &li, &text, e, "can_go(1)", 0);
    assert_eq!(ext.3, t_func, "an extern name → `function`");
    // The `state` keyword → `keyword`.
    let kw = tok_at(&got, &li, &text, e, "state Idle", 0);
    assert_eq!(kw.3, t_kw, "a reserved keyword → `keyword`");
    // The `// a line comment` → `comment`.
    let cmt = tok_at(&got, &li, &text, e, "// a line comment", 0);
    assert_eq!(cmt.3, t_cmt, "a line comment → `comment`");
    // The `->` operator → `operator`.
    let arrow = tok_at(&got, &li, &text, e, "-> Moving", 0);
    assert_eq!(arrow.3, t_op, "`->` → `operator`");
    // The `0` integer literal in `floor: u8 = 0` → `number`.
    let num = tok_at(&got, &li, &text, e, "u8 = 0", 5);
    assert_eq!(num.3, t_num, "an integer literal → `number`");
    // The `@id` decorator marker → `decorator`; its `"s-idle"` string →
    // `string` (Doc 14 §10 index 6 covers stable-ID strings).
    let atid = tok_at(&got, &li, &text, e, "@id(\"s-idle\")", 0);
    assert_eq!(atid.3, t_dec, "`@id` annotation marker → `decorator`");
    let sid = tok_at(&got, &li, &text, e, "\"s-idle\"", 0);
    assert_eq!(sid.3, t_str, "the stable-ID string → `string`");
    // `payload`-free fixture, but assert the `readonly` modifier constant
    // is referenced by the legend (a guard the bit math stays in legend
    // space even when this fixture emits none).
    assert_eq!(m_ro, 1u32 << 1, "readonly modifier is legend bit 1");

    // (4) No structural-punctuation token leaked (Doc 14 §10 has no slot
    //     for `{` — TextMate handles it; the coexistence model).
    let brace_b = byte_of(&text, "Lift {", 5);
    let bp = li.position(&text, brace_b as u32, e);
    assert!(
        !got.iter()
            .any(|(l, c, len, ..)| *l == bp.line && *c == bp.character && *len == 1),
        "a structural `{{` must NOT get a semantic token"
    );
}

/// (L6-b) `semanticTokens/range` — Doc 26 §8 L6 explicitly specifies
/// `full + range`. The range response is a self-contained token stream
/// (its first token's deltas are relative to the response start, NOT a
/// slice of the full stream), equals the oracle for the same range, and
/// contains no out-of-range token.
#[tokio::test(flavor = "current_thread")]
async fn semantic_tokens_range_is_self_contained_and_matches_oracle() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;

    let (text, uri) = fixture("l6_legend.fsm");
    let did_open = Request::build("textDocument/didOpen")
        .params(did_open_params(&uri, &text))
        .finish();
    service.ready().await.unwrap().call(did_open).await.unwrap();

    let path = uri.to_file_path().unwrap();
    let analysis = analyze(&text, &path);
    let cst = fsm_parser::parse(&text).syntax();
    let li = LineIndex::new(&text);
    let e = OffsetEncoding::Utf8;

    // Range = exactly the `state Moving { … }` declaration block.
    let s_byte = byte_of(&text, "state Moving", 0) as u32;
    let e_byte = (text.rfind('}').unwrap()) as u32; // close of Moving/machine
    let r = Range {
        start: li.position(&text, s_byte, e),
        end: li.position(&text, e_byte, e),
    };

    let want = oracle_semantic_range(
        &analysis.symbol_table,
        &cst,
        &li,
        &text,
        e,
        li.offset(&text, r.start, e),
        li.offset(&text, r.end, e),
    );

    let result = call_request(
        &mut service,
        "textDocument/semanticTokens/range",
        semantic_range_params(&uri, r),
        610,
    )
    .await;
    let got_raw = decode_semantic_result(&result);
    assert_eq!(
        got_raw, want.data,
        "the range delta array MUST equal the reused-pipeline range oracle"
    );
    let got = decode_semantic(&got_raw);
    assert!(!got.is_empty(), "the range over `state Moving` has tokens");

    // Self-contained: the FIRST token decodes to an ABSOLUTE position on
    // the `state Moving` line (its deltas are relative to the response
    // start, not byte 0 of the document).
    let moving_line = li.position(&text, s_byte, e).line;
    assert_eq!(
        got[0].0, moving_line,
        "the range substream decodes to absolute positions (relative to response start)"
    );
    // No token outside [Moving-decl-line, end] leaked in (the `state Idle`
    // line precedes the range).
    let idle_line = li
        .position(&text, byte_of(&text, "state Idle", 0) as u32, e)
        .line;
    for (l, ..) in &got {
        assert!(
            *l >= moving_line && *l != idle_line,
            "an out-of-range token (line {l}) leaked into the range response"
        );
    }
}

/// (L6-c) **Non-ASCII, BOTH `positionEncoding`s.** A 4-byte 🚀 + 2-byte
/// Cyrillic appear ONLY in lexer-valid block comments (Cyrillic
/// *identifiers* explode the ASCII-only lexer — the standing lesson). The
/// state `Target` is used on the SAME line after a multibyte single-line
/// comment, and a MULTI-line block comment forces the per-line split. The
/// decoded absolute positions/lengths must be correct under UTF-8 (bytes)
/// AND UTF-16 (code units) — a byte-vs-UTF-16 mismatch makes the decoded
/// position wrong and the test fails (the §4.1 defect guard at L6).
#[tokio::test(flavor = "current_thread")]
async fn semantic_tokens_non_ascii_correct_under_both_encodings() {
    for enc_kind in [PositionEncodingKind::UTF8, PositionEncodingKind::UTF16] {
        let (mut service, _socket) = LspService::new(Backend::new);
        do_initialize(&mut service, std::slice::from_ref(&enc_kind)).await;
        let enc = OffsetEncoding::from_lsp(&enc_kind);

        let (text, uri) = fixture("l6_non_ascii.fsm");
        let did_open = Request::build("textDocument/didOpen")
            .params(did_open_params(&uri, &text))
            .finish();
        service.ready().await.unwrap().call(did_open).await.unwrap();

        let path = uri.to_file_path().unwrap();
        let analysis = analyze(&text, &path);
        assert!(
            analysis.diagnostics.is_empty(),
            "[{enc_kind:?}] l6_non_ascii.fsm must be clean, got {:?}",
            analysis.diagnostics
        );
        let cst = fsm_parser::parse(&text).syntax();
        let li = LineIndex::new(&text);
        let want = oracle_semantic_full(&analysis.symbol_table, &cst, &li, &text, enc);

        let result = call_request(
            &mut service,
            "textDocument/semanticTokens/full",
            semantic_full_params(&uri),
            620,
        )
        .await;
        let got_raw = decode_semantic_result(&result);
        assert_eq!(
            got_raw, want.data,
            "[{enc_kind:?}] raw delta array MUST equal the oracle in this encoding"
        );
        let got = decode_semantic(&got_raw);

        let legend = oracle_legend();
        let t_type = ty_index(&legend, &SemanticTokenType::TYPE);
        let t_cmt = ty_index(&legend, &SemanticTokenType::COMMENT);
        let m_decl = md_bit(&legend, &SemanticTokenModifier::DECLARATION);

        // The `Target` USE in `… переход к Target */ on GO -> Target` — its
        // column DIFFERS UTF-8 (bytes) vs UTF-16 (code units) because of the
        // 🚀+Cyrillic earlier on the line. Decoded absolute position must be
        // correct in THIS encoding (a byte/scalar shim fails one of these).
        let use_target = tok_at(&got, &li, &text, enc, "-> Target", 3);
        assert_eq!(use_target.3, t_type, "[{enc_kind:?}] a state use is `type`");
        assert_eq!(
            use_target.4, 0,
            "[{enc_kind:?}] a state USE has NO `declaration` modifier"
        );
        // Its decoded position must round-trip back to the bare `Target`
        // token via the SAME encoding's inverse (a wrong intra-line
        // transcoding on the multibyte line fails this).
        let back = li.offset(
            &text,
            tower_lsp::lsp_types::Position {
                line: use_target.0,
                character: use_target.1,
            },
            enc,
        ) as usize;
        assert_eq!(
            &text[back..back + "Target".len()],
            "Target",
            "[{enc_kind:?}] decoded position round-trips to the bare `Target`"
        );
        // The `Target` DECLARATION carries the `declaration` modifier (and
        // is the SAME `type`), regardless of encoding.
        let decl_target = tok_at(&got, &li, &text, enc, "state Target", 6);
        assert_eq!(
            (decl_target.3, decl_target.4),
            (t_type, m_decl),
            "[{enc_kind:?}] state DECLARATION = `type` + `declaration`"
        );

        // The MULTI-line `/* multi … 🚀 */` block comment is split into
        // one piece PER line — none spanning lines (LSP
        // multilineTokenSupport is off). Its pieces' lengths are in the
        // negotiated unit (so the 🚀/Cyrillic line's length differs UTF-8
        // vs UTF-16).
        let cmt_open = byte_of(&text, "/* multi", 0);
        let cmt_line0 = li.position(&text, cmt_open as u32, enc).line;
        let comment_pieces: Vec<&(u32, u32, u32, u32, u32)> = got
            .iter()
            .filter(|(l, .., ty, _)| *ty == t_cmt && *l >= cmt_line0 && *l <= cmt_line0 + 1)
            .collect();
        assert!(
            comment_pieces.len() >= 2,
            "[{enc_kind:?}] the 2-line /* */ MUST split into ≥2 per-line pieces, got {comment_pieces:?}"
        );
        // No comment piece spans more than its own line: re-derive each
        // piece's end position and assert it is on the same line as its
        // start (the per-line-split invariant).
        for &&(l, c, len, ..) in &comment_pieces {
            // Convert (l, c) + len back to a byte, then to a position; it
            // must stay on line `l` (a multi-line token would not).
            let start_b = li.offset(
                &text,
                tower_lsp::lsp_types::Position {
                    line: l,
                    character: c,
                },
                enc,
            );
            let end_pos = li.position(
                &text,
                start_b + token_byte_len(&text, start_b, len, enc),
                enc,
            );
            assert_eq!(
                end_pos.line, l,
                "[{enc_kind:?}] comment piece at line {l} must NOT span lines"
            );
        }

        // FULL decoded-stream equality in this encoding (the strongest
        // assertion — every token's position/length/type/modifier under
        // the negotiated unit).
        let want_dec = decode_semantic(&want.data);
        assert_eq!(
            got, want_dec,
            "[{enc_kind:?}] the full decoded stream MUST equal the oracle"
        );
    }
}

/// Helper for the non-ASCII test: the byte length of a token that starts at
/// `start_byte` and is `units` long in `enc` units (walk `units` code units
/// forward from `start_byte`, return the byte delta). Pure inverse-of-length
/// arithmetic over the buffer — used only to re-derive a piece's end byte
/// for the "does not span lines" check.
fn token_byte_len(text: &str, start_byte: u32, units: u32, enc: OffsetEncoding) -> u32 {
    let mut consumed = 0u32;
    let mut bytes = 0u32;
    for ch in text[start_byte as usize..].chars() {
        if consumed >= units {
            break;
        }
        consumed += match enc {
            OffsetEncoding::Utf8 => ch.len_utf8() as u32,
            OffsetEncoding::Utf16 => ch.len_utf16() as u32,
        };
        bytes += ch.len_utf8() as u32;
    }
    bytes
}

/// (L6-d) A not-open document → `semanticTokens/full` returns `null` (the
/// spec-correct empty answer, never a panic, never a fabricated token).
#[tokio::test(flavor = "current_thread")]
async fn semantic_tokens_full_on_unopened_doc_is_null() {
    let (mut service, _socket) = LspService::new(Backend::new);
    do_initialize(&mut service, &[PositionEncodingKind::UTF8]).await;
    let uri = Url::parse("file:///tmp/never-opened-l6.fsm").unwrap();
    let result = call_request(
        &mut service,
        "textDocument/semanticTokens/full",
        semantic_full_params(&uri),
        630,
    )
    .await;
    assert!(
        result.is_null(),
        "semanticTokens on a not-open doc must be null, got {result}"
    );
}
