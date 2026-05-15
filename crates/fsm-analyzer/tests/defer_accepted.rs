//! v1.1 — `defer EVENT` is accepted by the analyzer and lowered into the
//! IR defer set.
//!
//! Background: the v1.0 stopgap (audit P0-5 option-b) rejected every
//! `defer EVENT` with `FSM-E0903` because codegen had no working defer
//! queue. v1.1 ships a real per-state defer buffer in both codegen-c and
//! the simulator (Doc 08 §10), so the rejection is removed and E0903 is
//! retired to the `deprecated` submodule. Supersedes docs/00 §11.7.
//!
//! Regression contract (SUBAGENT_CONVENTIONS §5.1): these tests FAIL on
//! `main` (where `defer` is rejected with E0903 + E0903 is a live enum
//! variant) and PASS after the v1.1 defer-runtime wave lands.

use fsm_analyzer::{analyze, DiagnosticCode};
use fsm_diagnostics::Severity;
use fsm_parser::parse;

fn has_code(d: &[fsm_diagnostics::Diagnostic], code: DiagnosticCode) -> bool {
    d.iter().any(|x| x.code == code)
}

fn error_count(d: &[fsm_diagnostics::Diagnostic]) -> usize {
    d.iter().filter(|x| x.severity == Severity::Error).count()
}

#[test]
fn single_defer_analyzes_without_e0903() {
    // On `main` this source is rejected with FSM-E0903; in v1.1 it must
    // analyze clean and the defer set must reach the IR.
    let pr = parse(
        r#"language fsm 2.0
feature deferred
machine M {
    events { DATA OTHER }
    initial S
    state S {
        defer DATA
        on OTHER -> S
    }
}"#,
    );
    let res = analyze(&pr);
    // FSM-E0903 was retired to the `deprecated` submodule — it is no
    // longer a live `DiagnosticCode` variant, so referencing it here
    // would not even compile. The clean-analyze assertion below subsumes
    // "no E0903": zero error-severity diagnostics means defer is accepted.
    assert_eq!(
        error_count(&res.diagnostics),
        0,
        "defer-bearing machine should analyze clean in v1.1; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| (d.code, d.message.clone()))
            .collect::<Vec<_>>(),
    );

    // The defer set must be lowered into the IR so codegen / simulator
    // can consume it (Doc 08 §10 + Doc 09 §4.1 `defers`).
    let ir = res.ir.expect("v1.1: defer-bearing source must produce IR");
    let machine = &ir.machines[0];
    let any_defer = machine.root.states.iter().any(|s| match s {
        fsm_ir::StateNode::Simple(ss) => !ss.defers.is_empty(),
        fsm_ir::StateNode::Composite(c) => !c.defers.is_empty(),
        fsm_ir::StateNode::Parallel(p) => !p.defers.is_empty(),
        _ => false,
    });
    assert!(
        any_defer,
        "lowering must populate `defers` on the deferring state; IR states: {:?}",
        machine
            .root
            .states
            .iter()
            .map(|s| format!("{s:?}"))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn multiple_defer_sites_all_accepted() {
    // Two states each with their own `defer` — both accepted, both
    // lowered. On `main` this produced two FSM-E0903 diagnostics.
    let pr = parse(
        r#"language fsm 2.0
feature hsm
feature deferred
machine M {
    events { A B }
    initial Outer
    state Outer {
        defer A
        initial Inner
        state Inner {
            defer B
        }
    }
}"#,
    );
    let res = analyze(&pr);
    assert_eq!(
        error_count(&res.diagnostics),
        0,
        "nested defer-bearing machine should analyze clean; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| (d.code, d.message.clone()))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn machine_without_defer_still_passes_clean() {
    // Guardrail unchanged from v1.0: defer-free machines analyze clean.
    let pr = parse(
        r#"language fsm 2.0
machine M {
    events { GO }
    initial A
    state A {
        on GO -> B
    }
    state B {
        on GO -> A
    }
}"#,
    );
    let res = analyze(&pr);
    assert_eq!(
        error_count(&res.diagnostics),
        0,
        "defer-free machine should analyze clean; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| (d.code, d.message.clone()))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn defer_plus_explicit_transition_on_same_event_still_e0310() {
    // E0310 is NOT retired — declaring `defer E` AND `on E -> ...` in the
    // same state is a genuine semantic conflict (UML 2.5.1 §14.2.3.9.1:
    // an enabled transition wins, making the `defer` dead). This must
    // still be a hard error in v1.1.
    let pr = parse(
        r#"language fsm 2.0
feature deferred
machine M {
    events { DATA }
    initial S
    state S {
        defer DATA
        on DATA -> S
    }
}"#,
    );
    let res = analyze(&pr);
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::E0310),
        "defer + explicit transition on same event must still raise FSM-E0310; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| (d.code, d.message.clone()))
            .collect::<Vec<_>>(),
    );
}
