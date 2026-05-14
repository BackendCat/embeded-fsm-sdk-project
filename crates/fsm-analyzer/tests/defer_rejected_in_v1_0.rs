//! Audit P0-5 option-b — analyzer rejects every `defer EVENT` with
//! FSM-E0903 in v1.0.
//!
//! Background: prior codegen had a runtime path that silently dropped
//! deferred events, violating Doc 02 G1 ("no undefined behaviour"). The
//! fix per audit option-b is to gate `defer` at analysis time until v1.1
//! ships a real queue.
//!
//! These tests are the canary: if a future change re-allows `defer EVENT`
//! to pass analyzer without a working codegen path, the silent-drop
//! regression returns. They MUST stay green until v1.1 lands the real
//! defer queue.

use fsm_analyzer::{analyze, DiagnosticCode};
use fsm_parser::parse;

fn has_code(d: &[fsm_diagnostics::Diagnostic], code: DiagnosticCode) -> bool {
    d.iter().any(|x| x.code == code)
}

fn count_code(d: &[fsm_diagnostics::Diagnostic], code: DiagnosticCode) -> usize {
    d.iter().filter(|x| x.code == code).count()
}

#[test]
fn single_defer_rejected_with_e0903() {
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
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::E0903),
        "expected FSM-E0903 (defer not supported in v1.0); got: {:?}",
        res.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
    );
}

#[test]
fn multiple_defer_sites_each_get_e0903() {
    // Two states each with their own `defer`; expect TWO E0903 diagnostics
    // — one per declaration site, so the user fixes every offending line.
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
    let n = count_code(&res.diagnostics, DiagnosticCode::E0903);
    assert_eq!(
        n,
        2,
        "expected two FSM-E0903 (one per defer); got {n}. Diagnostics: {:?}",
        res.diagnostics
            .iter()
            .map(|d| (d.code, d.message.clone()))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn machine_without_defer_passes_clean() {
    // Guardrail: defer-free machines must not pick up a spurious E0903.
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
    assert!(
        !has_code(&res.diagnostics, DiagnosticCode::E0903),
        "defer-free machine should not raise FSM-E0903; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| (d.code, d.message.clone()))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn e0903_message_mentions_v1_0_and_v1_1() {
    // The message is the user-facing fingerprint of this scope cut; keep
    // it pointing at the v1.1 roadmap so anyone tripping the error knows
    // where the feature is going.
    let pr = parse(
        r#"language fsm 2.0
feature deferred
machine M {
    events { DATA }
    initial S
    state S {
        defer DATA
    }
}"#,
    );
    let res = analyze(&pr);
    let diag = res
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::E0903)
        .expect("FSM-E0903 should be present");
    assert!(
        diag.message.contains("v1.0") && diag.message.contains("v1.1"),
        "FSM-E0903 message should reference v1.0 limit + v1.1 roadmap; got: {:?}",
        diag.message,
    );
}
