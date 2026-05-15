//! Phase-audit **P1-2**: a submachine reference (`state X is Sub`) nested
//! inside a composite or parallel state is REJECTED at analysis, so codegen
//! never receives it and the previously-broken `gcc -Werror` path is gone.
//!
//! Background. A *top-level* `state X is Sub` is fully implemented
//! end-to-end (W2a parser → W2b analyzer/IR → W2c simulator → W2d codegen;
//! gcc+sim≡codegen verified). A `state X is Sub` that appears as a
//! *descendant* of a composite/parallel state was NOT implemented: W2d's
//! `collect_sub_refs` walks only the machine's root region, so a nested ref
//! reached codegen as a leaf whose `entry_/exit_` were *called* but whose
//! prototypes were suppressed → dangling calls → C that fails the project's
//! own mandated `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` (the
//! phase-audit reproduced this for both the composite and the parallel
//! nesting). That is the prose-vs-code drift class the audit exists to
//! catch.
//!
//! The honest, bounded fix (the retired-E0903 / `defer` precedent — a
//! not-yet-implemented construct is cleanly *rejected* rather than silently
//! miscompiled): the analyzer emits an error-severity diagnostic for a
//! nested submachine ref and, defence-in-depth, the lowerer refuses to
//! lower a nested ref to `StateNode::Submachine`. Either guard alone keeps
//! codegen from ever seeing the broken construct.
//!
//! Diagnostic code: **FSM-E0502**. Doc 10 §9 reserves no code for the
//! positional constraint and adding a new `DiagnosticCode` variant is out
//! of this wave's scope. Among the existing submachine-family codes, E0502
//! ("submachine instantiation cycle detected") is the only one whose
//! semantics ("this submachine *instantiation structure* is not permitted")
//! and recoverability (**recoverable: No** — code generation is *blocked*,
//! so no broken C is produced) match. E0500/E0501 are recoverable: Yes
//! (compilation continues, emitting exactly the broken C this fix prevents)
//! and would be the wrong semantics. The instance message overrides E0502's
//! default text so the user sees the precise, actionable cause.
//!
//! FAIL-on-main / PASS-after (FSM-PROC-SUBAGENT §5.1):
//!   - on `main` the nested cases analyze CLEAN (`no_errors` true) and lower
//!     a `StateNode::Submachine` for the nested ref (the positive
//!     assertions here would fail, and the lowered nested ref is what made
//!     `fsm generate`'d C fail gcc -Werror);
//!   - after the fix the nested cases emit `FSM-E0502` and lower NO
//!     `StateNode::Submachine` for the nested ref.
//!   - The top-level control proves no regression: it still analyzes clean
//!     and still lowers to `StateNode::Submachine`.

use fsm_analyzer::{analyze, DiagnosticCode};
use fsm_diagnostics::Severity;
use fsm_ir::StateNode;
use fsm_parser::parse;

fn has_code(d: &[fsm_diagnostics::Diagnostic], code: DiagnosticCode) -> bool {
    d.iter().any(|x| x.code == code)
}

fn no_errors(d: &[fsm_diagnostics::Diagnostic]) -> bool {
    !d.iter().any(|x| x.severity == Severity::Error)
}

/// Recursively count every `StateNode::Submachine` anywhere in a machine's
/// state tree (root region + every composite/parallel region, transitively).
/// Used to assert the nested ref produced *no* submachine node while the
/// top-level control produced exactly one.
fn count_submachine_nodes(states: &[StateNode]) -> usize {
    let mut n = 0;
    for s in states {
        match s {
            StateNode::Submachine(_) => n += 1,
            StateNode::Composite(c) => {
                for r in &c.regions {
                    n += count_submachine_nodes(&r.states);
                }
            }
            StateNode::Parallel(p) => {
                for r in &p.regions {
                    n += count_submachine_nodes(&r.states);
                }
            }
            _ => {}
        }
    }
    n
}

/// Total `StateNode::Submachine` count across a machine's root region *and*
/// every submachine template it carries (templates are full `MachineObject`s
/// per Doc 09 §4.11; a nested ref inside a *template* must also be rejected).
fn submachine_nodes_in_ir(ir: &fsm_ir::Ir) -> usize {
    let mut n = 0;
    for m in &ir.machines {
        n += count_submachine_nodes(&m.root.states);
        for sub in &m.submachines {
            n += count_submachine_nodes(&sub.root.states);
        }
    }
    n
}

// ---------------------------------------------------------------------------
// NEGATIVE — nested submachine ref is rejected (composite + parallel).
// ---------------------------------------------------------------------------

/// `state Inner is Sub` nested inside the composite state `Outer`. On `main`
/// this analyzes clean and lowers a `StateNode::Submachine` deep in
/// `Outer`'s region (→ the gcc -Werror failure). After the fix it emits
/// `FSM-E0502` and lowers NO submachine node.
#[test]
fn submachine_ref_nested_in_composite_is_rejected_with_e0502() {
    let src = r#"language fsm 2.0

feature submachines

submachine Sub {
    events { GO DONE_EV }
    initial SubIdle
    state SubIdle { on GO -> SubBusy }
    state SubBusy { on DONE_EV -> SubFinal }
    final SubFinal
}

machine Host {
    events { GO DONE_EV LEAVE }
    initial Outer
    state Outer {
        initial Inner
        state Inner is Sub {
            done -> Leftover
        }
        state Leftover {
            on LEAVE -> Outer
        }
    }
}
"#;
    let res = analyze(&parse(src));

    // FAILS on main: main analyzes the nested ref clean (no_errors == true),
    // so `has_code(.., E0502)` is false there.
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::E0502),
        "nested-in-composite submachine ref must emit FSM-E0502; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| (d.code, &d.message))
            .collect::<Vec<_>>()
    );
    // The diagnostic is error-severity (recoverable: No) — this is what
    // makes `fsm check`/`fsm generate` exit non-zero and abort before
    // codegen (crates/fsm-cli/src/diagnostics.rs::any_errors).
    let e0502 = res
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::E0502)
        .unwrap();
    assert_eq!(e0502.severity, Severity::Error);
    // Actionable message: names the construct + the SUB-FU-2 tracking + the
    // remedy ("top level"). Not a bare default string.
    assert!(
        e0502.message.contains("top-level")
            && e0502.message.contains("SUB-FU-2")
            && e0502.message.contains("is Sub"),
        "E0502 message must be actionable, got: {}",
        e0502.message
    );

    // FAILS on main: main lowers a `StateNode::Submachine` for `Inner`
    // inside `Outer`'s region (count == 1). After the fix the nested ref
    // does NOT lower to a submachine node anywhere — codegen can never
    // receive the broken construct.
    let ir = res.ir.expect("partial IR is still produced (Doc 09 §1)");
    assert_eq!(
        submachine_nodes_in_ir(&ir),
        0,
        "no StateNode::Submachine may be lowered for a nested ref (codegen-safety)"
    );
}

/// `state Inner is Sub` nested inside a region of a parallel state. Same
/// reject + no-lowering guarantees as the composite case (the audit
/// reproduced the identical gcc -Werror failure for the parallel nesting).
#[test]
fn submachine_ref_nested_in_parallel_region_is_rejected_with_e0502() {
    let src = r#"language fsm 2.0

feature submachines
feature parallel

submachine Sub {
    events { GO DONE_EV }
    initial SubIdle
    state SubIdle { on GO -> SubBusy }
    state SubBusy { on DONE_EV -> SubFinal }
    final SubFinal
}

machine Host {
    events { GO DONE_EV LEAVE }
    initial P
    state P {
        region RA {
            initial Inner
            state Inner is Sub {
                done -> Settled
            }
            state Settled { on LEAVE -> Settled }
        }
        region RB {
            initial Bystander
            state Bystander { on LEAVE -> Bystander }
        }
    }
}
"#;
    let res = analyze(&parse(src));

    assert!(
        has_code(&res.diagnostics, DiagnosticCode::E0502),
        "nested-in-parallel-region submachine ref must emit FSM-E0502; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| (d.code, &d.message))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        res.diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::E0502)
            .unwrap()
            .severity,
        Severity::Error
    );

    // Cumulative-diagnostics principle (Doc 09 §1): the pre-existing
    // single-state-region warning (FSM-W0600 on `RB`) is still reported
    // alongside E0502 — the new diagnostic is additive, not a short-circuit.
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::W0600),
        "the unrelated W0600 must still be reported (cumulative diagnostics)"
    );

    let ir = res.ir.expect("partial IR is still produced (Doc 09 §1)");
    assert_eq!(
        submachine_nodes_in_ir(&ir),
        0,
        "no StateNode::Submachine may be lowered for a nested ref (codegen-safety)"
    );
}

/// A nested ref to an *also-nested* template position is rejected even when
/// the offending `state … is Sub` lives inside a `submachine` template body
/// (templates lower as full machines; the same root-region-only codegen
/// limitation applies). The ref is `state DeepRef is Inner` nested inside a
/// composite state *of the Outer submachine template*.
#[test]
fn submachine_ref_nested_inside_a_submachine_template_is_rejected() {
    let src = r#"language fsm 2.0

feature submachines

submachine Inner {
    events { TICK }
    initial I0
    state I0 { on TICK -> IFinal }
    final IFinal
}

submachine Outer {
    events { TICK GO }
    initial Comp
    state Comp {
        initial DeepRef
        state DeepRef is Inner {
            done -> CompDone
        }
        state CompDone { on GO -> Comp }
    }
}

machine Host {
    events { TICK GO }
    initial Run
    state Run is Outer {
        done -> Run
    }
}
"#;
    let res = analyze(&parse(src));
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::E0502),
        "a ref nested inside a submachine TEMPLATE body must also be \
         rejected; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| (d.code, &d.message))
            .collect::<Vec<_>>()
    );
    // Only the *top-level* `state Run is Outer` may lower to a submachine
    // node; the template-internal nested `DeepRef is Inner` must NOT.
    let ir = res.ir.expect("partial IR still produced");
    assert_eq!(
        submachine_nodes_in_ir(&ir),
        1,
        "exactly the top-level `state Run is Outer` lowers to a submachine \
         node; the template-internal nested ref does not"
    );
}

// ---------------------------------------------------------------------------
// CONTROL — top-level `state X is Sub` is UNAFFECTED (no regression).
// ---------------------------------------------------------------------------

/// The canonical supported shape (the W2a–W2d epic's verified path): a
/// top-level `state Connecting is Connection`. This MUST still analyze clean
/// and still lower to exactly one `StateNode::Submachine` — the fix is
/// strictly additive to the *nested* case and may not touch the supported
/// top-level behaviour.
#[test]
fn top_level_submachine_ref_still_analyzes_clean_and_lowers_submachine() {
    let src = r#"language fsm 2.0

feature submachines

submachine Connection {
    events { CONNECT ACK ESTABLISHED }
    initial Idle
    state Idle { on CONNECT -> Handshake }
    state Handshake { on ACK -> Established }
    state Established { on ESTABLISHED -> Done }
    final Done
}

machine Device {
    events { CONNECT ACK ESTABLISHED RECONNECT }
    initial Connecting
    state Connecting is Connection {
        done -> Online
        on RECONNECT -> Connecting
    }
    state Online {
        on RECONNECT -> Connecting
    }
}
"#;
    let res = analyze(&parse(src));

    // No regression: the supported top-level path stays error-free.
    assert!(
        no_errors(&res.diagnostics),
        "top-level submachine ref must remain clean; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| (d.code, &d.message))
            .collect::<Vec<_>>()
    );
    // And it must NOT spuriously trip the nested-ref rejection.
    assert!(
        !has_code(&res.diagnostics, DiagnosticCode::E0502),
        "top-level submachine ref must not emit the nested-ref E0502"
    );

    // It still lowers to exactly one `StateNode::Submachine` (the
    // `Connecting` state), proving the supported pipeline is intact.
    let ir = res.ir.expect("clean IR");
    assert_eq!(
        submachine_nodes_in_ir(&ir),
        1,
        "the top-level `state Connecting is Connection` must still lower to \
         a StateNode::Submachine"
    );
    let connecting = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| match s {
            StateNode::Submachine(sr) if sr.name == "Connecting" => Some(sr),
            _ => None,
        })
        .expect("`Connecting` is a StateNode::Submachine");
    assert_eq!(connecting.submachine_id, "m-Connection");
}
