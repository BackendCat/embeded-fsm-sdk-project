//! W1 behavioural-acceptance gate (Doc 30 §5.4 / §4.3).
//!
//! These tests are the *acceptance* for the keystone wave. Per Doc 30 §5.4
//! + the P0-1 lesson, **symbol-presence is explicitly NOT acceptance** —
//! "the crate compiles / `verify` exists" is rejected. Each test drives a
//! **real `.fsm` fixture through the real parse+analyze pipeline** and
//! asserts a *behavioural* property, cross-checked against the shipped
//! interpreter (which IS the oracle) and, for the deadlock case, against
//! Doc 08 by hand (the reasoning is in the fixture header + below).
//!
//! Coverage map (the brief's (a)–(e)):
//!   (a) `unreachable_state_is_reported`            — FSM-E0400 backing
//!   (b) `genuine_deadlock_is_detected_with_witness`— deadlock + witness
//!   (c) `sound_machine_is_proven_deadlock_free`    — ProvenNoDeadlock
//!   (d) `too_small_bound_is_inconclusive_not_false_proven` — honest bound
//!   (e) `verdict_is_deterministic_byte_equal`      — reproducibility
//!   +   `verifier_agrees_with_interpreter_oracle`  — the keystone proof
//!   +   `out_of_w1_scope_is_rejected_loudly`       — STOP-not-wrong guard

use std::collections::BTreeSet;

use fsm_simulator::{InitOptions, Interpreter};
use fsm_verify::{verify, Verdict, VerifyOptions, VerifyOutcome};

const DEADLOCK_FIXTURE: &str = include_str!("fixtures/deadlock_guard_trap.fsm");
const SOUND_FIXTURE: &str = include_str!("fixtures/motor_sound.fsm");
const ISLAND_FIXTURE: &str = include_str!("fixtures/unreachable_island.fsm");

/// Compile a fixture through the REAL pipeline (parser → analyzer →
/// IR). Asserts the fixture is clean so a test failure means a verifier
/// defect, never a malformed fixture.
fn ir_of(src: &str) -> fsm_ir::Ir {
    let pr = fsm_parser::parse(src);
    let res = fsm_analyzer::analyze(&pr);
    assert!(
        !res.diagnostics
            .iter()
            .any(|d| d.severity == fsm_diagnostics::Severity::Error),
        "fixture must analyze clean; got: {:?}",
        res.diagnostics
    );
    res.ir.expect("analyzer produced IR")
}

/// Resolve a state's lowered ID by its DSL name (fixtures speak names; the
/// IR/interpreter speak IDs).
fn state_id_by_name(ir: &fsm_ir::Ir, name: &str) -> String {
    use fsm_ir::StateNode;
    for s in &ir.machines[0].root.states {
        match s {
            StateNode::Simple(s) if s.name == name => return s.id.clone(),
            StateNode::Final(f) if f.name == name => return f.id.clone(),
            _ => {}
        }
    }
    panic!("no state named {name:?} in fixture");
}

// ───────────────────────────────────────────────────────────────────────
// (b) A genuine deadlock is detected WITH a concrete counterexample trace,
//     and the witness actually reaches the deadlocked config when replayed
//     through a FRESH interpreter (cross-checked against the oracle).
// ───────────────────────────────────────────────────────────────────────
#[test]
fn genuine_deadlock_is_detected_with_witness() {
    let ir = ir_of(DEADLOCK_FIXTURE);

    let out = verify(&ir, VerifyOptions::default()).expect("verify runs");

    let (report, witness) = match &out.verdict {
        Verdict::Deadlock { report, witness } => (report, witness),
        other => panic!("expected Deadlock, got {other:?}"),
    };

    // The deadlocked configuration must be exactly {Trap}.
    let trap = state_id_by_name(&ir, "Trap");
    assert_eq!(
        report.config,
        vec![trap.clone()],
        "the deadlock must be the Trap state"
    );

    // The witness must be a CONCRETE event sequence. By the fixture's
    // structure (Idle --GO--> Trap) the only path is [GO].
    assert_eq!(
        witness,
        &vec!["GO".to_string()],
        "witness must be the concrete event sequence reaching the deadlock"
    );

    // CROSS-CHECK AGAINST THE ORACLE: replay the witness through a FRESH
    // interpreter and assert we land in the deadlocked config AND that no
    // declared event makes progress from there (Doc 08 §3.1). This proves
    // the witness is valid against the *real* semantics, not the
    // verifier's bookkeeping.
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: ir.machines[0].name.clone(),
            initial_context: None,
            virtual_clock_start_ms: 0,
        })
        .unwrap();
    for ev in witness {
        interp.dispatch(ev).unwrap();
    }
    assert_eq!(
        interp.current_states(),
        vec![trap.clone()],
        "replaying the witness through a fresh interpreter must reach Trap"
    );
    // From Trap, every declared event must leave the config unchanged
    // (genuine stuck — the deadlock predicate, validated on the oracle).
    let at_trap = interp.snapshot().unwrap();
    for ev in &["GO", "UNSTICK"] {
        interp.restore(at_trap.clone()).unwrap();
        let before = interp.current_states();
        interp.dispatch(ev).unwrap();
        assert_eq!(
            interp.current_states(),
            before,
            "event {ev} must NOT make progress from the deadlocked Trap config \
             (Doc 08 §3.1: no enabled transition ⇒ config unchanged)"
        );
    }
}

// ───────────────────────────────────────────────────────────────────────
// (c) A sound machine (fully reachable, deadlock-free) ⇒ ProvenNoDeadlock,
//     and the reachable-set fact lists every concrete state.
// ───────────────────────────────────────────────────────────────────────
#[test]
fn sound_machine_is_proven_deadlock_free() {
    let ir = ir_of(SOUND_FIXTURE);

    let out = verify(&ir, VerifyOptions::default()).expect("verify runs");

    assert_eq!(
        out.verdict,
        Verdict::ProvenNoDeadlock,
        "the motor machine is deadlock-free and must be PROVEN so within a generous bound"
    );
    // ProvenNoDeadlock is only legitimate when the space was exhausted.
    assert!(
        !out.stats.bound_hit(),
        "a proof must come from an exhausted search, not a bound hit"
    );

    // Every concrete state is reachable ⇒ no FSM-E0400 would be emitted.
    assert!(
        out.reachability.unreachable.is_empty(),
        "every state in the motor machine is reachable; got unreachable: {:?}",
        out.reachability.unreachable
    );
    for name in ["Idle", "Running", "Stopped"] {
        assert!(
            out.reachability
                .reachable
                .contains(&state_id_by_name(&ir, name)),
            "{name} must be in the reachable set"
        );
    }
}

// ───────────────────────────────────────────────────────────────────────
// (d) The honest-bound guard (the cardinal verification sin): a machine
//     whose space exceeds the bound ⇒ Inconclusive, and explicitly NOT a
//     false ProvenNoDeadlock / Deadlock.
// ───────────────────────────────────────────────────────────────────────
#[test]
fn too_small_bound_is_inconclusive_not_false_proven() {
    let ir = ir_of(SOUND_FIXTURE);

    let out = verify(
        &ir,
        VerifyOptions {
            max_states: 1, // far smaller than the real reachable space
            ..VerifyOptions::default()
        },
    )
    .expect("verify runs");

    match &out.verdict {
        Verdict::Inconclusive { reason } => {
            assert!(
                reason.contains("NOT a proof"),
                "the inconclusive reason must be explicit that it is not a proof; got: {reason}"
            );
        }
        Verdict::ProvenNoDeadlock => panic!(
            "FALSE-PROVEN: returned ProvenNoDeadlock on a truncated search — \
             the cardinal verification sin (Doc 30 R1)"
        ),
        Verdict::Deadlock { .. } => {
            panic!("must not claim a deadlock on a bound-truncated search")
        }
    }
    assert!(
        out.stats.bound_hit(),
        "stats must record that a bound was hit"
    );
    // The strongest phrasing of the guard: it is NOT the proven variant.
    assert_ne!(out.verdict, Verdict::ProvenNoDeadlock);
}

// ───────────────────────────────────────────────────────────────────────
// (a) An unreachable state is reported (the honest FSM-E0400 backing —
//     Doc 30 §1.3). Reachable part is sound, so deadlock = ProvenNoDeadlock;
//     this isolates the reachability property.
// ───────────────────────────────────────────────────────────────────────
#[test]
fn unreachable_state_is_reported() {
    let ir = ir_of(ISLAND_FIXTURE);

    let out = verify(&ir, VerifyOptions::default()).expect("verify runs");

    let island = state_id_by_name(&ir, "Island");
    assert!(
        out.reachability.unreachable.contains(&island),
        "the Island state is unreachable and must be reported; got unreachable: {:?}",
        out.reachability.unreachable
    );
    // The reachable states must NOT include Island, and MUST include the
    // genuinely reachable ones.
    assert!(!out.reachability.reachable.contains(&island));
    for name in ["Idle", "Active"] {
        assert!(
            out.reachability
                .reachable
                .contains(&state_id_by_name(&ir, name)),
            "{name} is reachable and must be in the reachable set"
        );
    }
    // Reachable subgraph is deadlock-free.
    assert_eq!(out.verdict, Verdict::ProvenNoDeadlock);
}

// ───────────────────────────────────────────────────────────────────────
// (e) Determinism: the serialised outcome is byte-equal across two runs
//     (the project's reproducibility discipline — BTreeSet/sorted output).
// ───────────────────────────────────────────────────────────────────────
#[test]
fn verdict_is_deterministic_byte_equal() {
    let ir = ir_of(DEADLOCK_FIXTURE);

    let a = serialize(&verify(&ir, VerifyOptions::default()).unwrap());
    let b = serialize(&verify(&ir, VerifyOptions::default()).unwrap());
    assert_eq!(
        a, b,
        "verification output must be byte-identical across runs"
    );

    // And for the sound machine (exercises the reachable-set ordering).
    let ir2 = ir_of(SOUND_FIXTURE);
    let c = serialize(&verify(&ir2, VerifyOptions::default()).unwrap());
    let d = serialize(&verify(&ir2, VerifyOptions::default()).unwrap());
    assert_eq!(
        c, d,
        "sound-machine output must be byte-identical across runs"
    );
}

/// Deterministic textual rendering of an outcome for the byte-equality
/// check. Uses sorted/`BTreeSet` fields + `{:?}` over types whose `Debug`
/// is order-stable; this is the W1 stand-in for W2's `--json` (CLI is W2).
fn serialize(out: &VerifyOutcome) -> String {
    format!(
        "verdict={:?}\nreachable={:?}\nunreachable={:?}\nstats={:?}",
        out.verdict,
        out.reachability.reachable.iter().collect::<Vec<_>>(),
        out.reachability.unreachable.iter().collect::<Vec<_>>(),
        out.stats,
    )
}

// ───────────────────────────────────────────────────────────────────────
// THE KEYSTONE PROOF (Doc 30 §4.1): the verifier's reachability honours
// guards EXACTLY as the interpreter does. Construct a machine where a
// "fork the semantics" bug (ignoring the guard, or mis-handling the
// default priority 100) would make the verifier disagree with a direct
// `Interpreter` run — then assert they AGREE. This is the structural proof
// that no second transition-selection semantics was fabricated.
// ───────────────────────────────────────────────────────────────────────
#[test]
fn verifier_agrees_with_interpreter_oracle() {
    // Two transitions on the same (source, event), with STATICALLY
    // DISJOINT guards (`gate==1` vs `gate==0`) so the analyzer's
    // determinism check accepts the fixture (no FSM-E0300). A
    // re-implemented selector that ignored guards would pick the
    // first/wrong branch; the interpreter applies Doc 08 §4 (guard +
    // priority/doc-order). `gate` defaults 0 and is never assigned ⇒ only
    // the `[ctx.gate == 0]` transition is ever enabled, going to `B`; the
    // `[ctx.gate == 1] -> Wrong` branch is unsatisfiable on the reachable
    // context, so `Wrong` must be unreachable.
    let src = r#"language fsm 2.0
machine Pick {
    context { gate: u8 = 0 }
    events { EV }
    initial A
    state A {
        on EV [ctx.gate == 1] -> Wrong
        on EV [ctx.gate == 0] -> B
    }
    state B { on EV -> A }
    state Wrong { on EV -> A }
}"#;
    let ir = ir_of(src);

    // Direct oracle run: from A, dispatch EV — where does the REAL
    // interpreter go?
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: ir.machines[0].name.clone(),
            initial_context: None,
            virtual_clock_start_ms: 0,
        })
        .unwrap();
    interp.dispatch("EV").unwrap();
    let oracle_after_ev = interp.current_states();
    let b_id = state_id_by_name(&ir, "B");
    assert_eq!(
        oracle_after_ev,
        vec![b_id.clone()],
        "sanity: the interpreter (Doc 08 §4) goes A --EV--> B \
         (gate≡0 ⇒ only the [ctx.gate == 0] transition is enabled)"
    );

    // The verifier must therefore have reached B and NEVER `Wrong` (the
    // guarded transition is unsatisfiable on the reachable context). If the
    // verifier had a forked selector that ignored the guard it would have
    // reached `Wrong` — making it reachable. It must not be.
    let out = verify(&ir, VerifyOptions::default()).expect("verify runs");
    let wrong_id = state_id_by_name(&ir, "Wrong");
    assert!(
        out.reachability.reachable.contains(&b_id),
        "verifier must reach B exactly as the interpreter does"
    );
    assert!(
        !out.reachability.reachable.contains(&wrong_id),
        "verifier must NOT reach `Wrong` — the guard is unsatisfiable; reaching it \
         would prove a forked transition-selection semantics (the P0-1 anti-pattern)"
    );
    // A == B reachable, both have an exit ⇒ no deadlock; proves the
    // verifier's whole verdict tracks the oracle.
    assert_eq!(out.verdict, Verdict::ProvenNoDeadlock);

    // Independent confirmation that the reachable set the verifier computed
    // equals an interpreter-driven hand BFS over the same two-event space.
    let mut hand_reachable: BTreeSet<String> = BTreeSet::new();
    let mut i2 = Interpreter::new(&ir).unwrap();
    i2.init(InitOptions {
        machine_name: ir.machines[0].name.clone(),
        initial_context: None,
        virtual_clock_start_ms: 0,
    })
    .unwrap();
    hand_reachable.extend(i2.current_states());
    // From init (A): EV -> B
    let s_a = i2.snapshot().unwrap();
    i2.dispatch("EV").unwrap();
    hand_reachable.extend(i2.current_states());
    // From B: EV -> A
    i2.dispatch("EV").unwrap();
    hand_reachable.extend(i2.current_states());
    i2.restore(s_a).unwrap();
    assert_eq!(
        out.reachability.reachable, hand_reachable,
        "verifier's reachable set must equal an interpreter-driven hand BFS"
    );
}

// ───────────────────────────────────────────────────────────────────────
// STOP-not-wrong: a model outside W1's flat scope is rejected LOUDLY, not
// silently mis-verified (Doc 30 §4.2-W1 + §8). A composite state would make
// the explorer miss completion edges and risk a false deadlock — better an
// explicit error.
// ───────────────────────────────────────────────────────────────────────
#[test]
fn out_of_w1_scope_is_rejected_loudly() {
    let src = r#"language fsm 2.0
machine Hier {
    events { GO }
    initial Outer
    state Outer {
        initial Inner
        state Inner { on GO -> Inner }
    }
}"#;
    let ir = ir_of(src);
    let err = verify(&ir, VerifyOptions::default())
        .expect_err("a composite-state model must be rejected as out of W1 scope");
    let msg = err.to_string();
    assert!(
        msg.contains("flat single-machine") && msg.contains("W2"),
        "the error must clearly state W1-flat-only + point at W2; got: {msg}"
    );
}
