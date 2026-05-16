//! `fsm verify` — bounded explicit-state verification (v1.4-W2:
//! composite / parallel / history / timer / submachine; W1 was flat-only).
//!
//! The canonical, pipeline-grade verification interface (Doc 30
//! top-section TL architecture decision: CLI is the canonical surface; no
//! server, no daemon, no plugin host). A `fsm verify <file>` invocation
//! behaves exactly like `fsm check` / `fsm generate` — zero daemon, zero
//! network, deterministic — and is driveable by a CI / Make / factory step
//! via the documented exit-code contract + `--json`.
//!
//! ## Exit-code contract (DEFINED + DOCUMENTED here — load-bearing)
//!
//! This subcommand layers a *verification-verdict* contract on top of
//! Doc 18 §3's process-exit buckets. The three verdict codes are
//! **distinct** (the §5.2 / top-section gate explicitly rejects a generic
//! 0/1):
//!
//! | Exit | Meaning | Source |
//! |------|---------|--------|
//! | **0** | **verified** — all properties hold (no deadlock reachable; if the search was exhaustive, no unreachable-state error) | `Verdict::ProvenNoDeadlock` + no `FSM-E0400` |
//! | **1** | **property-violated** — a deadlock is reachable (with a counterexample witness) OR an `FSM-E0400` unreachable-state error was proven | `Verdict::Deadlock` \| any emitted `FSM-E0400` |
//! | **2** | **inconclusive** — a bound was hit; the reachable space was NOT fully explored. **Explicitly NOT "verified".** | `Verdict::Inconclusive` |
//! | **3** | input file not found / unreadable (Doc 18 §3 IO bucket) | — |
//! | **4** | the model does not parse / analyze (cannot verify what won't compile — Doc 18 §3 user-error bucket, kept distinct from a verdict) | — |
//!
//! **Reconciliation note (why exit 2 is a *verdict*, not a tool error).**
//! Doc 18 §3 nominally uses exit 2 for "tool error / invalid args"; clap
//! still emits exit 2 for *its own* usage errors *before* this module
//! runs, so there is no collision (that path never reaches verdict logic).
//! For `fsm verify`, exit 2 is the first-class **INCONCLUSIVE** verdict —
//! the honest-bound outcome (Doc 30 R1: never report "verified" when a
//! bound was hit). This is the binding contract for this subcommand and is
//! the documented surface a factory script keys on.
//!
//! ## `--json` schema (DEFINED here — the machine-readable contract)
//!
//! A single deterministic JSON object on stdout (sorted keys via
//! `BTreeMap`; arrays in stable order — the project's reproducibility
//! discipline):
//!
//! ```jsonc
//! {
//!   "schema": "fsm-verify/v1",
//!   "machine": "Motor",
//!   "verdict": "verified" | "property-violated" | "inconclusive",
//!   "exitCode": 0,
//!   "properties": {
//!     "deadlockFree": {
//!       "result": "holds" | "violated" | "inconclusive",
//!       // present iff violated:
//!       "counterexample": {
//!         "config": ["s-Trap"],          // deadlocked active leaves
//!         "witness": ["GO"]              // event sequence from init
//!       }
//!     },
//!     "reachability": {
//!       "result": "holds" | "violated" | "inconclusive",
//!       "reachableStates": ["s-Idle", ...],     // sorted
//!       "unreachableStates": ["s-Island", ...], // sorted; FSM-E0400 set
//!       "diagnostics": [ { "code": "FSM-E0400", "severity": "error",
//!                          "message": "...", "line": N, "col": N }, ... ]
//!     }
//!   },
//!   "bound": { "maxStates": 100000, "maxSteps": 2000000,
//!              "configsVisited": 3, "edgesExplored": 9,
//!              "hit": false, "stopReason": "exhausted" }
//! }
//! ```
//!
//! ### Schema versioning policy — `fsm-verify/vN` (the factory-contract
//! durability guarantee; the §11.3-W1-audit's named actionable gap)
//!
//! The `schema` field is the **stability contract a factory / CI
//! integrates against**. The versioning rule is **explicit and binding**:
//!
//! - **Additive changes keep the SAME major version** (`fsm-verify/v1`).
//!   *Additive* = a NEW key under an existing object (e.g. a new property
//!   under `properties`, a new field in `bound`), or a NEW element kind in
//!   an existing array (e.g. a new diagnostic `code`). A consumer that
//!   reads only the keys it knows (`properties.deadlockFree`,
//!   `verdict`, `exitCode`, …) is **unaffected** — JSON object readers
//!   ignore unknown keys. **W2's new coverage (composite / parallel /
//!   history / timer / submachine) is purely additive**: the same keys
//!   carry the same meaning; a timer-deadlock counterexample is the
//!   *existing* `properties.deadlockFree.counterexample` shape (its
//!   `witness` may now contain a synthetic `"<timer-fire @ Nms>"` step
//!   alongside declared-event names — a value-space extension of an
//!   existing field, not a shape change), and the reachable/unreachable
//!   sets + `FSM-E0400`/`FSM-W0602` diagnostics are the *existing* arrays.
//!   So W2 stays `fsm-verify/v1` and a W1-era integration does not break.
//! - **A breaking change bumps the major** (`fsm-verify/v2`). *Breaking* =
//!   the *meaning, type, or shape* of an EXISTING key changes, a key is
//!   removed/renamed, or an exit-code's meaning changes. Only then.
//! - The **exit-code contract (0/1/2/3/4) is itself part of the versioned
//!   surface** and is held stable across additive `vN` revisions.
//!
//! A consumer SHOULD therefore branch on the major (`schema` prefix
//! `fsm-verify/v1`) and tolerate unknown keys, rather than pinning an
//! exact byte shape. This sentence-level policy is the contract; it is
//! intentionally co-located with the schema it governs so a factory
//! integrator needs no other doc.

use std::process::ExitCode;

use fsm_analyzer::analyze_with_source;
use fsm_diagnostics::{LineColUnit, Severity};
use fsm_parser::parse;
use fsm_verify::{
    reachability_diagnostics, verify, StopReason, Verdict, VerifyError, VerifyOptions,
};
use serde_json::{json, Map, Value};

use crate::cli::VerifyArgs;
use crate::diagnostics;

pub(crate) fn run(args: VerifyArgs) -> ExitCode {
    let path = &args.file;
    let src = match crate::safe_io::read_to_string_capped(path, 16 * 1024 * 1024) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", path.display(), e);
            return ExitCode::from(3);
        }
    };
    let label = path.to_string_lossy().into_owned();

    // The model must compile before it can be verified — verifying an
    // un-analyzable model is meaningless. Reuse the SAME parse+analyze
    // pipeline `fsm check` / `fsm generate` use (no second front-end).
    let pr = parse(&src);
    let res = analyze_with_source(&pr, &label, &src);
    if diagnostics::any_errors(&res.diagnostics) {
        eprintln!(
            "error: {} has analysis errors; cannot verify a model that does not compile \
             (run `fsm check {}` to see them)",
            path.display(),
            path.display()
        );
        return ExitCode::from(4);
    }
    let Some(ir) = res.ir else {
        eprintln!(
            "error: analyzer produced no IR for {} (cannot verify)",
            path.display()
        );
        return ExitCode::from(4);
    };

    let opts = VerifyOptions {
        max_states: args.max_states.unwrap_or(fsm_verify::DEFAULT_MAX_STATES),
        max_steps: args.max_steps.unwrap_or(fsm_verify::DEFAULT_MAX_STEPS),
        machine_name: args.machine.clone().unwrap_or_default(),
    };

    let outcome = match verify(&ir, opts) {
        Ok(o) => o,
        Err(e) => return verify_error_exit(&e),
    };

    let machine = ir
        .machines
        .iter()
        .find(|m| {
            args.machine
                .as_deref()
                .map(|n| n == m.name)
                .unwrap_or(false)
        })
        .or_else(|| ir.machines.first())
        .expect("verify() succeeded ⇒ a machine was selected");

    // FSM-E0400 / FSM-W0602 — the honest catalog-drift close (Doc 30
    // §1.3). E0400 only when the search was exhaustive (handled inside
    // `reachability_diagnostics`).
    let reach_diags =
        reachability_diagnostics(machine, &outcome.reachability, outcome.stats.stop_reason);
    let has_e0400 = reach_diags.iter().any(|d| d.severity == Severity::Error);

    // ── Exit-code contract.
    let (verdict_label, exit) = match &outcome.verdict {
        Verdict::ProvenNoDeadlock if has_e0400 => ("property-violated", ExitCode::from(1)),
        Verdict::ProvenNoDeadlock => ("verified", ExitCode::SUCCESS),
        Verdict::Deadlock { .. } => ("property-violated", ExitCode::from(1)),
        Verdict::Inconclusive { .. } => ("inconclusive", ExitCode::from(2)),
    };
    let exit_code_num: u8 = match verdict_label {
        "verified" => 0,
        "property-violated" => 1,
        _ => 2,
    };

    if args.json {
        let v = build_json(
            &outcome,
            machine,
            verdict_label,
            exit_code_num,
            &reach_diags,
            &src,
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())
        );
    } else {
        render_human(&outcome, machine, verdict_label, &reach_diags, &src, &label);
    }
    exit
}

fn verify_error_exit(e: &VerifyError) -> ExitCode {
    match e {
        // Out-of-scope / unknown machine / no machines are *usage* problems
        // with the request, not a verification verdict — Doc 18 §3
        // user-error bucket (4), kept distinct from a verdict code so a
        // factory script never confuses "I asked for the wrong thing /
        // an unhandleable model" with "the model is unsafe". The W2
        // explorer covers all in-language shapes, so `OutOfScope` is the
        // durable STOP-not-wrong path (currently the empty set in-language)
        // rather than the W1 flat-only rejection — exit 4 is preserved so a
        // factory CI integrated against W1's contract does not break.
        VerifyError::OutOfScope(_)
        | VerifyError::UnknownMachine(_, _)
        | VerifyError::NoMachines => {
            eprintln!("error: {e}");
            ExitCode::from(4)
        }
        VerifyError::Interpreter(_) => {
            eprintln!("error: verification could not run: {e}");
            ExitCode::from(4)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_json(
    outcome: &fsm_verify::VerifyOutcome,
    machine: &fsm_ir::MachineObject,
    verdict_label: &str,
    exit_code: u8,
    reach_diags: &[fsm_diagnostics::Diagnostic],
    src: &str,
) -> Value {
    // `deadlockFree` property.
    let deadlock = match &outcome.verdict {
        Verdict::Deadlock { report, witness } => json!({
            "result": "violated",
            "counterexample": {
                "config": report.config,
                "witness": witness,
            }
        }),
        Verdict::Inconclusive { .. } => json!({ "result": "inconclusive" }),
        Verdict::ProvenNoDeadlock => json!({ "result": "holds" }),
    };

    // `reachability` property. Violated iff an FSM-E0400 was proven.
    let any_e0400 = reach_diags.iter().any(|d| d.severity == Severity::Error);
    let reach_result = match outcome.stats.stop_reason {
        StopReason::Exhausted if any_e0400 => "violated",
        StopReason::Exhausted => "holds",
        // A truncated search cannot conclude reachability either way.
        _ => "inconclusive",
    };
    let diag_arr: Vec<Value> = reach_diags
        .iter()
        .map(|d| {
            let (l, c) = fsm_diagnostics::compute_line_col(src, d.span.start, LineColUnit::Scalar);
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
    root.insert("machine".into(), json!(machine.name));
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
                StopReason::Exhausted => "exhausted",
                StopReason::MaxStatesHit => "max-states-hit",
                StopReason::MaxStepsHit => "max-steps-hit",
            },
        }),
    );
    Value::Object(root)
}

fn render_human(
    outcome: &fsm_verify::VerifyOutcome,
    machine: &fsm_ir::MachineObject,
    verdict_label: &str,
    reach_diags: &[fsm_diagnostics::Diagnostic],
    src: &str,
    label: &str,
) {
    match &outcome.verdict {
        Verdict::ProvenNoDeadlock => {
            println!(
                "verified: machine '{}' — no deadlock reachable \
                 ({} configurations explored, search exhaustive).",
                machine.name, outcome.stats.configs_visited
            );
        }
        Verdict::Deadlock { report, witness } => {
            println!(
                "property-violated: machine '{}' — DEADLOCK reachable.",
                machine.name
            );
            println!("  deadlocked configuration: {}", report.config.join(", "));
            if witness.is_empty() {
                println!("  counterexample: (the initial configuration is the deadlock)");
            } else {
                println!(
                    "  counterexample (events from init): {}",
                    witness.join(" → ")
                );
            }
        }
        Verdict::Inconclusive { reason } => {
            println!(
                "inconclusive: machine '{}' — NOT verified.\n  {}",
                machine.name, reason
            );
        }
    }
    if !reach_diags.is_empty() {
        println!("\nreachability diagnostics ({}):", reach_diags.len());
        diagnostics::render_human(reach_diags, src, label);
    }
    let _ = verdict_label; // human form embeds it in the lines above
}
