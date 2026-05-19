//! `fsm/simulate` — the debug-W1 LSP-embedded interactive-simulation
//! capability (Doc 33 §W1 / §2 the KEYSTONE-IN-DEBUG invariant; the v1.5
//! W-A2 `fsm/verify` `custom_method` pattern this mirrors exactly).
//!
//! # THE CARDINAL INVARIANT (Doc 33 §2 — read it; this is THE rule the
//! post-W1 source-derived §11.3 keystone phase-audit re-derives)
//!
//! This module contains **NO** transition-selection / guard-evaluation /
//! completion-event-synthesis / RTC-step / active-configuration-computation
//! logic. It is a *pure marshalling frontend* of the shipped
//! `fsm_simulator::Interpreter`: every `op` handler below is, by
//! construction, **exactly one shipped `Interpreter` call** plus JSON
//! (de)serialisation. Concretely:
//!
//!  - `init`        → [`Interpreter::init`]                (one call)
//!  - `dispatch`    → [`Interpreter::dispatch_with_payload`] (one call)
//!  - `advanceClock`→ [`Interpreter::advance_clock`]       (one call)
//!  - `getContext`  → [`Interpreter::context`] (+ the read-only
//!                     `current_states_named`/`virtual_clock_ms` accessors)
//!  - `setContext`  → [`Interpreter::set_context_field`]   (the SINGLE
//!                     sanctioned non-stepping additive helper — it runs
//!                     NO transition/guard/step; it is NOT semantics)
//!  - `snapshot`    → [`Interpreter::snapshot`]            (one call)
//!  - `restore`     → [`Interpreter::restore`]             (one call)
//!  - `load`        → [`Interpreter::new`] + per-session bookkeeping
//!  - `listInstances` → a read of the instance map (pure plumbing)
//!  - `unload`      → a remove from the instance map (pure plumbing)
//!
//! "Which transition fires", "is this event discarded", "what is the active
//! configuration", "did a completion event chain", "which timers fired" are
//! **always** the `Interpreter`'s answers, read straight off the
//! `Vec<StepRecord>` / `context()` / `current_states_named()` it returned.
//! This module decides **none** of them. Per-document/-instance session
//! bookkeeping (a `HashMap<instanceId, SimSession>` of `Interpreter` +
//! captured `snapshots`) is *pure plumbing* — zero semantics. The
//! `fsm/simulate*` `StepRecord` stream is therefore **byte-equal to
//! `fsm test`'s `execute_trace`** on the same `TraceCommand`s **by
//! construction** (same inputs → the same single `Interpreter` → the same
//! ordered `Vec<StepRecord>`). That byte-equality is the differential-oracle
//! proof the §5.4 acceptance + the post-W1 audit re-run. A second simulator
//! semantics — even a partial / "fast in-editor" reimplementation — is the
//! exact P0-1 / v1.4-keystone / v1.5-KEYSTONE-IN-UI regression the entire
//! epic guards against.
//!
//! # Why one `op`-dispatched custom request, not N standalone WS methods
//!
//! Doc 13 specifies a *standalone WS daemon* with distinct JSON-RPC methods
//! (`sim/init`, `sim/dispatch`, …). Owner decision D-2 (Doc 33) is to
//! **ride `fsm-lsp`** — NO new daemon, NO new port, NO new network surface —
//! reusing the proven v1.5 W-A2 single-`custom_method` seam. So Doc 13's
//! wire SHAPE (the request/response payloads + the `StepRecord` schema §11)
//! is reused as the message contract *inside* one `fsm/simulate` LSP custom
//! request whose params carry an `op` discriminator. This keeps the
//! keystone-violable surface a **single auditable entry point** (one
//! `custom_method` registration, exactly the W-A2 shape) — a disclosed
//! judgment call faithful to Doc 13's semantics, not a second protocol.
//!
//! # Why params/result are `serde_json::Value`, not a `derive`d struct
//!
//! `fsm-lsp` pulls `serde_json` but NOT a direct `serde`-derive dependency
//! (the only debug-W1 `Cargo.toml` deltas are the single `fsm-simulator`
//! edge + nothing else — Doc 33 §W1). So request params are read and the
//! response built with manual `serde_json::Value` accessors — the **exact
//! established pattern** `crate::capabilities::verify` (W-A2) and
//! `crate::config::InlayHintConfig::from_settings` already use. The
//! interpreter's own types (`InitOptions`, `StepRecord`, `Value`,
//! `InterpreterSnapshot`) are `serde`-`Serialize`/`Deserialize` in
//! `fsm-simulator` itself, so they round-trip through `serde_json::Value`
//! with **no** new dependency and **no** re-modelled schema here — the
//! payloads ARE the interpreter's serde forms (the trace `Value`'s
//! internally-tagged `{type,value}` form, the camelCase `StepRecord`), so
//! they are byte-equal to what `execute_trace`'s `TraceFile` carries.
//!
//! # Statefulness, the long-lived session, and `spawn_blocking`
//!
//! Unlike the stateless `fsm/verify`, a simulation session is inherently
//! stateful (an `Interpreter` carries the live runtime across calls — that
//! IS the design: author → step → inspect → rewind). The session map lives
//! on the [`crate::Backend`] behind a `tokio::sync::Mutex` (the same
//! interior-mutability discipline `Backend`'s `docs`/`debounce`/`inlay_cfg`
//! use). [`run_simulate`] is `async` because it must `.await` that session
//! lock; the actual `Interpreter` call inside each arm is **synchronous**
//! and runs inline — the *identical* execution model `fsm test`'s
//! `execute_trace` uses (it drives the same `Interpreter` synchronously
//! with no offload). This is a **disclosed judgment call**: unlike
//! `fsm/verify` (which offloads to `spawn_blocking` because `fsm-verify`
//! does a potentially huge *unbounded-until-the-bound* state-space
//! exploration), one RTC `dispatch`/`advance_clock` drains a *bounded*
//! queue to quiescence and returns — it is not an unbounded explore, and
//! a debug session is single-interactive-author traffic (one panel), not a
//! contended server path. Holding the brief `tokio::sync::Mutex` guard
//! across the synchronous, bounded `Interpreter` call is therefore correct
//! and does not risk an executor stall the way an unbounded verifier search
//! would; a `spawn_blocking` offload of a session held behind a tokio
//! `Mutex` would add complexity (the guard is not `'static`) for no real
//! responsiveness gain here. (If a future profile shows a pathological
//! machine wedging the reactor, the offload is a localized change — the
//! keystone shape is unaffected.) A malformed request (missing
//! `op`/`instanceId`/required field) is an honest JSON-RPC invalid-params,
//! and a `StepError` is surfaced **verbatim** (never a fabricated clean
//! end-of-run — the "inconclusive ≠ done" honest state, DBGUX §6; the
//! cardinal-sin bar at the request boundary).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use fsm_simulator::{InitOptions, Interpreter, InterpreterSnapshot, StepError, Value as SimValue};
use serde_json::{json, Map, Value};
use tokio::sync::Mutex;
use tower_lsp::lsp_types::Url;

use crate::analysis::analyze;

/// One live simulation session — a long-lived [`Interpreter`] plus the
/// per-revealed-step snapshot ring the client uses for time-travel rewind.
///
/// This struct holds **no** semantics: the `Interpreter` owns 100% of the
/// FSM behaviour; `snapshots` is an ordered capture log the W3 client indexes
/// into (`restore(snapshots[N])`). Pure session plumbing.
#[derive(Debug)]
pub struct SimSession {
    /// THE single semantic oracle for this instance.
    interp: Interpreter,
    /// Human-readable machine name (for `listInstances`).
    machine_name: String,
    /// Captured snapshots, append-only, one per `snapshot` op the client
    /// took (DBGUX §3.1: a snapshot per revealed step ⇒ per-micro-step
    /// rewind). Indexed by the client; this module never interprets them.
    snapshots: Vec<InterpreterSnapshot>,
}

/// The session registry: `instanceId → SimSession`. Lives on
/// [`crate::Backend`] behind a `Mutex`. A `HashMap` is sufficient (instance
/// count is tiny — one per open debug panel); the map is *pure plumbing*,
/// never semantics.
pub type SimSessions = Arc<Mutex<HashMap<String, SimSession>>>;

/// Build an empty session registry (the `Backend::new` initialiser).
pub fn new_sessions() -> SimSessions {
    Arc::new(Mutex::new(HashMap::new()))
}

/// A malformed-request error (surfaced as JSON-RPC invalid-params by the
/// `server.rs` handler). The `String` is the honest human reason — never a
/// simulate-of-empty/wrong input (the cardinal-sin bar at the boundary).
#[derive(Debug)]
pub struct InvalidParams(pub String);

fn want_str(params: &Value, key: &str) -> Result<String, InvalidParams> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            InvalidParams(format!(
                "fsm/simulate: missing required string field `{key}`"
            ))
        })
}

/// Map a [`StepError`] to the response error string, **verbatim** (the
/// `Display` impl carries the honest reason — e.g. the completion-loop
/// message, "not initialized"). Never collapsed to a fake clean verdict.
fn step_error_response(e: &StepError) -> Value {
    let mut root = Map::new();
    root.insert("error".into(), json!(format!("{e}")));
    // A machine-stable discriminator so the client can branch (e.g. show
    // the completion-loop banner) without string-parsing — the variant name
    // only; the human message above is authoritative.
    root.insert("errorKind".into(), json!(step_error_kind(e)));
    Value::Object(root)
}

/// A stable kebab discriminator for the [`StepError`] variant — a pure
/// projection of the variant (no decision made here). The human `error`
/// string remains authoritative; this is for branch-without-parse only.
fn step_error_kind(e: &StepError) -> &'static str {
    match e {
        StepError::NotInitialized => "not-initialized",
        StepError::AlreadyInitialized => "already-initialized",
        StepError::UnknownMachine(_) => "unknown-machine",
        StepError::NoMachines => "no-machines",
        StepError::InvalidEvent { .. } => "invalid-event",
        StepError::QueueOverflow(_) => "queue-overflow",
        StepError::Eval(_) => "eval",
        StepError::GuardEval(_) => "guard-eval",
        StepError::Stmt(_) => "stmt",
        StepError::CompletionLoop(_) => "completion-loop",
        StepError::Internal(_) => "internal",
    }
}

/// Serialise a `Vec<StepRecord>` to its canonical JSON array — the EXACT
/// camelCase serde form `StepRecord` defines (Doc 13 §11), byte-identical to
/// what `execute_trace`'s `TraceResult.actual` serialises to. No reshaping.
fn steps_json(steps: &[fsm_simulator::StepRecord]) -> Value {
    json!(steps)
}

/// The configuration + context read block shared by responses that report
/// post-call state (init/dispatch/advanceClock/getContext). Every field is a
/// **read** off the `Interpreter` (`current_states_named`/`context`/
/// `virtual_clock_ms`) — no computation. `context()` errors only before
/// `init`; here it always follows a successful init/step so it is `Ok`, but
/// the error is still surfaced honestly rather than unwrapped.
fn state_block(interp: &Interpreter) -> Result<Map<String, Value>, StepError> {
    let mut m = Map::new();
    m.insert(
        "configuration".into(),
        json!({ "activeStates": interp.current_states_named() }),
    );
    m.insert("context".into(), json!(interp.context()?));
    m.insert("currentMs".into(), json!(interp.virtual_clock_ms()));
    Ok(m)
}

/// Resolve the IR for a buffer through the **EXACT** `fsm check` reuse seam
/// (`crate::analysis::analyze` — the SAME single front-end every other LSP
/// capability and `fsm/verify` use; no second parser/analyzer). A model that
/// does not analyse cannot be simulated — surface the honest reason, never a
/// fabricated session over empty/broken input (the cardinal-sin bar).
fn ir_for(text: &str, uri: &Url) -> Result<fsm_ir::Ir, String> {
    let path: PathBuf = uri
        .to_file_path()
        .unwrap_or_else(|_| PathBuf::from("unsaved.fsm"));
    let analysis = analyze(text, &path);
    if analysis
        .diagnostics
        .iter()
        .any(|d| d.severity == fsm_diagnostics::Severity::Error)
    {
        return Err("the model has analysis errors; cannot simulate a model \
                     that does not compile (run `fsm check` to see them)"
            .to_owned());
    }
    analysis
        .ir
        .ok_or_else(|| "analyzer produced no IR (cannot simulate)".to_owned())
}

/// An ok response carrying an arbitrary object body.
fn ok(body: Map<String, Value>) -> Value {
    Value::Object(body)
}

/// Run one `fsm/simulate` op. `params` is the JSON-RPC params object; it
/// carries `op` (the discriminator — Doc 13's method tail) plus that op's
/// fields (Doc 13's per-method request shape). Returns the op's response
/// JSON (Doc 13's per-method response shape) or an [`InvalidParams`] for a
/// malformed request (→ JSON-RPC invalid-params at the server boundary).
///
/// **Every arm is exactly one shipped `Interpreter` call + (de)serialisation
/// — the keystone, by construction.** This function is `async` only to lock
/// the session `Mutex`; the `Interpreter` calls themselves are synchronous
/// and bounded (the same execution model `execute_trace` uses) — see the
/// module-level "Statefulness" note for why this is not `spawn_blocking`-
/// offloaded (a disclosed judgment call).
pub async fn run_simulate(sessions: &SimSessions, params: &Value) -> Result<Value, InvalidParams> {
    let op = want_str(params, "op")?;

    match op.as_str() {
        // ── load: Interpreter::new + register the session (plumbing) ──────
        // Doc 13 §4 `sim/load`. The ONLY semantics is `Interpreter::new`
        // (IR indexing); the rest is map bookkeeping.
        "load" => {
            let instance_id = want_str(params, "instanceId")?;
            let text = want_str(params, "text")?;
            let uri_s = want_str(params, "uri")?;
            let uri = Url::parse(&uri_s)
                .map_err(|e| InvalidParams(format!("fsm/simulate: `uri` invalid: {e}")))?;
            let ir = match ir_for(&text, &uri) {
                Ok(ir) => ir,
                Err(reason) => {
                    let mut m = Map::new();
                    m.insert("error".into(), json!(reason));
                    return Ok(ok(m));
                }
            };
            // THE one semantic call.
            let interp = match Interpreter::new(&ir) {
                Ok(i) => i,
                Err(e) => return Ok(step_error_response(&e)),
            };
            let machine_name = params
                .get("machineName")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| ir.machines.first().map(|m| m.name.clone()))
                .unwrap_or_default();
            let mut map = sessions.lock().await;
            map.insert(
                instance_id.clone(),
                SimSession {
                    interp,
                    machine_name: machine_name.clone(),
                    snapshots: Vec::new(),
                },
            );
            let mut m = Map::new();
            m.insert("instanceId".into(), json!(instance_id));
            m.insert("machineName".into(), json!(machine_name));
            Ok(ok(m))
        }

        // ── init: Interpreter::init (one call) ────────────────────────────
        // Doc 13 §5 `sim/init`. `context`/`virtualClockStartMs`/`machineName`
        // are `InitOptions` verbatim (its own serde form).
        "init" => {
            let instance_id = want_str(params, "instanceId")?;
            // `InitOptions` is `Deserialize` in fsm-simulator — build it
            // from the params' optional fields via the interpreter's OWN
            // serde shape (no re-modelled schema here).
            let initial_context = match params.get("context") {
                Some(v) if !v.is_null() => {
                    Some(serde_json::from_value(v.clone()).map_err(|e| {
                        InvalidParams(format!("fsm/simulate init: `context` shape: {e}"))
                    })?)
                }
                _ => None,
            };
            let virtual_clock_start_ms = params
                .get("virtualClockStartMs")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let mut map = sessions.lock().await;
            let s = map.get_mut(&instance_id).ok_or_else(|| {
                InvalidParams(format!("fsm/simulate: unknown instance `{instance_id}`"))
            })?;
            let machine_name = params
                .get("machineName")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| s.machine_name.clone());
            let opts = InitOptions {
                machine_name,
                initial_context,
                virtual_clock_start_ms,
            };
            // THE one semantic call.
            match s.interp.init(opts) {
                Ok(steps) => {
                    s.snapshots.clear();
                    let mut m = state_block(&s.interp).map_err(|e| {
                        InvalidParams(format!("fsm/simulate init: post-init read: {e}"))
                    })?;
                    m.insert("steps".into(), steps_json(&steps));
                    Ok(ok(m))
                }
                Err(e) => Ok(step_error_response(&e)),
            }
        }

        // ── dispatch: Interpreter::dispatch_with_payload (one call) ───────
        // Doc 13 §5 `sim/dispatch`. `event.name` + optional `event.payload`
        // (the trace `Value`'s internally-tagged serde form — byte-equal to
        // what a `TraceCommand::Dispatch` carries).
        "dispatch" => {
            let instance_id = want_str(params, "instanceId")?;
            let event = params
                .get("event")
                .ok_or_else(|| InvalidParams("fsm/simulate dispatch: missing `event`".into()))?;
            let name = event
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| InvalidParams("fsm/simulate dispatch: missing `event.name`".into()))?
                .to_owned();
            let payload: Option<std::collections::BTreeMap<String, SimValue>> =
                match event.get("payload") {
                    Some(v) if !v.is_null() => {
                        Some(serde_json::from_value(v.clone()).map_err(|e| {
                            InvalidParams(format!("fsm/simulate dispatch: `payload` shape: {e}"))
                        })?)
                    }
                    _ => None,
                };
            let mut map = sessions.lock().await;
            let s = map.get_mut(&instance_id).ok_or_else(|| {
                InvalidParams(format!("fsm/simulate: unknown instance `{instance_id}`"))
            })?;
            // THE one semantic call.
            match s.interp.dispatch_with_payload(&name, payload) {
                Ok(steps) => {
                    let mut m = state_block(&s.interp).map_err(|e| {
                        InvalidParams(format!("fsm/simulate dispatch: post-step read: {e}"))
                    })?;
                    m.insert("steps".into(), steps_json(&steps));
                    Ok(ok(m))
                }
                Err(e) => Ok(step_error_response(&e)),
            }
        }

        // ── advanceClock: Interpreter::advance_clock (one call) ───────────
        // Doc 13 §7 `sim/advanceClock`.
        "advanceClock" => {
            let instance_id = want_str(params, "instanceId")?;
            let delta_ms = params
                .get("deltaMs")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    InvalidParams("fsm/simulate advanceClock: missing u64 `deltaMs`".into())
                })?;
            let mut map = sessions.lock().await;
            let s = map.get_mut(&instance_id).ok_or_else(|| {
                InvalidParams(format!("fsm/simulate: unknown instance `{instance_id}`"))
            })?;
            // THE one semantic call.
            match s.interp.advance_clock(delta_ms) {
                Ok(steps) => {
                    let mut m = state_block(&s.interp).map_err(|e| {
                        InvalidParams(format!("fsm/simulate advanceClock: post read: {e}"))
                    })?;
                    m.insert("steps".into(), steps_json(&steps));
                    Ok(ok(m))
                }
                Err(e) => Ok(step_error_response(&e)),
            }
        }

        // ── getContext: Interpreter::context (read-only, no step) ─────────
        // Doc 13 §6 `sim/getContext` (+ the active config + clock readouts,
        // all pure reads).
        "getContext" => {
            let instance_id = want_str(params, "instanceId")?;
            let map = sessions.lock().await;
            let s = map.get(&instance_id).ok_or_else(|| {
                InvalidParams(format!("fsm/simulate: unknown instance `{instance_id}`"))
            })?;
            match state_block(&s.interp) {
                Ok(m) => Ok(ok(m)),
                Err(e) => Ok(step_error_response(&e)),
            }
        }

        // ── setContext: Interpreter::set_context_field (the SINGLE ─────────
        // sanctioned non-stepping additive helper — emits ZERO StepRecord).
        // Doc 13 §6 `sim/setContext`. `fields` is a map of field→Value (the
        // trace `Value` serde form). This is NOT a transition (DBGUX §3.3).
        "setContext" => {
            let instance_id = want_str(params, "instanceId")?;
            let fields_v = params
                .get("fields")
                .ok_or_else(|| InvalidParams("fsm/simulate setContext: missing `fields`".into()))?;
            let fields: std::collections::BTreeMap<String, SimValue> =
                serde_json::from_value(fields_v.clone()).map_err(|e| {
                    InvalidParams(format!("fsm/simulate setContext: `fields` shape: {e}"))
                })?;
            let mut map = sessions.lock().await;
            let s = map.get_mut(&instance_id).ok_or_else(|| {
                InvalidParams(format!("fsm/simulate: unknown instance `{instance_id}`"))
            })?;
            // THE one (non-stepping) call, per field. `set_context_field`
            // emits NO StepRecord by construction (its return type carries
            // none); the active config/clock/timers are untouched.
            for (k, v) in fields {
                if let Err(e) = s.interp.set_context_field(&k, v) {
                    return Ok(step_error_response(&e));
                }
            }
            // The response carries the post-write context + UNCHANGED config
            // + the explicit `steps: []` (zero records — the contract DBGUX
            // §3.3 / the label "no StepRecord" depends on).
            match state_block(&s.interp) {
                Ok(mut m) => {
                    m.insert("steps".into(), json!([]));
                    Ok(ok(m))
                }
                Err(e) => Ok(step_error_response(&e)),
            }
        }

        // ── snapshot: Interpreter::snapshot (one call) + append to ring ───
        // Doc 13 (time-travel substrate). The snapshot is captured by the
        // oracle; we only append it to the per-session ring + return its
        // index so the client can `restore` to it later.
        "snapshot" => {
            let instance_id = want_str(params, "instanceId")?;
            let mut map = sessions.lock().await;
            let s = map.get_mut(&instance_id).ok_or_else(|| {
                InvalidParams(format!("fsm/simulate: unknown instance `{instance_id}`"))
            })?;
            // THE one semantic call.
            match s.interp.snapshot() {
                Ok(snap) => {
                    s.snapshots.push(snap);
                    let index = s.snapshots.len() - 1;
                    let mut m = Map::new();
                    m.insert("snapshotIndex".into(), json!(index));
                    Ok(ok(m))
                }
                Err(e) => Ok(step_error_response(&e)),
            }
        }

        // ── restore: Interpreter::restore (one call) ──────────────────────
        // Doc 13 (time-travel). `snapshotIndex` selects a previously-taken
        // snapshot from the ring; the oracle restores its runtime to it.
        "restore" => {
            let instance_id = want_str(params, "instanceId")?;
            let index = params
                .get("snapshotIndex")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    InvalidParams("fsm/simulate restore: missing u64 `snapshotIndex`".into())
                })? as usize;
            let mut map = sessions.lock().await;
            let s = map.get_mut(&instance_id).ok_or_else(|| {
                InvalidParams(format!("fsm/simulate: unknown instance `{instance_id}`"))
            })?;
            let snap = s.snapshots.get(index).cloned().ok_or_else(|| {
                InvalidParams(format!("fsm/simulate restore: no snapshot #{index}"))
            })?;
            // THE one semantic call.
            match s.interp.restore(snap) {
                Ok(()) => match state_block(&s.interp) {
                    Ok(m) => Ok(ok(m)),
                    Err(e) => Ok(step_error_response(&e)),
                },
                Err(e) => Ok(step_error_response(&e)),
            }
        }

        // ── listInstances: a read of the session map (pure plumbing) ──────
        // Doc 13 §4 `sim/listInstances`.
        "listInstances" => {
            let map = sessions.lock().await;
            let mut instances: Vec<Value> = map
                .iter()
                .map(|(id, s)| json!({ "instanceId": id, "machineName": s.machine_name }))
                .collect();
            // Deterministic order (the map is unordered) — by instanceId.
            instances.sort_by(|a, b| a["instanceId"].as_str().cmp(&b["instanceId"].as_str()));
            let mut m = Map::new();
            m.insert("instances".into(), Value::Array(instances));
            Ok(ok(m))
        }

        // ── unload: remove the session (pure plumbing) ────────────────────
        // Doc 13 §4 `sim/unload`.
        "unload" => {
            let instance_id = want_str(params, "instanceId")?;
            let mut map = sessions.lock().await;
            let existed = map.remove(&instance_id).is_some();
            let mut m = Map::new();
            m.insert("success".into(), json!(existed));
            Ok(ok(m))
        }

        other => Err(InvalidParams(format!(
            "fsm/simulate: unknown op `{other}` (expected one of: load, init, \
             dispatch, advanceClock, getContext, setContext, snapshot, \
             restore, listInstances, unload)"
        ))),
    }
}
