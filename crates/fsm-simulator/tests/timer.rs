//! Timer lifecycle — Doc 08 §13.
//!
//! - Timer is armed on entry to the owning state.
//! - One-shot fires once at `now + duration_ms`.
//! - Periodic re-arms at the scheduled-next moment (no drift).
//! - `advance_clock(delta)` advances virtual clock and processes timers
//!   that would have fired in the interval.
//! - Timer is disarmed on exit (Doc 08 §13.2) — see
//!   `timer_cancelled_on_state_exit` (P0-4 regression).

mod common;

use common::*;
use fsm_ir::StateNode;
use fsm_simulator::{InitOptions, Interpreter, StepKind};

fn one_shot_ir() -> fsm_ir::Ir {
    // Idle —after 100 ms→ Done. The timer-id `tm-idle` matches both the
    // `TimerObject` and the `Trigger::After { timer_id }` per P0-4 so the
    // interpreter resolves the transition by id, not by (source,target).
    let mut idle = simple("s-idle");
    idle.transitions.push(timer_transition_with_id(
        "t-idle-done",
        "s-idle",
        "s-done",
        100,
        false,
        "tm-idle",
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
    active.transitions.push(timer_transition_with_id(
        "t-tick", "s-active", "s-active", 50, true, "tm-tick",
    ));
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

/// Two-state IR: Idle with `after 100 ms -> Faulted` plus a STOP transition
/// back to Idle (kept for symmetry). Used to verify that the timer fires
/// only when its owning state is the active leaf.
fn one_shot_with_stop_ir() -> fsm_ir::Ir {
    let mut idle = simple("s-idle");
    idle.transitions.push(transition(
        "t-idle-start",
        "s-idle",
        "s-running",
        "ev-start",
        fsm_ir::TransitionKind::External,
    ));
    let mut running = simple("s-running");
    running.transitions.push(timer_transition_with_id(
        "t-running-after",
        "s-running",
        "s-faulted",
        100,
        false,
        "tm-running",
    ));
    running.transitions.push(transition(
        "t-running-stop",
        "s-running",
        "s-idle",
        "ev-stop",
        fsm_ir::TransitionKind::External,
    ));
    running
        .timers
        .push(timer("tm-running", "s-running", "s-faulted", 100, false));
    let faulted = simple("s-faulted");
    let root = region(
        "r-root",
        "ps-init",
        vec![
            initial("ps-init", "s-idle"),
            StateNode::Simple(idle),
            StateNode::Simple(running),
            StateNode::Simple(faulted),
        ],
    );
    let mut m = machine("Watchdog", root);
    m.events.push(event("ev-start", "START"));
    m.events.push(event("ev-stop", "STOP"));
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

#[test]
fn periodic_fires_six_times_over_three_periods() {
    // 50ms period over 300ms: ticks at 50, 100, 150, 200, 250, 300 = 6 fires.
    let ir = periodic_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "PeriodicDemo".into(),
            ..Default::default()
        })
        .unwrap();
    let recs = interp.advance_clock(300).unwrap();
    let fires = recs
        .iter()
        .filter(|r| matches!(r.kind, StepKind::TimerFired))
        .count();
    assert_eq!(fires, 6, "expected 6 fires over 300ms with 50ms period");
}

#[test]
fn timer_cancelled_on_state_exit() {
    // P0-4 regression: enter Running (arms timer), exit via STOP before the
    // 100ms deadline; advance another second and assert no fire occurred.
    let ir = one_shot_with_stop_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Watchdog".into(),
            ..Default::default()
        })
        .unwrap();
    interp.dispatch("START").unwrap();
    assert_eq!(interp.current_states(), vec!["s-running".to_string()]);
    interp.advance_clock(50).unwrap();
    assert_eq!(interp.current_states(), vec!["s-running".to_string()]);
    interp.dispatch("STOP").unwrap();
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
    // Past the timer deadline: must NOT fire because Running was exited.
    let recs_after = interp.advance_clock(1000).unwrap();
    let fires = recs_after
        .iter()
        .filter(|r| matches!(r.kind, StepKind::TimerFired))
        .count();
    assert_eq!(
        fires, 0,
        "no fire expected — owning state Running was exited before deadline"
    );
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
}

#[test]
fn one_shot_armed_after_re_entry_to_owning_state() {
    // Idle -START-> Running (timer armed @100ms). Advance 50ms. -STOP->
    // Idle. -START-> Running again — fresh arm, must fire 100ms after
    // re-entry.
    let ir = one_shot_with_stop_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Watchdog".into(),
            ..Default::default()
        })
        .unwrap();
    interp.dispatch("START").unwrap();
    interp.advance_clock(50).unwrap();
    interp.dispatch("STOP").unwrap();
    interp.dispatch("START").unwrap();
    assert_eq!(interp.current_states(), vec!["s-running".to_string()]);
    interp.advance_clock(99).unwrap();
    assert_eq!(
        interp.current_states(),
        vec!["s-running".to_string()],
        "timer should NOT fire 99ms after re-entry"
    );
    interp.advance_clock(1).unwrap();
    assert_eq!(
        interp.current_states(),
        vec!["s-faulted".to_string()],
        "timer should fire 100ms after re-entry"
    );
}
