//! Submachine analyzer + IR-lowering acceptance (v1.1-W2b, epic wave 2/4).
//!
//! W2a made `submachine Name { … }` / `state X is Sub { … }` *parse*. This
//! wave makes it *lower* to correct IR with the right diagnostics. Scope is
//! analyzer + IR only — simulator runtime (W2c) and codegen (W2d) are later
//! waves, so the proof here is "schema-valid IR with the submachine template
//! + reference modeled" and "the four submachine diagnostics fire", NOT a
//! gcc-run (that is W2d's behavioural acceptance).
//!
//! Every test FAILS on `main`:
//!  - the positive assertions fail because `lower_machine` hard-codes
//!    `submachines: Vec::new()` and never emits `StateNode::Submachine` (the
//!    `is` binding is silently dropped);
//!  - the negative assertions fail because the stub `checks/submachine.rs`
//!    token-scans `machines()` only (submachine templates live in the
//!    disjoint `submachines()` view W2a introduced) so it emits none of
//!    FSM-E0103 / FSM-E0610 / FSM-E0500 / FSM-E0501 for these inputs.
//!
//! Per FSM-PROC-SUBAGENT §5.1 (FAIL-on-main / PASS-after) + §5.4 (this is
//! the analyzer-layer acceptance; the end-to-end gcc-run lands with codegen
//! in W2d, which is out of scope here).

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

// ---------------------------------------------------------------------------
// POSITIVE — a valid submachine lowers to schema-valid IR.
// ---------------------------------------------------------------------------

/// The canonical Doc 04 §15 shape: a `submachine Connection { … }` template
/// and a `state Connecting is Connection { done -> Online }` reference.
///
/// Asserts (a) `ir.machines[0].submachines` holds the Connection template
/// lowered structurally like a machine; (b) the `Connecting` state lowered
/// to `StateNode::Submachine` whose `submachine_id` points at that template;
/// (c) the state's own `done ->` completion edge survives on the
/// `SubmachineRef.transitions`; (d) analyze returns clean IR — reaching the
/// final assertion at all means the analyzer's `debug_assert_ir_schema`
/// gate (W0/PD-3) did NOT panic on the new submachine IR, i.e. it is
/// schema-valid.
#[test]
fn submachine_template_and_reference_lower_to_schema_valid_ir() {
    // `feature submachines` is file-scoped per Doc 04 §2.2 (top_level_decl);
    // an in-`machine`-body `feature` is itself a parse error (W2a). This is
    // the canonical valid shape.
    let src = r#"language fsm 2.0

feature submachines

submachine Connection {
    initial Idle
    state Idle { on START -> Online }
    final Online
}

machine Device {
    events { GO }

    initial Connecting

    state Connecting is Connection {
        done -> Online
    }

    state Online { }
}"#;

    let pr = parse(src);
    let res = analyze(&pr);

    assert!(
        no_errors(&res.diagnostics),
        "valid submachine must analyze clean; got: {:?}",
        res.diagnostics
    );

    // Reaching here means `analyze` did not panic in `debug_assert_ir_schema`
    // — the submachine IR is schema-valid (the W0 gate is load-bearing).
    let ir = res.ir.expect("analyzer produced IR");
    let device = ir
        .machines
        .iter()
        .find(|m| m.name == "Device")
        .expect("Device machine lowered");

    // (a) The Connection template is registered as a submachine of Device,
    //     lowered structurally like a machine (its own root region/states).
    assert_eq!(
        device.submachines.len(),
        1,
        "Device must carry the Connection submachine template; got {:?}",
        device
            .submachines
            .iter()
            .map(|s| &s.name)
            .collect::<Vec<_>>()
    );
    let conn = &device.submachines[0];
    assert_eq!(conn.name, "Connection");
    assert!(
        conn.root
            .states
            .iter()
            .any(|s| matches!(s, StateNode::Simple(x) if x.name == "Idle")),
        "Connection template must lower its own Idle state"
    );
    assert!(
        conn.root
            .states
            .iter()
            .any(|s| matches!(s, StateNode::Final(x) if x.name == "Online")),
        "Connection template must lower its own final Online state"
    );

    // (b) The `Connecting` state lowered to a SubmachineRef pointing at the
    //     Connection template's machine id.
    let connecting = device
        .root
        .states
        .iter()
        .find_map(|s| match s {
            StateNode::Submachine(sr) if sr.name == "Connecting" => Some(sr),
            _ => None,
        })
        .expect("`state Connecting is Connection` lowered to StateNode::Submachine");
    assert_eq!(
        connecting.submachine_id, conn.id,
        "SubmachineRef.submachine_id must resolve to the Connection template id"
    );

    // (c) The state's own `done -> Online` completion edge survives on the
    //     SubmachineRef (per Doc 09 §4.11 the ref carries the state's own
    //     transitions).
    assert!(
        !connecting.transitions.is_empty(),
        "the `done -> Online` completion edge must survive on the SubmachineRef"
    );

    // Sanity: an ordinary sibling state is NOT a submachine ref.
    assert!(
        device
            .root
            .states
            .iter()
            .any(|s| matches!(s, StateNode::Simple(x) if x.name == "Online")),
        "ordinary `state Online` stays a Simple state"
    );
}

// ---------------------------------------------------------------------------
// NEGATIVE — the four submachine diagnostics.
// ---------------------------------------------------------------------------

/// FSM-E0103 ("unknown machine reference"): `state X is Ghost {}` where no
/// `submachine Ghost` (and no machine Ghost) is declared.
#[test]
fn unknown_submachine_reference_emits_e0103() {
    let src = r#"language fsm 2.0

feature submachines

machine M {
    initial X
    state X is Ghost { }
}"#;
    let pr = parse(src);
    let res = analyze(&pr);
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::E0103),
        "unresolved `is Ghost` must emit FSM-E0103; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>()
    );
}

/// FSM-E0610 ("construct used without required `feature` flag"): submachine
/// syntax with no `feature submachines` declared (Doc 04 §2.2 / §15).
#[test]
fn submachine_without_feature_flag_emits_e0610() {
    let src = r#"language fsm 2.0

submachine Connection {
    initial Idle
    state Idle { }
}

machine Parent {
    initial Link
    state Link is Connection { }
}"#;
    let pr = parse(src);
    let res = analyze(&pr);
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::E0610),
        "submachine syntax without `feature submachines` must emit FSM-E0610; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>()
    );
}

/// FSM-E0500 ("submachine entry point not declared"): a submachine with no
/// `initial` and no `entry_point` cannot be entered (Doc 08 §12.2 — entry is
/// either a named entry point or the submachine's initial state; with
/// neither there is no entry point).
#[test]
fn submachine_without_entry_point_emits_e0500() {
    let src = r#"language fsm 2.0

feature submachines

submachine NoEntry {
    state Lonely { }
}

machine Host {
    initial Use
    state Use is NoEntry { }
}"#;
    let pr = parse(src);
    let res = analyze(&pr);
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::E0500),
        "submachine with no initial/entry_point must emit FSM-E0500; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>()
    );
}

/// FSM-E0501 ("submachine exit point not declared"): the referencing state
/// declares a `done ->` completion (it expects the submachine to complete),
/// but the submachine has no `final` state / `exit_point` — there is no exit
/// point to drive that completion (Doc 08 §12.3).
#[test]
fn submachine_done_without_exit_point_emits_e0501() {
    let src = r#"language fsm 2.0

feature submachines

submachine NoExit {
    initial Idle
    state Idle { on START -> Idle }
}

machine Host {
    events { START }
    initial Use
    state Use is NoExit {
        done -> Finished
    }
    state Finished { }
}"#;
    let pr = parse(src);
    let res = analyze(&pr);
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::E0501),
        "`done ->` on a submachine ref whose template has no final/exit_point \
         must emit FSM-E0501; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>()
    );
}

/// FSM-E0502 ("submachine instantiation cycle detected"): submachine A
/// references B and B references A. Recoverable: No (Doc 10 §9). This
/// guards the rewired cycle detector against the real SUBMACHINE_REF CST
/// (the stub scanned `machines()`, which never contains templates).
#[test]
fn submachine_instantiation_cycle_emits_e0502() {
    let src = r#"language fsm 2.0

feature submachines

submachine A {
    initial Sa
    state Sa is B { }
}

submachine B {
    initial Sb
    state Sb is A { }
}

machine Top {
    initial Use
    state Use is A { }
}"#;
    let pr = parse(src);
    let res = analyze(&pr);
    assert!(
        has_code(&res.diagnostics, DiagnosticCode::E0502),
        "mutually-referencing submachines must emit FSM-E0502; got: {:?}",
        res.diagnostics
            .iter()
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>()
    );
}
