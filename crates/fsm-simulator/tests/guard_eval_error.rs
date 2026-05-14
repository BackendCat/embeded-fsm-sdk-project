//! Audit D P1-B / prior P1-7: a runtime error from guard evaluation must
//! propagate as `StepError::GuardEval(...)` rather than silently disable the
//! transition (`unwrap_or(false)`).
//!
//! Build a transition with a guard that references `payload.x` and dispatch
//! the event with `None` payload — the evaluator raises
//! `EvalError::PayloadUnavailable`. Pre-fix the simulator turned that into
//! `false` and the transition silently never fired; post-fix the error
//! bubbles up to the caller as `StepError::GuardEval`.

mod common;

use common::*;
use fsm_ir::{
    CmpOp, EventObject, FieldRef, GuardExpr, GuardOperand, IntLit, Literal, Param, StateNode,
    TransitionKind, Type,
};
use fsm_simulator::{InitOptions, Interpreter, StepError};

fn payloaded_event(name: &str, ev_id: &str) -> EventObject {
    EventObject {
        id: ev_id.into(),
        stable_id: format!("M:event:{name}"),
        name: name.into(),
        // Declare an `x` field on the event so a guard referencing
        // `payload.x` is grammatically valid — the runtime error then comes
        // from the dispatcher passing `None` instead of a populated map.
        payload: vec![Param {
            name: "x".into(),
            ty: Type::Primitive { name: "i32".into() },
            id: None,
            loc: None,
        }],
        loc: loc(),
    }
}

fn ir_with_payload_guard() -> fsm_ir::Ir {
    let mut idle = simple("s-idle");
    let mut t = transition(
        "t-go",
        "s-idle",
        "s-running",
        "ev-go",
        TransitionKind::External,
    );
    t.guard = Some(GuardExpr::FieldCmp {
        lhs: FieldRef::Payload { field: "x".into() },
        op: CmpOp::Gt,
        rhs: GuardOperand::Literal(Literal::Int(IntLit {
            value: 5,
            loc: None,
        })),
    });
    idle.transitions.push(t);
    let running = simple("s-running");
    let root = region(
        "r-root",
        "ps-init",
        vec![
            initial("ps-init", "s-idle"),
            StateNode::Simple(idle),
            StateNode::Simple(running),
        ],
    );
    let mut m = machine("PayloadGuard", root);
    m.events.push(payloaded_event("GO", "ev-go"));
    ir_one_machine(m)
}

#[test]
fn guard_runtime_error_propagates_as_step_error_guard_eval() {
    let ir = ir_with_payload_guard();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "PayloadGuard".into(),
            ..Default::default()
        })
        .unwrap();
    // Dispatch GO with NO payload — the guard references `payload.x`, so the
    // evaluator returns `EvalError::PayloadUnavailable`. Pre-fix that became
    // a silent `false`; post-fix it must bubble up as `StepError::GuardEval`.
    let result = interp.dispatch("GO");
    match result {
        Err(StepError::GuardEval(_)) => {
            // Expected — the runtime error reaches the caller intact.
        }
        Ok(_) => panic!(
            "expected StepError::GuardEval, got Ok — the guard error was \
             swallowed (unwrap_or(false) regression)"
        ),
        Err(other) => panic!("expected StepError::GuardEval, got {other:?}"),
    }
}
