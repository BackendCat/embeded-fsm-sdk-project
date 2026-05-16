//! `fsm/verify` — the v1.5 W-A2 LSP-embedded live verification capability
//! (Doc 31 §1 W-A2 / §2 the keystone-in-UI invariant; Doc 26 §8 the
//! capability + tower-lsp pattern this reuses).
//!
//! # THE CARDINAL INVARIANT (Doc 31 §2 — read it; this is THE rule the
//! post-A2 source-derived §11.3 keystone phase-audit re-derives)
//!
//! This module contains **NO** reachability / deadlock /
//! transition-selection / guard-evaluation / completion / state-exploration
//! logic. It is a *pure frontend* of `fsm-verify`: it
//!
//!  1. parses the buffer via the EXACT existing [`crate::analysis::analyze`]
//!     reuse seam (the SAME `fsm_parser::parse` + `analyze_with_source`
//!     `fsm check` / `fsm verify` run — no second front-end);
//!  2. calls **`fsm_verify::verify`** and **`fsm_verify::reachability_diagnostics`**
//!     — the *identical* public functions
//!     `crates/fsm-cli/src/cmd/verify.rs` calls (GT-6) — and reads back the
//!     [`fsm_verify::VerifyOutcome`] it returns;
//!  3. **marshals that outcome verbatim** into the documented
//!     `fsm-verify/v1` JSON shape `verify.rs`'s `build_json` produces.
//!
//! "Which transition fires", "is this a deadlock", "is this state
//! reachable", "was the bound hit" are **always** `fsm-verify`'s answers,
//! read straight off `VerifyOutcome` / the `Diagnostic`s
//! `reachability_diagnostics` returned. This module decides **none** of
//! them. Marshalling a value-object into the project's documented JSON
//! contract is *data serialisation*, not verification — exactly the
//! category of `verify.rs`'s own `build_json` and A1's
//! `editors/vscode/src/commands/verifyResult.ts` render. The editor verdict
//! is byte-equal to `fsm verify --json` **by construction** (same inputs →
//! same `fsm-verify` call → same documented shape) — that byte-equality is
//! the differential-oracle proof the §5.4 acceptance + the post-A2 audit
//! re-run. A second verifier — even a partial / "fast in-editor"
//! reimplementation — is the exact P0-1 / v1.4-keystone regression the
//! entire epic guards against.
//!
//! # Why the verdict/exit mapping below is mirrored, not re-decided
//!
//! The `verdict` string + `exitCode` + per-property `result` strings are
//! part of the **`fsm-verify/v1` schema contract itself** (defined in
//! `verify.rs`'s module doc), not an independent verification judgment:
//! each is a fixed, total projection of `outcome.verdict` +
//! `has_e0400` (and `has_e0400` is itself derived purely from the
//! `Diagnostic`s `fsm_verify::reachability_diagnostics` produced — an
//! `fsm-verify` output, not a local decision). It is transcribed here
//! **line-for-line from the canonical `verify.rs`** precisely so the
//! editor and the CLI cannot drift: the differential oracle would catch any
//! divergence, and a forked mapping would be the keystone smell. The
//! canonical definition lives in `verify.rs`; this is its faithful mirror
//! for the in-editor frontend, making zero verification decisions of its
//! own. (The boundary forbids touching `fsm-cli`, and `fsm-lsp` must not
//! depend on the CLI *binary* crate — Doc 26 §6 — so the contract shape is
//! re-stated against the documented schema, exactly as A1's TS consumer
//! re-states it; both are honest frontends of the one `fsm-verify`.)
//!
//! # Why params/result are `serde_json::Value`, not a `derive`d struct
//!
//! `fsm-lsp` pulls `serde_json` but NOT a direct `serde`-derive dependency
//! (the only A2 `Cargo.toml` delta is the single `fsm-verify` edge — Doc
//! 31 §1 W-A2). So the request params are read and the response built with
//! manual `serde_json::Value` accessors — the **exact established pattern
//! this crate already uses for LSP params** (`crate::config::InlayHintConfig
//! ::from_settings` reads `workspace/didChangeConfiguration` settings the
//! same way). `serde_json::Value` already satisfies tower-lsp's
//! `FromParams`/`IntoResponse`, so no new dependency is needed.
//!
//! # Trigger / debounce / large-FSM ceiling
//!
//! This is a **request** (`fsm/verify`), not an auto-on-every-keystroke
//! analysis: it runs only when the client explicitly issues it. The
//! debounce + the large-FSM ceiling are enforced **client-side** (the VS
//! Code extension; Doc 31 §1 W-A2 (2)) because the trigger policy is a
//! client UX concern and keeping it there leaves this handler a pure,
//! stateless `Ir → outcome` function (the minimal keystone surface — no
//! server-side timer/lens state machine to audit). `fsm-verify` is itself
//! bounded-by-construction (`max_states`/`max_steps`), so even an
//! unbounded-explosive model returns [`fsm_verify::Verdict::Inconclusive`]
//! within its bound rather than hanging; the client additionally short-
//! circuits very large buffers to an honest "run explicitly" state (it
//! never silently churns) and may pass a tighter `maxStates`/`maxSteps`
//! through the request params to keep the in-editor round-trip snappy while
//! still being honest (a hit bound ⇒ `inconclusive`, never a false
//! `verified`).

use fsm_diagnostics::{compute_line_col, LineColUnit, Severity};
use serde_json::{json, Map, Value};
use tower_lsp::lsp_types::Url;

use crate::analysis::analyze;

/// The parsed `fsm/verify` request parameters.
///
/// `text` is the authoritative in-editor buffer (the client sends it
/// explicitly — the request is stateless w.r.t. the document store, so it
/// works for an unsaved/untitled buffer and never races the debounced
/// `publishDiagnostics`). `uri` is used **only** for the import-security
/// containment boundary, exactly as `analyze`/`fsm check` use the path.
/// `machine` / `max_states` / `max_steps` mirror the CLI's `VerifyArgs`
/// optional knobs 1:1 — absent ⇒ the SAME `fsm_verify` defaults the CLI
/// uses, so a default-knob editor run is byte-equal to a default-knob
/// `fsm verify --json`.
#[derive(Debug, Clone)]
pub struct VerifyRequestParams {
    /// The document URI (for the import-security containment boundary only).
    pub uri: Url,
    /// The authoritative editor buffer to verify.
    pub text: String,
    /// Optional: the machine to verify (empty/absent ⇒ first machine — the
    /// SAME rule `VerifyOptions::machine_name` + `verify.rs` use).
    pub machine: Option<String>,
    /// Optional: `max_states` bound (absent ⇒ `fsm_verify::DEFAULT_MAX_STATES`
    /// — the SAME default `verify.rs` applies).
    pub max_states: Option<usize>,
    /// Optional: `max_steps` bound (absent ⇒ `fsm_verify::DEFAULT_MAX_STEPS`).
    pub max_steps: Option<usize>,
}

impl VerifyRequestParams {
    /// Parse the JSON-RPC `params` object (camelCase keys, the LSP
    /// convention). Defensive: a missing/mistyped optional knob falls back
    /// to "absent" (⇒ the CLI default) rather than erroring — the same
    /// tolerant-read posture `config::InlayHintConfig::from_settings` uses.
    /// `uri` + `text` are required; their absence is a malformed request
    /// (surfaced as an `Err` so the client sees an honest invalid-params,
    /// never a verify of empty/wrong input).
    pub fn from_value(params: &Value) -> Result<Self, String> {
        let uri_str = params
            .get("uri")
            .and_then(Value::as_str)
            .ok_or_else(|| "fsm/verify: missing required string field `uri`".to_owned())?;
        let uri = Url::parse(uri_str)
            .map_err(|e| format!("fsm/verify: `uri` is not a valid URL: {e}"))?;
        let text = params
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| "fsm/verify: missing required string field `text`".to_owned())?
            .to_owned();
        // An empty/whitespace `machine` ⇒ "first machine" (the SAME
        // `unwrap_or_default()` → empty-string rule `verify.rs:152` uses;
        // an empty string and `None` are equivalent to `fsm_verify`).
        let machine = params
            .get("machine")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let max_states = params
            .get("maxStates")
            .and_then(Value::as_u64)
            .map(|n| n as usize);
        let max_steps = params
            .get("maxSteps")
            .and_then(Value::as_u64)
            .map(|n| n as usize);
        Ok(Self {
            uri,
            text,
            machine,
            max_states,
            max_steps,
        })
    }
}

/// Build the `fsm/verify` JSON-RPC response object.
///
/// `verifyJson` is the `fsm-verify/v1` object **byte-equal** to
/// `fsm verify --json`'s stdout JSON (the differential-oracle contract);
/// `exitCode` is the CLI's verdict exit code (0 verified / 1
/// property-violated / 2 inconclusive / 4 not-analyzable) so a client can
/// branch on the SAME contract a CI keys on. When the model does not
/// parse/analyze, `verifyJson` is `null` and `error` carries the same
/// human reason the CLI prints on exit 4 (the honest "cannot verify what
/// won't compile" — never a fabricated clean verdict).
fn response(verify_json: Option<Value>, exit_code: u8, error: Option<String>) -> Value {
    let mut root = Map::new();
    root.insert("verifyJson".into(), verify_json.unwrap_or(Value::Null));
    root.insert("exitCode".into(), json!(exit_code));
    if let Some(e) = error {
        root.insert("error".into(), json!(e));
    }
    Value::Object(root)
}

/// Run `fsm/verify` for `params`: the EXACT `fsm check`/`fsm verify`
/// front-end, then `fsm_verify::verify` + `fsm_verify::reachability_diagnostics`,
/// then marshal the outcome into the documented `fsm-verify/v1` JSON.
///
/// This mirrors `crates/fsm-cli/src/cmd/verify.rs::run` step-for-step (the
/// canonical surface), differing ONLY in the I/O boundary: the CLI reads
/// the file from disk, this reads the in-editor buffer the client sent
/// (the SAME `analyze`-vs-CLI difference Doc 26 §3 documents for every
/// other LSP capability). Every verification datum is `fsm-verify`'s — none
/// is computed here. Returns the `fsm/verify` response JSON.
pub fn run_verify(params: &VerifyRequestParams) -> Value {
    let path = params
        .uri
        .to_file_path()
        .unwrap_or_else(|_| std::path::PathBuf::from("unsaved.fsm"));

    // 1. parse + analyze — the EXACT `fsm check`/`fsm verify` front-end
    //    (`crate::analysis::analyze` == `verify.rs`'s
    //    `parse` + `analyze_with_source`; the one reuse seam, no second
    //    front-end). A model that does not analyze cannot be verified —
    //    surface the SAME exit-4 reason `verify.rs` prints (an honest
    //    "won't compile", never a fabricated verdict — the cardinal sin).
    let analysis = analyze(&params.text, &path);
    if analysis.diagnostics.iter().any(is_error) {
        return response(
            None,
            4,
            Some(
                "the model has analysis errors; cannot verify a model that \
                 does not compile (run `fsm check` to see them)"
                    .to_owned(),
            ),
        );
    }
    let Some(ir) = analysis.ir else {
        return response(
            None,
            4,
            Some("analyzer produced no IR (cannot verify)".to_owned()),
        );
    };

    // 2. THE keystone call — the IDENTICAL public API `verify.rs:108-111`
    //    consumes. `VerifyOptions` mirrors `verify.rs:149-153` exactly
    //    (absent knobs ⇒ the SAME `fsm_verify` defaults the CLI applies).
    let opts = fsm_verify::VerifyOptions {
        max_states: params.max_states.unwrap_or(fsm_verify::DEFAULT_MAX_STATES),
        max_steps: params.max_steps.unwrap_or(fsm_verify::DEFAULT_MAX_STEPS),
        machine_name: params.machine.clone().unwrap_or_default(),
    };
    let outcome = match fsm_verify::verify(&ir, opts) {
        Ok(o) => o,
        Err(e) => {
            // `verify.rs`'s `verify_error_exit`: an out-of-scope / unknown
            // machine / interpreter error is a *usage* problem (exit 4),
            // kept distinct from a verdict — never a fabricated verdict.
            return response(None, 4, Some(format!("{e}")));
        }
    };

    // The selected machine — the SAME selection rule `verify.rs:160-170`
    // uses (named machine, else the first). Structural IR read only.
    let machine = ir
        .machines
        .iter()
        .find(|m| {
            params
                .machine
                .as_deref()
                .map(|n| n == m.name)
                .unwrap_or(false)
        })
        .or_else(|| ir.machines.first())
        .expect("verify() succeeded ⇒ a machine was selected");

    // 3. The honest reachability catalog — the IDENTICAL `fsm-verify`
    //    function `verify.rs:175-176` calls. E0400 only when the search was
    //    exhaustive (decided INSIDE `reachability_diagnostics`, not here).
    let reach_diags = fsm_verify::reachability_diagnostics(
        machine,
        &outcome.reachability,
        outcome.stats.stop_reason,
    );
    let has_e0400 = reach_diags.iter().any(|d| d.severity == Severity::Error);

    // The verdict-label + exit-code projection — transcribed line-for-line
    // from `verify.rs:180-190`. This is the `fsm-verify/v1` *contract*, a
    // total function of `outcome.verdict` + `has_e0400` (both `fsm-verify`
    // outputs); it makes NO verification decision. Mirrored, never
    // re-derived, so the differential oracle is byte-equal by construction.
    let verdict_label = match &outcome.verdict {
        fsm_verify::Verdict::ProvenNoDeadlock if has_e0400 => "property-violated",
        fsm_verify::Verdict::ProvenNoDeadlock => "verified",
        fsm_verify::Verdict::Deadlock { .. } => "property-violated",
        fsm_verify::Verdict::Inconclusive { .. } => "inconclusive",
    };
    let exit_code: u8 = match verdict_label {
        "verified" => 0,
        "property-violated" => 1,
        _ => 2,
    };

    let verify_json = build_verify_json(
        &outcome,
        &machine.name,
        verdict_label,
        exit_code,
        &reach_diags,
        &params.text,
    );

    response(Some(verify_json), exit_code, None)
}

/// True iff a diagnostic is an error (the analyze-gate predicate — the
/// SAME "any errors ⇒ cannot verify" rule `verify.rs` applies via
/// `diagnostics::any_errors`).
fn is_error(d: &fsm_diagnostics::Diagnostic) -> bool {
    d.severity == Severity::Error
}

/// Build the `fsm-verify/v1` JSON object.
///
/// **Byte-equal to `crates/fsm-cli/src/cmd/verify.rs`'s `build_json`** —
/// transcribed key-for-key (sorted-key `Map`, same field names, same value
/// projections) so `fsm/verify`'s result deep-equals `fsm verify --json`'s
/// stdout (the differential-oracle contract — the §5.4 acceptance + the
/// post-A2 keystone audit assert this byte-equality). Every value is read
/// **verbatim** from the `VerifyOutcome` / `Diagnostic`s `fsm-verify`
/// produced: nothing here computes a verification fact. The canonical
/// definition is `verify.rs`'s `build_json`; this is its faithful mirror
/// for the in-editor frontend (the boundary forbids importing the CLI
/// binary crate — Doc 26 §6 — and A1's TS consumer mirrors the very same
/// shape; all three are honest frontends of one `fsm-verify`).
fn build_verify_json(
    outcome: &fsm_verify::VerifyOutcome,
    machine_name: &str,
    verdict_label: &str,
    exit_code: u8,
    reach_diags: &[fsm_diagnostics::Diagnostic],
    src: &str,
) -> Value {
    // `deadlockFree` property — verbatim from `outcome.verdict` (verify.rs
    // `build_json` deadlock block). `config`/`witness` are the
    // `fsm-verify`-produced report/witness, not recomputed.
    let deadlock = match &outcome.verdict {
        fsm_verify::Verdict::Deadlock { report, witness } => json!({
            "result": "violated",
            "counterexample": {
                "config": report.config,
                "witness": witness,
            }
        }),
        fsm_verify::Verdict::Inconclusive { .. } => json!({ "result": "inconclusive" }),
        fsm_verify::Verdict::ProvenNoDeadlock => json!({ "result": "holds" }),
    };

    // `reachability` property. Violated iff an FSM-E0400 was proven —
    // `any_e0400` derived from the `fsm-verify` diagnostics; the
    // `StopReason` is `fsm-verify`'s. Mirrors verify.rs `build_json`.
    let any_e0400 = reach_diags.iter().any(|d| d.severity == Severity::Error);
    let reach_result = match outcome.stats.stop_reason {
        fsm_verify::StopReason::Exhausted if any_e0400 => "violated",
        fsm_verify::StopReason::Exhausted => "holds",
        // A truncated search cannot conclude reachability either way.
        _ => "inconclusive",
    };
    let diag_arr: Vec<Value> = reach_diags
        .iter()
        .map(|d| {
            // The SAME `compute_line_col(.., Scalar)` mapping verify.rs's
            // `build_json` applies — the CLI's 1-based Unicode-scalar
            // coordinate contract (NOT a second/forked position converter).
            let (l, c) = compute_line_col(src, d.span.start, LineColUnit::Scalar);
            json!({
                "code": d.code.to_string(),
                "severity": match d.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                    Severity::Info => "info",
                    Severity::Hint => "hint",
                },
                "message": d.message,
                "line": l,
                "col": c,
            })
        })
        .collect();

    let mut root = Map::new();
    root.insert("schema".into(), json!("fsm-verify/v1"));
    root.insert("machine".into(), json!(machine_name));
    root.insert("verdict".into(), json!(verdict_label));
    root.insert("exitCode".into(), json!(exit_code));

    let mut props = Map::new();
    props.insert("deadlockFree".into(), deadlock);
    props.insert(
        "reachability".into(),
        json!({
            "result": reach_result,
            "reachableStates": outcome.reachability.reachable.iter().collect::<Vec<_>>(),
            "unreachableStates": outcome.reachability.unreachable.iter().collect::<Vec<_>>(),
            "diagnostics": diag_arr,
        }),
    );
    root.insert("properties".into(), Value::Object(props));

    root.insert(
        "bound".into(),
        json!({
            "maxStates": outcome.stats.max_states,
            "maxSteps": outcome.stats.max_steps,
            "configsVisited": outcome.stats.configs_visited,
            "edgesExplored": outcome.stats.edges_explored,
            "hit": outcome.stats.bound_hit(),
            "stopReason": match outcome.stats.stop_reason {
                fsm_verify::StopReason::Exhausted => "exhausted",
                fsm_verify::StopReason::MaxStatesHit => "max-states-hit",
                fsm_verify::StopReason::MaxStepsHit => "max-steps-hit",
            },
        }),
    );
    Value::Object(root)
}
