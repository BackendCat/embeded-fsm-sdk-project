//! v1.1 deferred-event runtime — simulator behavior (Doc 08 §10).
//!
//! Printer machine:
//! - Initial: Idle
//! - Idle      —PRINT_JOB→   Printing
//! - Idle      —ENTER_MAINT→ Maintenance
//! - Printing  —JOB_DONE→    Idle
//! - Maintenance { defer PRINT_JOB }  —MAINT_DONE→ Idle
//!
//! While in Maintenance, a `PRINT_JOB` is HELD (no transition consumes it
//! there, Maintenance declares `defer PRINT_JOB`). On `MAINT_DONE` the
//! machine exits Maintenance → Idle; the deferred `PRINT_JOB` is released
//! to the FRONT of the queue and reprocessed in Idle, where it IS
//! consumed (Idle —PRINT_JOB→ Printing).
//!
//! Regression contract (SUBAGENT_CONVENTIONS §5.1): on `main` the
//! simulator deferred the event *before* the queue and returned an empty
//! record vec (the deferral was invisible and the event was effectively
//! dropped — `current_states()` never reached Printing after release).
//! These assertions FAIL on `main` and PASS after the v1.1 wave.

mod common;

use common::*;
use fsm_ir::{DeferDecl, SourceLocation, Span, StateNode, TransitionKind};
use fsm_simulator::{InitOptions, Interpreter, StepKind};

fn defer_decl(event_id: &str) -> DeferDecl {
    DeferDecl {
        event_id: event_id.to_string(),
        loc: SourceLocation::new("printer.fsm", Span::new(0, 1), 1, 1),
    }
}

fn printer_ir() -> fsm_ir::Ir {
    let mut idle = simple("s-idle");
    idle.transitions.push(transition(
        "t-idle-print",
        "s-idle",
        "s-printing",
        "ev-print-job",
        TransitionKind::External,
    ));
    idle.transitions.push(transition(
        "t-idle-maint",
        "s-idle",
        "s-maintenance",
        "ev-enter-maint",
        TransitionKind::External,
    ));

    let mut printing = simple("s-printing");
    printing.transitions.push(transition(
        "t-printing-done",
        "s-printing",
        "s-idle",
        "ev-job-done",
        TransitionKind::External,
    ));

    let mut maintenance = simple("s-maintenance");
    // The deferring state: holds PRINT_JOB while active.
    maintenance.defers.push(defer_decl("ev-print-job"));
    maintenance.transitions.push(transition(
        "t-maint-done",
        "s-maintenance",
        "s-idle",
        "ev-maint-done",
        TransitionKind::External,
    ));

    let root = region(
        "r-root",
        "ps-initial-0",
        vec![
            initial("ps-initial-0", "s-idle"),
            StateNode::Simple(idle),
            StateNode::Simple(printing),
            StateNode::Simple(maintenance),
        ],
    );
    let mut m = machine("Printer", root);
    m.events.push(event("ev-print-job", "PRINT_JOB"));
    m.events.push(event("ev-enter-maint", "ENTER_MAINT"));
    m.events.push(event("ev-job-done", "JOB_DONE"));
    m.events.push(event("ev-maint-done", "MAINT_DONE"));
    ir_one_machine(m)
}

fn boot() -> Interpreter {
    let ir = printer_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Printer".into(),
            ..Default::default()
        })
        .unwrap();
    interp
}

#[test]
fn print_job_is_held_while_maintenance_active_then_replayed_on_exit() {
    let mut interp = boot();
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);

    // Enter Maintenance.
    let recs = interp.dispatch("ENTER_MAINT").unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(interp.current_states(), vec!["s-maintenance".to_string()]);

    // Dispatch PRINT_JOB while in Maintenance. No transition consumes it
    // there; Maintenance defers it → it is HELD, not discarded. Exactly
    // one step record, kind = EventDeferred, configuration unchanged.
    let recs = interp.dispatch("PRINT_JOB").unwrap();
    assert_eq!(
        recs.len(),
        1,
        "the deferral must be a visible single step (not an empty vec as on main)"
    );
    let d = &recs[0];
    assert_eq!(d.kind, StepKind::EventDeferred);
    assert!(d.transition_taken.is_none());
    assert_eq!(d.config_before, d.config_after, "defer changes no config");
    assert_eq!(d.config_after, vec!["s-maintenance".to_string()]);
    assert_eq!(
        d.event_received.as_ref().map(|e| e.name.as_str()),
        Some("PRINT_JOB")
    );
    // Still in Maintenance — the held event did not move us.
    assert_eq!(interp.current_states(), vec!["s-maintenance".to_string()]);

    // MAINT_DONE: exits Maintenance → Idle. On exit from the (last)
    // deferring state, PRINT_JOB is released to the FRONT of the queue and
    // reprocessed in Idle, where Idle —PRINT_JOB→ Printing consumes it.
    // So this single dispatch yields TWO records:
    //   1. MAINT_DONE: Maintenance → Idle
    //   2. PRINT_JOB (redispatched): Idle → Printing
    let recs = interp.dispatch("MAINT_DONE").unwrap();
    assert_eq!(
        recs.len(),
        2,
        "MAINT_DONE plus the replayed PRINT_JOB = 2 steps; got: {:?}",
        recs.iter()
            .map(|r| (r.kind, r.config_after.clone()))
            .collect::<Vec<_>>()
    );

    let maint = &recs[0];
    assert_eq!(maint.kind, StepKind::Dispatched);
    assert_eq!(
        maint.event_received.as_ref().map(|e| e.name.as_str()),
        Some("MAINT_DONE")
    );
    assert_eq!(maint.exited_states, vec!["s-maintenance".to_string()]);
    assert_eq!(maint.entered_states, vec!["s-idle".to_string()]);

    let replay = &recs[1];
    assert_eq!(
        replay.kind,
        StepKind::EventRedispatched,
        "the released deferred event must record as EventRedispatched"
    );
    assert_eq!(
        replay.event_received.as_ref().map(|e| e.name.as_str()),
        Some("PRINT_JOB")
    );
    assert_eq!(replay.exited_states, vec!["s-idle".to_string()]);
    assert_eq!(replay.entered_states, vec!["s-printing".to_string()]);
    let tt = replay
        .transition_taken
        .as_ref()
        .expect("replayed PRINT_JOB consumed by Idle -> Printing");
    assert_eq!(tt.stable_id, "t-idle-print");

    // Final configuration: the replayed job drove us into Printing.
    assert_eq!(interp.current_states(), vec!["s-printing".to_string()]);
    // Defer set is now empty (the held event was released and consumed).
    let snap = interp.snapshot().unwrap();
    assert!(
        snap.defer_set.is_empty(),
        "defer set should be drained after release; got {:?}",
        snap.defer_set
    );
}

#[test]
fn transition_wins_over_defer_when_same_state_has_both_paths() {
    // UML 2.5.1 §14.2.3.9.1 / Doc 08 §10.1: an enabled transition takes
    // precedence over deferral. We model a state that BOTH defers an event
    // AND (via an ancestor / sibling reachable transition) could consume
    // it — here Idle consuming PRINT_JOB directly. While Idle is active,
    // PRINT_JOB must transition (NOT defer), because selection succeeds
    // before the deferral check is even reached.
    let mut interp = boot();
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);

    let recs = interp.dispatch("PRINT_JOB").unwrap();
    assert_eq!(recs.len(), 1);
    let r = &recs[0];
    assert_eq!(
        r.kind,
        StepKind::Dispatched,
        "Idle consumes PRINT_JOB directly — must NOT be deferred"
    );
    assert_eq!(r.entered_states, vec!["s-printing".to_string()]);
    assert_eq!(interp.current_states(), vec!["s-printing".to_string()]);
}

#[test]
fn unconsumed_non_deferred_event_is_still_discarded() {
    // Guardrail: an event that is neither consumed NOR deferred by the
    // active configuration is discarded exactly as before (no behavior
    // change for non-defer machines / events).
    let mut interp = boot();
    // JOB_DONE is declared but Idle neither consumes nor defers it.
    let recs = interp.dispatch("JOB_DONE").unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].kind, StepKind::Dispatched);
    assert!(recs[0].transition_taken.is_none());
    assert_eq!(recs[0].config_before, recs[0].config_after);
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
}
