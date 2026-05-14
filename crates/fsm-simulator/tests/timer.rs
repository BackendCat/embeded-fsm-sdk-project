//! Timer lifecycle — Doc 08 §13.
//!
//! - Timer is armed on entry to the owning state.
//! - One-shot fires once at `now + duration_ms`.
//! - Periodic re-arms at the scheduled-next moment (no drift).
//! - `advance_clock(delta)` advances virtual clock and processes timers
//!   that would have fired in the interval.

mod common;

use common::*;
use fsm_ir::StateNode;
use fsm_simulator::{InitOptions, Interpreter, StepKind};

fn one_shot_ir() -> fsm_ir::Ir {
    // Idle —after 100 ms→ Done
    let mut idle = simple("s-idle");
    idle.transitions.push(timer_transition(
        "t-idle-done",
        "s-idle",
        "s-done",
        100,
        false,
    ));
    idle.timers
        .push(timer("tm-idle", "s-idle", "s-done", 100, false));
    let done = simple("s-done");
    let root = region(
        "r-root",
        "ps-init",
        vec![
            initial("ps-init", "s-idle"),
            StateNode::Simple(idle),
            StateNode::Simple(done),
        ],
    );
    let m = machine("TimerDemo", root);
    ir_one_machine(m)
}

fn periodic_ir() -> fsm_ir::Ir {
    let mut active = simple("s-active");
    active
        .transitions
        .push(timer_transition("t-tick", "s-active", "s-active", 50, true));
    active
        .timers
        .push(timer("tm-tick", "s-active", "s-active", 50, true));
    let root = region(
        "r-root",
        "ps-init",
        vec![initial("ps-init", "s-active"), StateNode::Simple(active)],
    );
    let m = machine("PeriodicDemo", root);
    ir_one_machine(m)
}

#[test]
fn one_shot_does_not_fire_before_due() {
    let ir = one_shot_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "TimerDemo".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
    interp.advance_clock(50).unwrap();
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
    assert_eq!(interp.virtual_clock_ms(), 50);
}

#[test]
fn one_shot_fires_at_due() {
    let ir = one_shot_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "TimerDemo".into(),
            ..Default::default()
        })
        .unwrap();
    interp.advance_clock(50).unwrap();
    let recs = interp.advance_clock(50).unwrap();
    assert_eq!(interp.current_states(), vec!["s-done".to_string()]);
    assert!(
        recs.iter().any(|r| matches!(r.kind, StepKind::TimerFired)),
        "expected at least one TimerFired record"
    );
}

#[test]
fn periodic_re_arms_without_drift() {
    let ir = periodic_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "PeriodicDemo".into(),
            ..Default::default()
        })
        .unwrap();
    let recs = interp.advance_clock(200).unwrap();
    // 50ms period over 200ms = 4 fires (at 50, 100, 150, 200).
    let fires = recs
        .iter()
        .filter(|r| matches!(r.kind, StepKind::TimerFired))
        .count();
    assert_eq!(fires, 4, "expected 4 fires over 200ms with 50ms period");
    // Self-transition stays on s-active.
    assert_eq!(interp.current_states(), vec!["s-active".to_string()]);
}
