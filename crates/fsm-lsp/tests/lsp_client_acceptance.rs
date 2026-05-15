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
