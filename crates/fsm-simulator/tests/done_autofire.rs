//! Audit 2026-05-14 — `done` on a non-final state must auto-fire.
//!
//! Pre-fix, the simulator's `check_and_enqueue_completion` enqueued a
//! `Completion` event only for parent composites/parallels whose region
//! had reached a `Final` substate. A simple state with `done -> Target`
//! produced no event; the transition could fire only if the caller
//! manually dispatched `EVENT__COMPLETION`, which is a leaking
//! implementation detail.
//!
//! The fix: when a Simple state with a `TransitionKind::Completion`
//! transition is entered, enqueue `Completion(state_id)` at the front of
//! the queue. The interpreter's existing completion-matching path (which
//! already handles `t.trigger = None` against the just-completed state)
//! then fires the transition on the next RTC step.
//!
//! Important: only **Simple** states auto-fire. Composite/Parallel `done`
//! is gated by their Final-state semantics — auto-firing them would
//! short-circuit B-08.

mod common;

use common::*;
use fsm_ir::{StateNode, TransitionKind};
use fsm_simulator::{InitOptions, Interpreter};

/// Build a flat machine:
/// ```text
/// initial Start
/// state Start { done -> Finished }
/// state Finished {}
/// ```
fn build_simple_done_machine() -> fsm_ir::Ir {
    let mut start = simple("s-start");
    start.transitions.push(completion_transition(
        "t-start-done",
        "s-start",
        "s-finished",
    ));
    let finished = simple("s-finished");
    let root = region(
        "r-root",
        "ps-init",
        vec![
            initial("ps-init", "s-start"),
            StateNode::Simple(start),
            StateNode::Simple(finished),
        ],
    );
    let m = machine("DoneAutoFire", root);
    ir_one_machine(m)
}

#[test]
fn done_on_simple_state_auto_fires_at_init() {
    // After init, the runtime enters `Start`. `Start` has `done -> Finished`,
    // so the auto-completion must fire during init's queue drain and the
    // machine should settle in `Finished` *without* an external dispatch.
    let ir = build_simple_done_machine();
    let mut interp = Interpreter::new(&ir).unwrap();
    let records = interp
        .init(InitOptions {
            machine_name: "DoneAutoFire".into(),
            ..Default::default()
        })
        .expect("init");
    // At least one record beyond the init record: the auto-fired completion.
    assert!(
        records.len() >= 2,
        "expected init + completion records, got {:#?}",
        records
    );
    assert_eq!(
        interp.current_states(),
        vec!["s-finished".to_string()],
        "Simple state with `done -> Finished` must auto-fire and land in Finished"
    );
}

/// Chained `done`: `A -done-> B -done-> C`. All three must collapse into
/// `C` after init with no external events.
#[test]
fn chained_done_transitions_collapse_to_terminal() {
    let mut a = simple("s-a");
    a.transitions
        .push(completion_transition("t-a-done", "s-a", "s-b"));
    let mut b = simple("s-b");
    b.transitions
        .push(completion_transition("t-b-done", "s-b", "s-c"));
    let c = simple("s-c");
    let root = region(
        "r-root",
        "ps-init",
        vec![
            initial("ps-init", "s-a"),
            StateNode::Simple(a),
            StateNode::Simple(b),
            StateNode::Simple(c),
        ],
    );
    let m = machine("ChainedDone", root);
    let ir = ir_one_machine(m);
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "ChainedDone".into(),
            ..Default::default()
        })
        .expect("init");
    assert_eq!(
        interp.current_states(),
        vec!["s-c".to_string()],
        "chained done transitions must collapse to terminal state"
    );
}

/// Parallel state with `done -> Stopped` must NOT auto-fire on entry —
/// completion is gated by B-08 (both regions Final). This is the
/// regression guard for the parallel-completion semantics that the
/// auto-fire fix could trample if it weren't scoped to Simple states.
#[test]
fn done_on_parallel_state_does_not_short_circuit_b08() {
    // Region A: Active -FINISH_A-> SensorsDone (final)
    let mut active = simple("s-active");
    active.transitions.push(transition(
        "t-active-done",
        "s-active",
        "s-sensors-done",
        "ev-finish-a",
        TransitionKind::External,
    ));
    let region_a = region(
        "r-sensors",
        "ps-init-sensors",
        vec![
            initial("ps-init-sensors", "s-active"),
            StateNode::Simple(active),
            final_state("s-sensors-done"),
        ],
    );
    // Region B: On -FINISH_B-> OutputDone (final)
    let mut on = simple("s-on");
    on.transitions.push(transition(
        "t-on-done",
        "s-on",
        "s-output-done",
        "ev-finish-b",
        TransitionKind::External,
    ));
    let region_b = region(
        "r-output",
        "ps-init-output",
        vec![
            initial("ps-init-output", "s-on"),
            StateNode::Simple(on),
            final_state("s-output-done"),
        ],
    );
    let mut monitor = parallel("s-monitor", vec![region_a, region_b]);
    monitor.transitions.push(completion_transition(
        "t-monitor-done",
        "s-monitor",
        "s-stopped",
    ));
    let stopped = simple("s-stopped");
    let root = region(
        "r-root",
        "ps-init-root",
        vec![
            initial("ps-init-root", "s-monitor"),
            StateNode::Parallel(monitor),
            StateNode::Simple(stopped),
        ],
    );
    let mut m = machine("ParallelDoneGuard", root);
    m.events.push(event("ev-finish-a", "FINISH_A"));
    m.events.push(event("ev-finish-b", "FINISH_B"));
    let ir = ir_one_machine(m);
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "ParallelDoneGuard".into(),
            ..Default::default()
        })
        .unwrap();
    let mut current = interp.current_states();
    current.sort();
    assert_eq!(
        current,
        vec!["s-active".to_string(), "s-on".to_string()],
        "parallel `done` must NOT auto-fire on entry — both regions still non-final"
    );
}
