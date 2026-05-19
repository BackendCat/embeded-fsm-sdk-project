//! F-2 — `FSM-E0412` rejecting diagnostic for a non-power-of-two in-source
//! `queue { capacity = N }`.
//!
//! Behavioural acceptance (PD-2 — exercises the *diagnostic path*, not
//! symbol presence). The C99 ring buffer indexes with `& (capacity - 1)`
//! (Doc 11 §12); a non-2^N capacity would silently corrupt the modulo, so
//! the analyzer rejects it LOUDLY (the timer-`FSM-E0411` / `defer`→E0903
//! invalid/deferred-config precedent — reject, never silently round). This
//! is the class-of-issues completion of F-2: ALL silent-misconfig of the
//! queue block, not just the capacity-ignored instance the lowerer fix
//! addressed.

use fsm_analyzer::{analyze, DiagnosticCode};
use fsm_ir::OverflowPolicy;
use fsm_parser::parse;

fn diagnostics(src: &str) -> Vec<fsm_diagnostics::Diagnostic> {
    analyze(&parse(src)).diagnostics
}

const PROLOGUE: &str = "language fsm 2.0\n";

fn machine_with_capacity(cap: &str) -> String {
    format!(
        r#"{PROLOGUE}machine M {{
    events {{ A B }}
    queue {{
        capacity = {cap}
        overflow = assert
    }}
    initial S0
    state S0 {{ on A -> S1 }}
    state S1 {{ on B -> S0 }}
}}"#
    )
}

#[test]
fn non_power_of_two_capacity_is_rejected() {
    for bad in ["3", "30", "7", "100", "1000"] {
        let diags = diagnostics(&machine_with_capacity(bad));
        assert!(
            diags.iter().any(|d| d.code == DiagnosticCode::E0412),
            "capacity = {bad} (NOT 2^N) must emit FSM-E0412; got {diags:#?}"
        );
    }
}

#[test]
fn zero_capacity_is_rejected() {
    // 0 is not a power of two and a zero-length ring is degenerate (the
    // codegen power-of-2 guard rejects it identically).
    let diags = diagnostics(&machine_with_capacity("0"));
    assert!(
        diags.iter().any(|d| d.code == DiagnosticCode::E0412),
        "capacity = 0 must emit FSM-E0412; got {diags:#?}"
    );
}

#[test]
fn power_of_two_capacities_are_accepted() {
    for ok in ["1", "2", "4", "8", "16", "32", "64", "128", "256"] {
        let diags = diagnostics(&machine_with_capacity(ok));
        assert!(
            !diags.iter().any(|d| d.code == DiagnosticCode::E0412),
            "capacity = {ok} IS a power of two — FSM-E0412 must NOT fire; got {diags:#?}"
        );
    }
}

#[test]
fn absent_queue_block_does_not_emit_e0412() {
    // No `queue {}` at all → the IR default (16, a power of two) applies;
    // the check must stay silent (it speaks only to an EXPLICIT in-source
    // capacity literal).
    let src = format!(
        r#"{PROLOGUE}machine M {{
    events {{ A B }}
    initial S0
    state S0 {{ on A -> S1 }}
    state S1 {{ on B -> S0 }}
}}"#
    );
    let diags = diagnostics(&src);
    assert!(
        !diags.iter().any(|d| d.code == DiagnosticCode::E0412),
        "no queue block must not emit FSM-E0412; got {diags:#?}"
    );
}

#[test]
fn lowerer_carries_in_source_queue_values_into_the_ir() {
    // The F-2 ROOT regression test. Pre-fix `lower_queue` found the FIRST
    // value-shaped token in the CONFIG_ENTRY, which is the `capacity` /
    // `overflow` *key* `Ident` (Ident is itself value-shaped) — NOT the
    // RHS. So `capacity = 64` parsed `"capacity"` (u32 parse fails → silent
    // default 16) and `overflow = drop_oldest` matched the catch-all →
    // `Assert`. The whole in-source `queue {}` was silently dropped with a
    // clean `fsm check`. This asserts the lowered IR now carries the
    // ACTUAL declared values (via the typed `ConfigEntry::value()`
    // accessor the analyzer check shares — the F-1 single-source-of-truth).
    let src = r#"language fsm 2.0
machine M {
    events { A B }
    queue {
        capacity = 64
        overflow = drop_oldest
    }
    initial S0
    state S0 { on A -> S1 }
    state S1 { on B -> S0 }
}"#;
    let ir = analyze(&parse(src)).ir.expect("lowered IR");
    let q = &ir.machines[0].queue;
    assert_eq!(
        q.capacity, 64,
        "in-source `capacity = 64` must reach the IR (pre-F-2: silently 16)"
    );
    assert_eq!(
        q.overflow_policy,
        OverflowPolicy::DropOldest,
        "in-source `overflow = drop_oldest` must reach the IR (pre-F-2: silently Assert)"
    );
    assert!(
        q.is_explicit(),
        "an in-source `queue {{}}` block must be `is_explicit()` (drives the F-2 \
         codegen precedence: declared block vs integrator override)"
    );
}

#[test]
fn absent_queue_block_lowers_to_non_explicit_default() {
    let src = r#"language fsm 2.0
machine M {
    events { A B }
    initial S0
    state S0 { on A -> S1 }
    state S1 { on B -> S0 }
}"#;
    let ir = analyze(&parse(src)).ir.expect("lowered IR");
    let q = &ir.machines[0].queue;
    assert!(
        !q.is_explicit(),
        "no `queue {{}}` block ⇒ synthesized default ⇒ NOT explicit (so the F-2 \
         precedence falls through to the codegen default, not a phantom override)"
    );
}

#[test]
fn non_integer_capacity_does_not_emit_e0412() {
    // `capacity = foo` is a separate malformed-config concern the lowerer
    // already tolerates (parse fails → default). E0412 speaks ONLY to the
    // power-of-two invariant of an actual integer literal, so it must not
    // fire here (no false positive on a different error class).
    let diags = diagnostics(&machine_with_capacity("foo"));
    assert!(
        !diags.iter().any(|d| d.code == DiagnosticCode::E0412),
        "non-integer capacity must not emit FSM-E0412; got {diags:#?}"
    );
}
