//! §5.4-W-A2 behavioural acceptance — the **differential-oracle proof**
//! (Doc 31 §1 W-A2 / §2 the keystone-in-UI invariant; §5.4; the
//! v1.2-LSP-L1..L7 / v1.3 in-process tower-lsp precedent).
//!
//! This is the LSP analogue of A1's Extension-Host differential oracle: an
//! **in-process `tower-lsp` client** issues the real `fsm/verify` custom
//! request and asserts its `verifyJson` **byte-equals** `fsm verify --json`
//! on the SAME fixture — where the oracle is the **real `fsm` binary
//! spawned in-test** (A1's CLI seam is the oracle, Doc 31 §1 W-A2). Symbol/
//! capability-presence ("the request is registered", "the function
//! exists") is explicitly NOT acceptance — the byte-equal verdict + witness
//! + reachability diagnostics across the fixture battery is the proof.
//!
//! Battery (Doc 31 §1 W-A2 — the W4a-verified known-good canon, NOT
//! authored here):
//!  - `examples/verify/clean.fsm`      → `verified`              (sound)
//!  - `examples/verify/deadlocks.fsm`  → `property-violated`, witness `["ARM"]`
//!  - `examples/verify/clean.fsm` + a tiny `maxStates` → `inconclusive`,
//!    asserted **NOT** a false `verified` (the cardinal verification-UI sin)
//!  - `editors/vscode/src/test/fixtures/unreachable.fsm` → a reachability
//!    defect (FSM-E0400/FSM-W0602 with concrete line/col)
//!  - a large/explosive in-test model + a tiny bound → `inconclusive`
//!    within a bounded wall-time (the large-FSM-ceiling-guard proof — the
//!    editor does not hang).
//!
//! The harness builds the `LspService` **exactly as `fsm_lsp::run_stdio`
//! does** — `LspService::build(Backend::new).custom_method("fsm/verify",
//! Backend::verify_request).finish()` — so the test exercises the EXACT
//! request the shipped server serves (not a test-only re-wiring). The
//! oracle is the real `fsm` binary; the LSP result is required to deep-
//! equal it, so a forked/diverging verifier in `fsm-lsp` (the keystone
//! regression) makes the assertion fail — that is the point.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tower::{Service, ServiceExt};
use tower_lsp::jsonrpc::Request;
use tower_lsp::lsp_types::Url;
use tower_lsp::LspService;

use fsm_lsp::Backend;

/// The workspace root (two levels up from `crates/fsm-lsp`).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/fsm-lsp has a workspace grandparent")
        .to_path_buf()
}

/// Resolve the **real `fsm` binary** (the in-test oracle — the SAME binary
/// the CI/factory + A1's extension spawn). Strategy, zero-new-dependency
/// (the only A2 `Cargo.toml` delta is the `fsm-verify` edge — no
/// `assert_cmd`/`escargot`): the test executable lives under
/// `<target>/debug/deps/`, so `<target>/debug/fsm` is its sibling-of-parent
/// (the shared `/root/dev/embeded-fsm-sdk-target` per `.cargo/config.toml`).
/// If it is not present (a targeted `cargo test -p fsm-lsp` that didn't
/// build the bin), build it idempotently via the `CARGO` the harness was
/// invoked with (a *build of a different package* — it does not deadlock
/// the running test binary, which holds no build lock during execution).
fn fsm_binary() -> PathBuf {
    // <target>/debug/deps/<thisbin> → <target>/debug/fsm
    if let Ok(exe) = std::env::current_exe() {
        if let Some(debug_dir) = exe.parent().and_then(Path::parent) {
            let cand = debug_dir.join(if cfg!(windows) { "fsm.exe" } else { "fsm" });
            if cand.is_file() {
                return cand;
            }
            // Not built yet — build just the `fsm` bin, idempotently.
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
            let status = Command::new(cargo)
                .args(["build", "-p", "fsm-cli", "--bin", "fsm"])
                .current_dir(workspace_root())
                .status()
                .expect("spawn `cargo build -p fsm-cli --bin fsm`");
            assert!(status.success(), "building the oracle `fsm` binary failed");
            assert!(
                cand.is_file(),
                "oracle `fsm` still absent at {} after build",
                cand.display()
            );
            return cand;
        }
    }
    panic!("cannot resolve the oracle `fsm` binary from current_exe()");
}

/// Run the **oracle**: spawn the real `fsm verify --json [extra] <fixture>`
/// and return its parsed stdout JSON. A non-zero exit is a *verdict*
/// (0/1/2), not a failure — the JSON is still on stdout (the CLI's own
/// contract); exit 3/4 (IO / not-analyzable) is a genuine error here
/// because every battery fixture analyses. This is byte-for-byte what A1's
/// extension consumes.
fn oracle_verify(fixture: &Path, extra: &[&str]) -> Value {
    let out = Command::new(fsm_binary())
        .arg("verify")
        .arg("--json")
        .args(extra)
        .arg(fixture)
        .output()
        .expect("spawn the oracle `fsm verify`");
    let code = out.status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 1 || code == 2,
        "oracle `fsm verify` exit {code} (stderr: {})",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("oracle stdout is UTF-8");
    serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("oracle `fsm verify --json` did not emit JSON: {e}\n{stdout}"))
}

/// Build the LSP service **exactly as `fsm_lsp::run_stdio` does** — the
/// `fsm/verify` custom request registered the SAME way the shipped server
/// registers it (so the test drives the real capability, not a re-wiring).
fn build_service() -> LspService<Backend> {
    let (service, _socket) = LspService::build(Backend::new)
        .custom_method("fsm/verify", Backend::verify_request)
        .finish();
    service
}

/// Issue an id-bearing JSON-RPC request and decode its `result` (the
/// `call_request` shape from the L2..L7 harness, kept local so this
/// differential-oracle file is self-contained).
async fn call<S>(service: &mut S, method: &'static str, params: Value, id: i64) -> Value
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

async fn do_initialize<S>(service: &mut S)
where
    S: Service<Request, Response = Option<tower_lsp::jsonrpc::Response>>,
    S::Error: std::fmt::Debug,
{
    let req = Request::build("initialize")
        .params(json!({ "capabilities": {} }))
        .id(1)
        .finish();
    service
        .ready()
        .await
        .unwrap()
        .call(req)
        .await
        .unwrap()
        .expect("initialize returns a response");
}

/// Drive `fsm/verify` for `fixture` (its on-disk text is the buffer the
/// editor would hold) + `extra` knobs, returning the `result` object
/// (`{ verifyJson, exitCode, error? }`).
async fn lsp_verify(fixture: &Path, machine: Option<&str>, max_states: Option<u64>) -> Value {
    let mut service = build_service();
    do_initialize(&mut service).await;
    let text = std::fs::read_to_string(fixture)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", fixture.display()));
    let uri = Url::from_file_path(fixture).expect("fixture path is absolute");
    let mut params = json!({ "uri": uri, "text": text });
    if let Some(m) = machine {
        params["machine"] = json!(m);
    }
    if let Some(n) = max_states {
        params["maxStates"] = json!(n);
    }
    call(&mut service, "fsm/verify", params, 100).await
}

fn fixture(rel: &str) -> PathBuf {
    workspace_root().join(rel)
}

// ─────────────────────────────────────────────────────────────────────────
// THE differential-oracle battery. Each test asserts the LSP-produced
// `verifyJson` DEEP-EQUALS the real `fsm verify --json` on the SAME fixture
// — editor ≡ CLI by construction (a forked verifier breaks this).
// ─────────────────────────────────────────────────────────────────────────

/// (battery-1) SOUND fixture → the LSP `verifyJson` byte-equals
/// `fsm verify --json`, verdict `verified`, exit 0.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clean_fsm_lsp_verdict_byte_equals_cli_oracle() {
    let fx = fixture("examples/verify/clean.fsm");
    let oracle = oracle_verify(&fx, &[]);
    let res = lsp_verify(&fx, None, None).await;

    assert_eq!(
        res["verifyJson"], oracle,
        "LSP fsm/verify verifyJson MUST deep-equal `fsm verify --json` \
         (the differential oracle — editor ≡ CLI by construction)"
    );
    assert_eq!(res["verifyJson"]["verdict"], json!("verified"));
    assert_eq!(res["exitCode"], json!(0));
    assert_eq!(oracle["verdict"], json!("verified"), "oracle sanity");
}

/// (battery-2) DEADLOCKING fixture → the LSP `verifyJson` byte-equals the
/// oracle, verdict `property-violated`, and the **counterexample witness**
/// is the exact `["ARM"]` event sequence the CLI emits (the headline
/// surface — proven equal, not recomputed).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deadlocking_fsm_lsp_witness_byte_equals_cli_oracle() {
    let fx = fixture("examples/verify/deadlocks.fsm");
    let oracle = oracle_verify(&fx, &[]);
    let res = lsp_verify(&fx, None, None).await;

    assert_eq!(
        res["verifyJson"], oracle,
        "LSP fsm/verify verifyJson MUST deep-equal `fsm verify --json` \
         (witness, config, reachability — all byte-equal)"
    );
    assert_eq!(res["verifyJson"]["verdict"], json!("property-violated"));
    assert_eq!(res["exitCode"], json!(1));
    // The witness is the load-bearing surface — assert the concrete value
    // (and that it equals the oracle's, which the deep-equal above already
    // proves; this makes the contract explicit).
    assert_eq!(
        res["verifyJson"]["properties"]["deadlockFree"]["counterexample"]["witness"],
        json!(["ARM"]),
        "the deadlock counterexample witness must be the exact CLI sequence"
    );
    assert_eq!(
        res["verifyJson"]["properties"]["deadlockFree"]["counterexample"]["witness"],
        oracle["properties"]["deadlockFree"]["counterexample"]["witness"]
    );
}

/// (battery-3) THE false-proven guard (the cardinal verification-UI sin).
/// A tiny `maxStates` makes the bounded search hit its ceiling: the LSP
/// MUST return `inconclusive` (byte-equal to the oracle), and explicitly
/// **NOT** a false `verified`. Mirrors the CLI's own
/// `too_small_bound_is_inconclusive` discipline.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tiny_bound_is_inconclusive_not_false_verified() {
    let fx = fixture("examples/verify/clean.fsm");
    // The SAME `--max-states 1` the oracle gets ⇒ same bound ⇒ same verdict.
    let oracle = oracle_verify(&fx, &["--max-states", "1"]);
    let res = lsp_verify(&fx, None, Some(1)).await;

    assert_eq!(
        res["verifyJson"], oracle,
        "LSP inconclusive verifyJson MUST deep-equal the CLI's inconclusive"
    );
    assert_eq!(
        res["verifyJson"]["verdict"],
        json!("inconclusive"),
        "a hit bound MUST read as inconclusive"
    );
    assert_ne!(
        res["verifyJson"]["verdict"],
        json!("verified"),
        "THE cardinal sin: a bound-hit must NEVER render as a false `verified`"
    );
    assert_eq!(res["exitCode"], json!(2), "exit 2 = inconclusive, NOT 0");
    assert_eq!(res["verifyJson"]["bound"]["hit"], json!(true));
    assert_eq!(oracle["verdict"], json!("inconclusive"), "oracle sanity");
}

/// (battery-4) REACHABILITY-DEFECT fixture → the LSP `verifyJson`
/// byte-equals the oracle, including the `properties.reachability.
/// diagnostics[]` FSM-E0400/FSM-W0602 with their concrete `line`/`col`
/// (the navigable in-editor reveal A1 renders — proven equal to the CLI,
/// not recomputed by a forked reachability analysis).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reachability_defect_diagnostics_byte_equal_cli_oracle() {
    let fx = fixture("editors/vscode/src/test/fixtures/unreachable.fsm");
    let oracle = oracle_verify(&fx, &[]);
    let res = lsp_verify(&fx, None, None).await;

    assert_eq!(
        res["verifyJson"], oracle,
        "LSP fsm/verify verifyJson (incl. reachability diagnostics) MUST \
         deep-equal `fsm verify --json`"
    );
    let diags = &res["verifyJson"]["properties"]["reachability"]["diagnostics"];
    assert!(
        diags.as_array().map(|a| !a.is_empty()).unwrap_or(false),
        "the reachability-defect fixture must yield ≥1 diagnostic, got {diags}"
    );
    // The line/col are the SAME the oracle emits (the deep-equal proves it;
    // assert the concrete contract explicitly — these are what the editor
    // makes click-navigable).
    assert_eq!(
        diags, &oracle["properties"]["reachability"]["diagnostics"],
        "every diagnostic's code/severity/message/line/col must be the CLI's"
    );
    assert_eq!(diags[0]["code"], json!("FSM-E0400"));
    assert_eq!(diags[0]["line"], json!(37));
    assert_eq!(diags[0]["col"], json!(5));
}

/// (battery-5) THE large-FSM-ceiling-guard proof (Doc 31 §1 W-A2 (2); §5.4
/// "a large/explosive FSM does not wedge the capability"). A model whose
/// reachable space is far larger than a tiny bound, verified with that tiny
/// bound, MUST return **inconclusive within a bounded wall-time** (not a
/// hang, not a false `verified`). The model is an *in-test inline string*
/// (NOT a new `.fsm` fixture under `crates/` — the scope boundary): a wide
/// fan-out machine (one source, many target states, many independent
/// counter-incrementing context vars) whose config×context space explodes,
/// driven with `maxStates: 8`. `fsm-verify` is bounded-by-construction so
/// it returns within the bound; the assertion is that the LSP round-trip
/// completes FAST and honestly. (The deep-equal-vs-oracle property is
/// already proven on the canon battery 1–4; this test's job is the
/// ceiling-guard timing + the honest-inconclusive outcome.)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn large_explosive_fsm_short_circuits_inconclusive_within_bounded_time() {
    // A deliberately explosive model: 12 independent boolean-ish context
    // vars each toggled by its own event from the single hub state ⇒ the
    // reachable context space is ~2^12 even before state interleavings,
    // dwarfing `maxStates: 8`. Inline (a test string, not a fixture file).
    let mut src = String::from("language fsm 2.0\n\nmachine Explode {\n  context {\n");
    for i in 0..12 {
        src.push_str(&format!("    v{i}: u8 = 0\n"));
    }
    src.push_str("  }\n\n  events {\n");
    for i in 0..12 {
        src.push_str(&format!("    E{i}\n"));
    }
    src.push_str("  }\n\n  initial Hub\n\n  state Hub {\n");
    for i in 0..12 {
        // Each event is an internal (no-target) transition on Hub whose
        // action flips its own context var — a huge reachable context
        // space (~2^12), all from one state. `on E : ctx.f = expr` is the
        // DSL transition-action form (Doc 04 §action_list; e.g.
        // `crates/fsm-parser/tests/grammar.rs:234`).
        src.push_str(&format!("    on E{i} : ctx.v{i} = 1\n"));
    }
    src.push_str("  }\n}\n");

    let mut service = build_service();
    do_initialize(&mut service).await;
    let uri = Url::parse("file:///tmp/explode-a2.fsm").unwrap();
    let params = json!({ "uri": uri, "text": src, "maxStates": 8 });

    let started = Instant::now();
    let res = call(&mut service, "fsm/verify", params, 200).await;
    let elapsed = started.elapsed();

    // The capability did NOT wedge: the bounded explorer + the explicit
    // tiny ceiling short-circuit promptly. A very generous ceiling (the
    // editor-hang bar is "seconds-to-minutes"; a bounded `fsm-verify` run
    // at maxStates=8 is milliseconds — 20s is orders of magnitude of slack
    // yet still fails a genuine hang).
    assert!(
        elapsed < Duration::from_secs(20),
        "large-FSM ceiling guard FAILED: fsm/verify took {elapsed:?} \
         (a bounded run at maxStates=8 must be near-instant — a hang here \
         is the editor-wedge regression Doc 31 §1 W-A2 (2) forbids)"
    );
    // …and it short-circuited HONESTLY: inconclusive (bound hit), never a
    // false `verified` on a truncated explosive search.
    assert_eq!(
        res["verifyJson"]["verdict"],
        json!("inconclusive"),
        "an explosive model under a tiny bound MUST be inconclusive"
    );
    assert_ne!(
        res["verifyJson"]["verdict"],
        json!("verified"),
        "THE cardinal sin: never a false `verified` on a truncated search"
    );
    assert_eq!(res["verifyJson"]["bound"]["hit"], json!(true));
    assert_eq!(res["exitCode"], json!(2));
}

/// (battery-6) A model that does NOT analyse → an honest exit-4 error,
/// `verifyJson: null`, NEVER a fabricated clean verdict (the cardinal-sin
/// bar at the not-analyzable boundary — mirrors the CLI's exit-4
/// "cannot verify what won't compile"). This is the negative half of the
/// honesty contract.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unanalyzable_model_is_honest_error_not_fake_verdict() {
    let mut service = build_service();
    do_initialize(&mut service).await;
    let uri = Url::parse("file:///tmp/broken-a2.fsm").unwrap();
    // A machine with a state but no `initial` ⇒ FSM-E0107 (analysis
    // error) ⇒ cannot be verified.
    let src = "language fsm 2.0\n\nmachine M {\n  state S {}\n}\n";
    let params = json!({ "uri": uri, "text": src });
    let res = call(&mut service, "fsm/verify", params, 201).await;

    assert!(
        res["verifyJson"].is_null(),
        "an unanalyzable model MUST yield verifyJson:null, NOT a verdict; got {res}"
    );
    assert_eq!(
        res["exitCode"],
        json!(4),
        "exit 4 = not-analyzable (the CLI's own bucket), distinct from a verdict"
    );
    assert!(
        res["error"]
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "the honest 'cannot verify a model that does not compile' reason must be present"
    );
}

/// (battery-7) A malformed request (no `text`) → an honest JSON-RPC
/// invalid-params error, NEVER a verify of empty input. The request-shape
/// half of the cardinal-sin bar.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_request_is_invalid_params_not_empty_verify() {
    let mut service = build_service();
    do_initialize(&mut service).await;
    let req = Request::build("fsm/verify")
        .params(json!({ "uri": "file:///tmp/x.fsm" })) // no `text`
        .id(202)
        .finish();
    let resp = service
        .ready()
        .await
        .unwrap()
        .call(req)
        .await
        .unwrap()
        .expect("a response (an error) is returned");
    let v: Value = serde_json::to_value(resp).unwrap();
    assert!(
        v.get("error").is_some(),
        "a request missing `text` MUST be a JSON-RPC error, not a result; got {v}"
    );
    assert!(
        v["result"].is_null(),
        "a malformed request must NOT produce a verify result"
    );
}
