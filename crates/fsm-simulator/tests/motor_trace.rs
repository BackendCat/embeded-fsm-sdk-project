//! Motor — positive flow. Mirrors the canonical "Motor" example from Doc 08
//! §15 (TrafficLight pattern, simpler shape).
//!
//! Machine:
//! - Initial: Idle
//! - Idle —START→ Running
//! - Running —STOP→ Idle

mod common;

use common::*;
use fsm_ir::{StateNode, TransitionKind};
use fsm_simulator::{InitOptions, Interpreter, StepKind};

fn motor_ir() -> fsm_ir::Ir {
    let mut idle = simple("s-idle");
    idle.transitions.push(transition(
        "t-idle-running",
        "s-idle",
        "s-running",
        "ev-start",
        TransitionKind::External,
    ));
    let mut running = simple("s-running");
    running.transitions.push(transition(
        "t-running-idle",
        "s-running",
        "s-idle",
        "ev-stop",
        TransitionKind::External,
    ));
    let root = region(
        "r-root",
        "ps-initial-0",
        vec![
            initial("ps-initial-0", "s-idle"),
            StateNode::Simple(idle),
            StateNode::Simple(running),
        ],
    );
    let mut m = machine("Motor", root);
    m.events.push(event("ev-start", "START"));
    m.events.push(event("ev-stop", "STOP"));
    ir_one_machine(m)
}

#[test]
fn init_enters_idle() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    let records = interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(records.len(), 1);
    let r = &records[0];
    assert_eq!(r.kind, StepKind::Init);
    assert_eq!(r.entered_states, vec!["s-idle".to_string()]);
    assert_eq!(r.config_after, vec!["s-idle".to_string()]);
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
}

#[test]
fn dispatch_start_transitions_to_running() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .unwrap();
    let records = interp.dispatch("START").unwrap();
    assert_eq!(records.len(), 1, "single RTC step");
    let r = &records[0];
    assert_eq!(r.kind, StepKind::Dispatched);
    assert_eq!(r.exited_states, vec!["s-idle".to_string()]);
    assert_eq!(r.entered_states, vec!["s-running".to_string()]);
    let tt = r.transition_taken.as_ref().expect("transition recorded");
    assert_eq!(tt.stable_id, "t-idle-running");
    assert_eq!(interp.current_states(), vec!["s-running".to_string()]);
}

#[test]
fn dispatch_stop_returns_to_idle() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .unwrap();
    interp.dispatch("START").unwrap();
    let records = interp.dispatch("STOP").unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].entered_states, vec!["s-idle".to_string()]);
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
}

#[test]
fn dispatch_unknown_event_is_invalid() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .unwrap();
    let err = interp.dispatch("DOES_NOT_EXIST").unwrap_err();
    assert!(matches!(
        err,
        fsm_simulator::StepError::InvalidEvent { name } if name == "DOES_NOT_EXIST"
    ));
}

#[test]
fn unhandled_event_in_state_is_discarded() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .unwrap();
    // STOP is declared but not handled in Idle.
    let records = interp.dispatch("STOP").unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, StepKind::Dispatched);
    assert!(records[0].transition_taken.is_none());
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
}

#[test]
fn current_states_named_uses_dot_paths() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .unwrap();
    let named = interp.current_states_named();
    assert_eq!(named, vec!["Motor.s-idle".to_string()]);
}

#[test]
fn snapshot_restore_round_trips() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .unwrap();
    let snap = interp.snapshot();
    interp.dispatch("START").unwrap();
    assert_eq!(interp.current_states(), vec!["s-running".to_string()]);
    interp.restore(snap);
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
}
