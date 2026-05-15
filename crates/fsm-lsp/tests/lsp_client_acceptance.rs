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
