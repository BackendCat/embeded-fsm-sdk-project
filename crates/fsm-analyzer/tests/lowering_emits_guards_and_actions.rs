//! Regression for P0-1 (AUDIT_2026_05_14 §P0-1).
//!
//! Pre-fix, every transition surfaced `guard: None`, `actions: []` and every
//! `Trigger::Event` carried `payload_binding: None` because `lower_external`
//! / `lower_internal` / `lower_local` / `lower_completion` (and the four
//! timer helpers) hard-coded those fields. Entry / exit action blocks were
//! collapsed to `Vec::new()` at `lower_state`. The IR was structurally
//! complete and behaviourally empty.
//!
//! This test parses the rich Motor fixture, walks the resulting IR, and
//! asserts every DSL-authored guard / action / payload reference reached
//! the lowerer's output untouched.
//!
//! Companion to `crates/fsm-cli/tests/motor_emits_guards_and_actions.rs`
//! (which pins the codegen surface) and `golden_simulator_runs_motor`
//! (which pins the runtime semantics).

use std::fs;

use fsm_analyzer::analyze_with_source;
use fsm_ir::{Expr, GuardExpr, Literal, MachineObject, StateNode, Statement, TransitionObject};
use fsm_parser::parse;

const RICH_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/motor/motor.fsm"
);

fn motor_ir() -> MachineObject {
    let src = fs::read_to_string(RICH_FIXTURE).expect("read examples/motor/motor.fsm");
    let pr = parse(&src);
    let res = analyze_with_source(&pr, RICH_FIXTURE, &src);
    let errors: Vec<_> = res
        .diagnostics
        .iter()
        .filter(|d| d.severity == fsm_diagnostics::Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "analyzer reported errors on examples/motor/motor.fsm: {:?}",
        errors
    );
    res.ir.expect("ir produced").machines.remove(0)
}

fn collect_transitions(machine: &MachineObject) -> Vec<TransitionObject> {
    let mut out = Vec::new();
    fn walk(states: &[StateNode], out: &mut Vec<TransitionObject>) {
        for s in states {
            match s {
                StateNode::Simple(s) => out.extend(s.transitions.iter().cloned()),
                StateNode::Composite(c) => {
                    out.extend(c.transitions.iter().cloned());
                    for r in &c.regions {
                        walk(&r.states, out);
                    }
                }
                StateNode::Parallel(p) => {
                    out.extend(p.transitions.iter().cloned());
                    for r in &p.regions {
                        walk(&r.states, out);
                    }
                }
                _ => {}
            }
        }
    }
    walk(&machine.root.states, &mut out);
    out
}

#[test]
fn start_transition_carries_can_start_guard() {
    let machine = motor_ir();
    let transitions = collect_transitions(&machine);
    // The `START` transition out of `Idle` is identified by trigger event id.
    let start = transitions
        .iter()
        .find(|t| {
            matches!(
                &t.trigger,
                Some(fsm_ir::Trigger::Event { event_id, .. }) if event_id == "ev-Motor-START"
            ) && t.source == "s-Motor-Idle"
        })
        .expect("START transition out of Idle must be lowered");
    let guard = start
        .guard
        .as_ref()
        .expect("P0-1 fix: `on START [can_start]` must lower to a real GuardExpr, not None");
    match guard {
        GuardExpr::ExternCall { callee, args } => {
            assert_eq!(
                callee, "can_start",
                "guard extern call must preserve the DSL name"
            );
            assert!(args.is_empty(), "can_start has no args in DSL");
        }
        other => panic!(
            "expected GuardExpr::ExternCall {{ callee: 'can_start' }}, got: {:?}",
            other
        ),
    }
}

#[test]
fn start_transition_has_count_assign_and_set_speed_call_actions() {
    let machine = motor_ir();
    let transitions = collect_transitions(&machine);
    let start = transitions
        .iter()
        .find(|t| t.source == "s-Motor-Idle")
        .expect("Idle has a transition");
    assert!(
        !start.actions.is_empty(),
        "P0-1 fix: action block `ctx.count = ctx.count + 1; set_speed(100)` must lower to Statements"
    );
    // First statement: assignment to ctx.count whose RHS is a binary +.
    let assign = start
        .actions
        .iter()
        .find_map(|s| match s {
            Statement::Assign { target, value } => Some((target, value)),
            _ => None,
        })
        .expect("first action statement must be an assignment");
    match assign.0 {
        fsm_ir::FieldRef::Ctx { field } => assert_eq!(field, "count"),
        other => panic!(
            "expected FieldRef::Ctx {{ field: 'count' }}, got: {:?}",
            other
        ),
    }
    match assign.1 {
        Expr::Binary { op, .. } => {
            assert!(
                matches!(op, fsm_ir::BinaryOp::Add),
                "expected `count + 1` (Add op), got: {:?}",
                op
            );
        }
        other => panic!("expected Binary `ctx.count + 1`, got: {:?}", other),
    }
    // Second statement: bare-name extern call `set_speed(100)`.
    let call = start
        .actions
        .iter()
        .find_map(|s| match s {
            Statement::Call { callee, args } if callee == "set_speed" => Some(args),
            _ => None,
        })
        .expect("set_speed(100) must lower to Statement::Call");
    assert_eq!(call.len(), 1, "set_speed takes one arg in DSL");
    match &call[0] {
        Expr::Literal(Literal::Int(i)) => assert_eq!(i.value, 100),
        other => panic!("expected literal 100 as set_speed arg, got: {:?}", other),
    }
}

#[test]
fn fault_transition_assigns_payload_into_context_and_calls_reset_link() {
    let machine = motor_ir();
    let transitions = collect_transitions(&machine);
    let fault = transitions
        .iter()
        .find(|t| {
            matches!(
                &t.trigger,
                Some(fsm_ir::Trigger::Event { event_id, .. }) if event_id == "ev-Motor-FAULT"
            )
        })
        .expect("FAULT transition must be lowered");
    let assign = fault
        .actions
        .iter()
        .find_map(|s| match s {
            Statement::Assign { target, value } => Some((target, value)),
            _ => None,
        })
        .expect("FAULT action must contain an assignment");
    match assign.0 {
        fsm_ir::FieldRef::Ctx { field } => assert_eq!(field, "last_fault_code"),
        other => panic!("expected ctx.last_fault_code target, got: {:?}", other),
    }
    match assign.1 {
        Expr::FieldRef {
            field_ref: fsm_ir::FieldRef::Payload { field },
        } => assert_eq!(field, "code", "RHS must be payload.code"),
        other => panic!("expected `payload.code` on the RHS, got: {:?}", other),
    }
    let has_reset_link = fault.actions.iter().any(|s| {
        matches!(
            s,
            Statement::Call { callee, args } if callee == "reset_link" && args.is_empty()
        )
    });
    assert!(
        has_reset_link,
        "FAULT action must lower `reset_link()` to a zero-arg Statement::Call: {:?}",
        fault.actions
    );
}

#[test]
fn unguarded_transitions_keep_none_guard() {
    let machine = motor_ir();
    let transitions = collect_transitions(&machine);
    // The `on STOP -> Idle` transition in Running has no guard — the lowerer
    // must NOT synthesize one.
    let stop = transitions
        .iter()
        .find(|t| {
            t.source == "s-Motor-Running"
                && matches!(
                    &t.trigger,
                    Some(fsm_ir::Trigger::Event { event_id, .. }) if event_id == "ev-Motor-STOP"
                )
        })
        .expect("STOP transition exists");
    assert!(
        stop.guard.is_none(),
        "transitions without `[guard]` must have guard: None, got: {:?}",
        stop.guard
    );
}
