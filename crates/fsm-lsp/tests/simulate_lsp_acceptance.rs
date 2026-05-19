//! §5.4-debug-W1 behavioural acceptance — the **differential-oracle proof**
//! (Doc 33 §W1 / §2 the KEYSTONE-IN-DEBUG invariant; §5.4; the v1.5-W-A2
//! in-process tower-lsp precedent this mirrors exactly).
//!
//! This is the simulate-surface analogue of W-A2's differential oracle: an
//! **in-process `tower-lsp` client** drives the real `fsm/simulate` custom
//! request through a sequence of ops (`load → init → dispatch(+payload) →
//! advanceClock → snapshot → … → restore`) and asserts the concatenated
//! `StepRecord` stream it returns **byte-equals** `fsm_simulator`'s OWN
//! `execute_trace` output for the *identical* `TraceCommand` sequence —
//! where `execute_trace` is the EXACT same function `fsm test`'s
//! `cmd/test.rs::execute_trace` calls (GT-8). Symbol/capability-presence
//! ("the request is registered", "the handler exists") is explicitly NOT
//! acceptance — the byte-equal `StepRecord` stream across the battery, the
//! verbatim-`StepError` surface, and the zero-`StepRecord` `setContext`
//! contract are the proof (the #128 / F-2 / PD-2 standard).
//!
//! Why this proves the keystone: both sides serialise the SAME
//! `fsm_simulator::StepRecord` type through `serde_json`. Byte-equality of
//! the two JSON streams therefore holds **iff** the LSP surface produced the
//! *same ordered records* the shipped `Interpreter` produces — i.e. iff the
//! `fsm/simulate*` layer is a pure marshalling frontend of the one oracle
//! and computes **no** transition/guard/step/config of its own. A forked or
//! diverging simulator in `fsm-lsp` (the cardinal regression Doc 33 §2
//! forbids) makes the assertion fail — that is the point.
//!
//! The harness builds the `LspService` **exactly as `fsm_lsp::run_stdio`
//! does** — `LspService::build(Backend::new).custom_method("fsm/verify",
//! Backend::verify_request).custom_method("fsm/simulate",
//! Backend::simulate_request).finish()` — so the test exercises the EXACT
//! request the shipped server serves (not a test-only re-wiring). The
//! oracle is `fsm_simulator::execute_trace` itself (the same library
//! `cmd/test.rs` drives), required to be byte-equal — no second
//! implementation anywhere.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fsm_simulator::{
    execute_trace, InitTrace, StepKind, TraceCommand, TraceFile, Value as SimValue,
};
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

fn fixture(rel: &str) -> PathBuf {
    workspace_root().join(rel)
}

/// Build the LSP service **exactly as `fsm_lsp::run_stdio` does** — both
/// custom requests registered the SAME way the shipped server registers
/// them (so the test drives the real capability, not a re-wiring).
fn build_service() -> LspService<Backend> {
    let (service, _socket) = LspService::build(Backend::new)
        .custom_method("fsm/verify", Backend::verify_request)
        .custom_method("fsm/simulate", Backend::simulate_request)
        .finish();
    service
}

/// Issue an id-bearing JSON-RPC request and decode its `result` (the
/// `call`/`result` shape from the W-A2 harness, kept local so this
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

/// The raw JSON-RPC response value (for the error-surface tests).
async fn call_raw<S>(service: &mut S, method: &'static str, params: Value, id: i64) -> Value
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

/// Build the trace `Value`'s internally-tagged serde form (`{type,value}`)
/// — the EXACT shape `TraceCommand`/`InitOptions` carry, so the LSP payload
/// is byte-equal to what `execute_trace`'s `TraceFile` uses.
fn u16v(n: u16) -> Value {
    serde_json::to_value(SimValue::U16(n)).unwrap()
}

/// Run the **oracle**: `fsm_simulator::execute_trace` over the SAME IR +
/// `TraceFile` the LSP ops correspond to, returning its `actual`
/// `Vec<StepRecord>` serialised to JSON (the byte-equality reference). This
/// is the IDENTICAL function `crates/fsm-cli/src/cmd/test.rs::execute_trace`
/// drives — there is no second implementation.
fn oracle_steps(ir: &fsm_ir::Ir, trace: &TraceFile) -> Value {
    let res = execute_trace(ir, trace).expect("oracle execute_trace");
    json!(res.actual)
}

/// Analyse a fixture into IR through the SAME `fsm check` front-end the LSP
/// `fsm/simulate` `load` op uses internally — so the oracle and the LSP see
/// the byte-identical IR (no front-end skew).
fn analyse(fixture: &Path) -> (String, Url, fsm_ir::Ir) {
    let text = std::fs::read_to_string(fixture)
        .unwrap_or_else(|e| panic!("read fixture {}: {e}", fixture.display()));
    let uri = Url::from_file_path(fixture).expect("fixture path is absolute");
    // The CLI front-end: parse + analyze (same as `analyze`/`fsm check`).
    let pr = fsm_parser::parse(&text);
    let result = fsm_analyzer::analyze_with_source(&pr, &fixture.to_string_lossy(), &text);
    let ir = result.ir.expect("fixture analyses to IR");
    (text, uri, ir)
}

/// Concatenate the `steps` arrays the LSP returned across a sequence of ops
/// — the LSP-side `StepRecord` stream (init's records + every dispatch/
/// advanceClock's records, in order), to compare byte-for-byte against the
/// oracle's single `actual` vector.
fn concat_steps(op_results: &[Value]) -> Value {
    let mut all: Vec<Value> = Vec::new();
    for r in op_results {
        if let Some(arr) = r.get("steps").and_then(Value::as_array) {
            all.extend(arr.iter().cloned());
        }
    }
    Value::Array(all)
}

// ─────────────────────────────────────────────────────────────────────────
// THE differential-oracle battery. Each test asserts the LSP-produced
// `StepRecord` stream DEEP-EQUALS `fsm_simulator::execute_trace`'s `actual`
// on the SAME TraceCommands — the debug surface is just another oracle
// frontend (a forked simulator breaks this).
// ─────────────────────────────────────────────────────────────────────────

/// (battery-1) init + dispatch(+payload) + a guarded transition →
/// the LSP `StepRecord` stream byte-equals `execute_trace` on the
/// equivalent `TraceFile`. Uses the vending-machine (context + `COIN`
/// payload + a `[ctx.balance >= ctx.price]` context guard — deterministic,
/// no extern-gated guard), so the stream covers `Init`, `Dispatched`,
/// `Completion` (the `done -> PaymentFinal`) records.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn init_dispatch_payload_stream_byte_equals_execute_trace_oracle() {
    let fx = fixture("examples/vending-machine/vending-machine.fsm");
    let (text, uri, ir) = analyse(&fx);

    // The TraceFile the oracle runs: init (defaults) → COIN(value:100) →
    // COIN(value:100) → DISPENSE (now balance 200 ≥ price 150) → SELECT.
    let trace = TraceFile {
        machine_file: None,
        description: None,
        init: InitTrace::default(),
        steps: vec![
            TraceCommand::Dispatch {
                event: "COIN".into(),
                payload: Some(BTreeMap::from([("value".into(), SimValue::U16(100))])),
            },
            TraceCommand::Dispatch {
                event: "COIN".into(),
                payload: Some(BTreeMap::from([("value".into(), SimValue::U16(100))])),
            },
            TraceCommand::Dispatch {
                event: "DISPENSE".into(),
                payload: None,
            },
            TraceCommand::Dispatch {
                event: "SELECT".into(),
                payload: None,
            },
        ],
        expected: vec![],
    };
    let oracle = oracle_steps(&ir, &trace);

    // The LSP op sequence mirroring that TraceFile, EXACTLY.
    let mut svc = build_service();
    do_initialize(&mut svc).await;
    let iid = "vm-1";

    let load = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "load", "instanceId": iid, "uri": uri, "text": text }),
        10,
    )
    .await;
    assert!(
        load.get("error").is_none(),
        "load must succeed on a clean fixture, got {load}"
    );

    let init = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "init", "instanceId": iid }),
        11,
    )
    .await;
    let d1 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid,
                "event": { "name": "COIN", "payload": { "value": u16v(100) } } }),
        12,
    )
    .await;
    let d2 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid,
                "event": { "name": "COIN", "payload": { "value": u16v(100) } } }),
        13,
    )
    .await;
    let d3 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid,
                "event": { "name": "DISPENSE" } }),
        14,
    )
    .await;
    let d4 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid,
                "event": { "name": "SELECT" } }),
        15,
    )
    .await;

    let lsp_stream = concat_steps(&[init.clone(), d1, d2, d3, d4]);

    assert_eq!(
        lsp_stream, oracle,
        "the LSP fsm/simulate* StepRecord stream MUST deep-equal \
         fsm_simulator::execute_trace's `actual` on the SAME TraceCommands \
         (the differential oracle — the debug surface is just another \
         frontend of the one Interpreter, by construction)"
    );

    // Sanity: the stream is non-trivial and starts with the Init record
    // (proves the assertion is not vacuously equal on two empty arrays).
    let arr = lsp_stream.as_array().expect("stream is an array");
    assert!(
        arr.len() >= 5,
        "expected ≥5 records (init + 4 dispatches' worth), got {}",
        arr.len()
    );
    assert_eq!(
        arr[0]["kind"],
        json!(serde_json::to_value(StepKind::Init).unwrap()),
        "record #0 must be the Init step"
    );

    // The post-run context the LSP reports via getContext is a pure read
    // that equals the oracle's final context (the panel renders the
    // oracle's state, computes nothing — battery-3 exercises the full
    // snapshot/restore byte-equality; this asserts the read consistency).
    let gc = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "getContext", "instanceId": iid }),
        16,
    )
    .await;
    let oracle_final = execute_trace(&ir, &trace).unwrap();
    // Re-derive the oracle's final context by replaying through a fresh
    // interpreter (the byte-equal stream above already proves identical
    // behaviour; this makes the getContext-equals-oracle contract explicit).
    let mut ref_interp = fsm_simulator::Interpreter::new(&ir).unwrap();
    ref_interp
        .init(fsm_simulator::InitOptions {
            machine_name: ir.machines[0].name.clone(),
            ..Default::default()
        })
        .unwrap();
    for cmd in &trace.steps {
        if let TraceCommand::Dispatch { event, payload } = cmd {
            ref_interp
                .dispatch_with_payload(event, payload.clone())
                .unwrap();
        }
    }
    assert_eq!(
        gc["context"],
        json!(ref_interp.context().unwrap()),
        "the LSP getContext MUST equal the oracle's final context (a pure \
         read of the one Interpreter, computed by nothing in the LSP)"
    );
    assert_eq!(
        oracle_final.actual.len(),
        arr.len(),
        "stream length parity with the oracle"
    );
}

/// (battery-2) advance_clock + timers → the LSP stream byte-equals
/// `execute_trace`. Uses the traffic-light (pure `after N ms` timers, no
/// externs, no payload), driving `advanceClock` through several timer
/// fires — covers `Init` + `TimerFired` records (and the history machinery
/// is exercised by the `OVERRIDE`/`RESUME` dispatch around it).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn advance_clock_timer_stream_byte_equals_execute_trace_oracle() {
    let fx = fixture("examples/traffic-light/traffic-light.fsm");
    let (text, uri, ir) = analyse(&fx);

    let trace = TraceFile {
        machine_file: None,
        description: None,
        init: InitTrace::default(),
        steps: vec![
            // Red --2000ms--> GreenAccelerating --2000ms--> Green
            TraceCommand::AdvanceClock { delta_ms: 2000 },
            TraceCommand::AdvanceClock { delta_ms: 2000 },
            // Override to Manual, resume to history (Red again via HAuto).
            TraceCommand::Dispatch {
                event: "OVERRIDE".into(),
                payload: None,
            },
            TraceCommand::Dispatch {
                event: "RESUME".into(),
                payload: None,
            },
            // Another timer tick from the restored history config.
            TraceCommand::AdvanceClock { delta_ms: 2000 },
        ],
        expected: vec![],
    };
    let oracle = oracle_steps(&ir, &trace);

    let mut svc = build_service();
    do_initialize(&mut svc).await;
    let iid = "tl-1";

    call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "load", "instanceId": iid, "uri": uri, "text": text }),
        20,
    )
    .await;
    let init = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "init", "instanceId": iid }),
        21,
    )
    .await;
    let a1 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "advanceClock", "instanceId": iid, "deltaMs": 2000 }),
        22,
    )
    .await;
    let a2 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "advanceClock", "instanceId": iid, "deltaMs": 2000 }),
        23,
    )
    .await;
    let o1 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid, "event": { "name": "OVERRIDE" } }),
        24,
    )
    .await;
    let r1 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid, "event": { "name": "RESUME" } }),
        25,
    )
    .await;
    let a3 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "advanceClock", "instanceId": iid, "deltaMs": 2000 }),
        26,
    )
    .await;

    let lsp_stream = concat_steps(&[init, a1, a2, o1, r1, a3]);

    assert_eq!(
        lsp_stream, oracle,
        "the LSP advanceClock/dispatch StepRecord stream MUST deep-equal \
         execute_trace's `actual` (timer fires + history are 100% the \
         oracle's — the debug surface computes none of it)"
    );
    let arr = lsp_stream.as_array().unwrap();
    assert!(
        arr.len() >= 4,
        "expected several records, got {}",
        arr.len()
    );
    assert!(
        arr.iter()
            .any(|r| r["kind"] == json!(serde_json::to_value(StepKind::TimerFired).unwrap())),
        "the stream must contain ≥1 TimerFired record (the oracle's)"
    );
}

/// (battery-3) snapshot + restore = byte-identical rewind. Take a snapshot
/// mid-run, dispatch further, restore, and assert the post-restore config +
/// context the LSP reports **byte-equals** an independent oracle replay to
/// exactly the snapshot point (the time-travel keystone — `Interpreter::
/// {snapshot,restore}`, no second mechanism). Also asserts a fresh dispatch
/// after restore produces the SAME records as the oracle continuing from
/// that point (rewind is exact, not approximate).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_restore_rewind_is_byte_identical_to_oracle_replay() {
    let fx = fixture("examples/vending-machine/vending-machine.fsm");
    let (text, uri, ir) = analyse(&fx);

    let mut svc = build_service();
    do_initialize(&mut svc).await;
    let iid = "vm-rw";

    call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "load", "instanceId": iid, "uri": uri, "text": text }),
        30,
    )
    .await;
    call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "init", "instanceId": iid }),
        31,
    )
    .await;
    // One COIN(100), then snapshot HERE (balance == 100, pre second coin).
    call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid,
                "event": { "name": "COIN", "payload": { "value": u16v(100) } } }),
        32,
    )
    .await;
    let snap = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "snapshot", "instanceId": iid }),
        33,
    )
    .await;
    let snap_idx = snap["snapshotIndex"].as_u64().expect("snapshotIndex");

    // The state at the snapshot point, per the LSP (a pure getContext read).
    let at_snap = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "getContext", "instanceId": iid }),
        34,
    )
    .await;

    // Independent oracle: replay JUST the init + one COIN(100) → its final
    // config/context is what the snapshot point MUST be.
    let trace_to_snap = TraceFile {
        machine_file: None,
        description: None,
        init: InitTrace::default(),
        steps: vec![TraceCommand::Dispatch {
            event: "COIN".into(),
            payload: Some(BTreeMap::from([("value".into(), SimValue::U16(100))])),
        }],
        expected: vec![],
    };
    {
        // Build a fresh interpreter, replay to the snapshot point, read its
        // config+context — the byte-equality reference for the rewind.
        let mut interp = fsm_simulator::Interpreter::new(&ir).unwrap();
        interp
            .init(fsm_simulator::InitOptions {
                machine_name: ir.machines[0].name.clone(),
                ..Default::default()
            })
            .unwrap();
        interp
            .dispatch_with_payload(
                "COIN",
                Some(BTreeMap::from([("value".into(), SimValue::U16(100))])),
            )
            .unwrap();
        let ref_cfg = json!(interp.current_states_named());
        let ref_ctx = json!(interp.context().unwrap());
        assert_eq!(
            at_snap["configuration"]["activeStates"], ref_cfg,
            "the LSP config at the snapshot point MUST equal the oracle \
             replayed to that point"
        );
        assert_eq!(
            at_snap["context"], ref_ctx,
            "the LSP context at the snapshot point MUST equal the oracle's"
        );
        // The oracle's full `actual` to the snapshot point (sanity).
        let _ = oracle_steps(&ir, &trace_to_snap);
    }

    // Dispatch FURTHER (balance → 200, DISPENSE succeeds), then REWIND.
    call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid,
                "event": { "name": "COIN", "payload": { "value": u16v(100) } } }),
        35,
    )
    .await;
    call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid, "event": { "name": "DISPENSE" } }),
        36,
    )
    .await;
    let restored = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "restore", "instanceId": iid, "snapshotIndex": snap_idx }),
        37,
    )
    .await;

    // After restore, the config+context MUST be byte-identical to the
    // snapshot point (the oracle's `restore`, not a recomputation).
    assert_eq!(
        restored["configuration"]["activeStates"], at_snap["configuration"]["activeStates"],
        "post-restore config MUST byte-equal the snapshot point (rewind is \
         exact — Interpreter::restore, the keystone time-travel)"
    );
    assert_eq!(
        restored["context"], at_snap["context"],
        "post-restore context MUST byte-equal the snapshot point"
    );

    // And a fresh dispatch from the restored state branches EXACTLY as the
    // oracle continuing from the snapshot point: balance 100 + COIN(100) =
    // 200, DISPENSE now succeeds — byte-equal to a clean replay of
    // [init, COIN, COIN, DISPENSE].
    let post = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid,
                "event": { "name": "COIN", "payload": { "value": u16v(100) } } }),
        38,
    )
    .await;
    let post2 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid, "event": { "name": "DISPENSE" } }),
        39,
    )
    .await;
    let branch_stream = concat_steps(&[post, post2]);

    // Oracle: the records produced by COIN then DISPENSE *starting from the
    // snapshot config* == the tail of a full [init,COIN,COIN,DISPENSE] run
    // after the first COIN's records.
    let full = TraceFile {
        machine_file: None,
        description: None,
        init: InitTrace::default(),
        steps: vec![
            TraceCommand::Dispatch {
                event: "COIN".into(),
                payload: Some(BTreeMap::from([("value".into(), SimValue::U16(100))])),
            },
            TraceCommand::Dispatch {
                event: "COIN".into(),
                payload: Some(BTreeMap::from([("value".into(), SimValue::U16(100))])),
            },
            TraceCommand::Dispatch {
                event: "DISPENSE".into(),
                payload: None,
            },
        ],
        expected: vec![],
    };
    let full_actual = execute_trace(&ir, &full).unwrap().actual;
    let to_snap_actual = execute_trace(&ir, &trace_to_snap).unwrap().actual;
    // The post-snapshot tail = full minus the prefix produced up to the
    // snapshot point.
    let tail = json!(full_actual[to_snap_actual.len()..].to_vec());
    assert_eq!(
        branch_stream, tail,
        "a dispatch from the RESTORED state MUST produce byte-identical \
         records to the oracle continuing from that exact point — rewind \
         is exact, the branch is the oracle's"
    );
}

/// (battery-4) `setContext` emits **ZERO** StepRecord and is NOT a step
/// (DBGUX §3.3 — the single sanctioned non-stepping helper). Force-set a
/// context field; assert the response carries `steps: []`, the context
/// changed, and the active configuration is UNCHANGED (no transition ran).
/// Then prove behaviour still flows through the one oracle afterwards.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_context_field_emits_zero_step_record_and_is_not_a_step() {
    let fx = fixture("examples/vending-machine/vending-machine.fsm");
    let (text, uri, ir) = analyse(&fx);

    let mut svc = build_service();
    do_initialize(&mut svc).await;
    let iid = "vm-sc";

    call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "load", "instanceId": iid, "uri": uri, "text": text }),
        40,
    )
    .await;
    let init = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "init", "instanceId": iid }),
        41,
    )
    .await;
    let cfg_before = init["configuration"]["activeStates"].clone();

    // Force-set balance = 175 (≥ price 150) WITHOUT dispatching anything.
    let sc = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "setContext", "instanceId": iid,
                "fields": { "balance": u16v(175) } }),
        42,
    )
    .await;

    assert_eq!(
        sc["steps"],
        json!([]),
        "setContext MUST emit ZERO StepRecord (it runs no transition/guard/\
         step — the single sanctioned non-stepping helper, DBGUX §3.3); \
         got {}",
        sc["steps"]
    );
    assert_eq!(
        sc["context"]["balance"],
        u16v(175),
        "the forced field MUST be written"
    );
    assert_eq!(
        sc["configuration"]["activeStates"], cfg_before,
        "setContext MUST NOT change the active configuration (it is NOT a \
         transition — no step ran)"
    );

    // getContext confirms the write persisted and STILL no records exist
    // anywhere (a pure read, no step).
    let gc = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "getContext", "instanceId": iid }),
        43,
    )
    .await;
    assert_eq!(gc["context"]["balance"], u16v(175));
    assert!(
        gc.get("steps").is_none(),
        "getContext is a pure read — no steps key"
    );

    // Cross-check vs the oracle: an `execute_trace` whose `init.context`
    // pre-seeds balance=175 (the SAME nature as set_context_field — a raw
    // context poke, exactly what InitOptions.initial_context does) lands in
    // the SAME configuration after the SAME first DISPENSE. This proves the
    // helper sets state identically to the init-context path the keystone
    // explicitly sanctions, and that real stepping still flows through the
    // one oracle.
    let d = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid, "event": { "name": "DISPENSE" } }),
        44,
    )
    .await;
    let lsp_after = concat_steps(&[d]);

    let mut ctx0 = BTreeMap::new();
    ctx0.insert("balance".to_string(), SimValue::U16(175));
    let oracle_trace = TraceFile {
        machine_file: None,
        description: None,
        init: InitTrace {
            context: Some(ctx0),
            ..Default::default()
        },
        steps: vec![TraceCommand::Dispatch {
            event: "DISPENSE".into(),
            payload: None,
        }],
        expected: vec![],
    };
    // The oracle's records for JUST the DISPENSE (drop its Init record —
    // the LSP's Init happened earlier in `init`, before setContext).
    let oracle_full = execute_trace(&ir, &oracle_trace).unwrap().actual;
    let oracle_dispatch_tail = json!(oracle_full[1..].to_vec());
    assert_eq!(
        lsp_after, oracle_dispatch_tail,
        "after a non-stepping setContext, a real DISPENSE MUST produce \
         byte-identical records to the oracle initialised with the SAME \
         context (set_context_field sets state exactly as \
         InitOptions.initial_context does — the keystone-sanctioned nature)"
    );
}

/// (battery-5) A `StepError` is surfaced **VERBATIM**, never a fabricated
/// clean end-of-run (DBGUX §6 the "inconclusive ≠ done" honest state).
/// Dispatching before `init` MUST return the `NotInitialized` error text +
/// its stable `errorKind`, NOT an empty `steps` "success".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn step_error_is_surfaced_verbatim_not_a_fake_clean_end() {
    let fx = fixture("examples/vending-machine/vending-machine.fsm");
    let (text, uri, _ir) = analyse(&fx);

    let mut svc = build_service();
    do_initialize(&mut svc).await;
    let iid = "vm-err";

    call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "load", "instanceId": iid, "uri": uri, "text": text }),
        50,
    )
    .await;
    // NO init — dispatch must error with NotInitialized, verbatim.
    let d = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid, "event": { "name": "SELECT" } }),
        51,
    )
    .await;

    assert!(
        d.get("steps").is_none(),
        "a StepError MUST NOT be dressed up as an empty-steps success; got {d}"
    );
    assert_eq!(
        d["errorKind"],
        json!("not-initialized"),
        "the StepError variant MUST be surfaced (stable discriminator)"
    );
    let msg = d["error"].as_str().unwrap_or("");
    assert!(
        msg.contains("not initialized"),
        "the StepError Display text MUST be verbatim, got {msg:?}"
    );

    // An unknown event AFTER init is also surfaced verbatim (InvalidEvent),
    // never a silent discard masquerading as a clean run.
    call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "init", "instanceId": iid }),
        52,
    )
    .await;
    let bad = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": iid, "event": { "name": "NOPE_NOT_AN_EVENT" } }),
        53,
    )
    .await;
    assert_eq!(
        bad["errorKind"],
        json!("invalid-event"),
        "an undeclared event MUST surface InvalidEvent verbatim, not a \
         fabricated clean discard; got {bad}"
    );
}

/// (battery-6) A malformed request → an honest JSON-RPC invalid-params
/// error, NEVER a simulate of empty/wrong input (the request-shape half of
/// the cardinal-sin bar, mirroring W-A2 battery-7).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_request_is_invalid_params_not_empty_simulate() {
    let mut svc = build_service();
    do_initialize(&mut svc).await;

    // Missing `op`.
    let v = call_raw(&mut svc, "fsm/simulate", json!({ "instanceId": "x" }), 60).await;
    assert!(
        v.get("error").is_some(),
        "a request missing `op` MUST be a JSON-RPC error, not a result; got {v}"
    );
    assert!(
        v["result"].is_null(),
        "a malformed request must NOT produce a simulate result"
    );

    // Unknown op.
    let v2 = call_raw(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "frobnicate", "instanceId": "x" }),
        61,
    )
    .await;
    assert!(
        v2.get("error").is_some(),
        "an unknown op MUST be a JSON-RPC error; got {v2}"
    );

    // Op on an unknown instance → invalid-params (not a fake empty run).
    let v3 = call_raw(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "dispatch", "instanceId": "never-loaded",
                "event": { "name": "X" } }),
        62,
    )
    .await;
    assert!(
        v3.get("error").is_some(),
        "dispatch on an unknown instance MUST be a JSON-RPC error, never a \
         simulate of nothing; got {v3}"
    );
}

/// (battery-7) `load`/`listInstances`/`unload` session plumbing is correct
/// and ordered (pure bookkeeping, zero semantics — but it must actually
/// work, not be advertised-but-broken).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn session_lifecycle_load_list_unload() {
    let fx = fixture("examples/vending-machine/vending-machine.fsm");
    let (text, uri, _ir) = analyse(&fx);

    let mut svc = build_service();
    do_initialize(&mut svc).await;

    for id in ["b-1", "a-1"] {
        call(
            &mut svc,
            "fsm/simulate",
            json!({ "op": "load", "instanceId": id, "uri": uri, "text": text }),
            70,
        )
        .await;
    }
    let list = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "listInstances" }),
        71,
    )
    .await;
    let ids: Vec<&str> = list["instances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["instanceId"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec!["a-1", "b-1"],
        "listInstances MUST be deterministically ordered by instanceId"
    );
    assert_eq!(
        list["instances"][0]["machineName"], "VendingMachine",
        "the machine name is reported"
    );

    let un = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "unload", "instanceId": "b-1" }),
        72,
    )
    .await;
    assert_eq!(un["success"], json!(true), "unload of a live instance");
    let un2 = call(
        &mut svc,
        "fsm/simulate",
        json!({ "op": "unload", "instanceId": "b-1" }),
        73,
    )
    .await;
    assert_eq!(
        un2["success"],
        json!(false),
        "unload of an already-removed instance is an honest false, not an error"
    );
}
