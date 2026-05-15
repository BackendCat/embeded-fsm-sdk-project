//! Submachine runtime semantics — v1.1-W2c behavioural acceptance
//! (epic wave 3/4), Doc 08 §12.
//!
//! W2b made `submachine Connection { … }` / `state Connecting is Connection
//! { done -> Online }` *lower* to correct IR (`MachineObject.submachines`
//! populated + a `StateNode::Submachine` ref-state). Pre-W2c the simulator
//! treated `StateNode::Submachine` as a leaf no-op (the defensive arm), so
//! EVERY assertion below fails on `main`:
//!  - the sub-instance never advances (no nested `RuntimeState`), so
//!    `sub_active_leaf` is unobservable / empty;
//!  - the parent never receives the sub's completion, so `Connecting`
//!    never transitions to `Online` without an external event;
//!  - there is no sub-instance to tear down or to re-init fresh.
//!
//! Per FSM-PROC-SUBAGENT §5.4: this drives the Device+Connection FSM
//! through the in-process interpreter and ASSERTS observable state +
//! completion against Doc 08 §12 semantics (the gcc-RUN sim≡codegen
//! cross-check is W2d, which reuses `examples/submachine/`).

mod common;

use common::*;
use fsm_analyzer::analyze;
use fsm_ir::{Ir, StateNode, TransitionKind};
use fsm_parser::parse;
use fsm_simulator::{InitOptions, Interpreter, StepKind};

/// The canonical Doc 04 §15 shape, lowered through the real W2b path so
/// the simulator consumes exactly the IR the toolchain emits.
fn device_connection_ir() -> Ir {
    let src = r#"language fsm 2.0

feature submachines

submachine Connection {
    events { CONNECT ACK DONE }
    initial Idle
    state Idle { on CONNECT -> Handshake }
    state Handshake { on ACK -> Established }
    state Established { on DONE -> Done }
    final Done
}

machine Device {
    events { CONNECT ACK DONE RESET }

    initial Connecting

    state Connecting is Connection {
        done -> Online
        on RESET -> Connecting
    }

    state Online {
        on RESET -> Connecting
    }
}"#;
    let pr = parse(src);
    let res = analyze(&pr);
    assert!(
        !res.diagnostics
            .iter()
            .any(|d| d.severity == fsm_diagnostics::Severity::Error),
        "Device+Connection must analyze clean; got: {:?}",
        res.diagnostics
    );
    res.ir.expect("analyzer produced IR")
}

fn sub_active_leaf(interp: &Interpreter, ref_state: &str) -> Option<String> {
    interp
        .submachine_active_states(ref_state)
        .and_then(|v| v.first().cloned())
}

// ---------------------------------------------------------------------------
// (i) While the ref-state is active, the sub-instance's active leaf
//     advances Idle -> Handshake -> Established -> Final on delegated
//     events (Doc 08 §12.1/§12.2 — parent-unconsumed event delegates).
// ---------------------------------------------------------------------------

#[test]
fn submachine_subinstance_advances_through_delegated_events() {
    let ir = device_connection_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Device".into(),
            ..Default::default()
        })
        .expect("init");

    // Device's root-initial lands on `Connecting` (a submachine ref). The
    // sub-instance must exist and sit at the template's initial leaf.
    assert_eq!(
        interp.current_states(),
        vec!["s-Device-Connecting".to_string()],
        "parent settles on the submachine-ref state"
    );
    assert_eq!(
        sub_active_leaf(&interp, "s-Device-Connecting").as_deref(),
        Some("s-Connection-Idle"),
        "sub-instance inits at the Connection template's initial (Idle)"
    );

    // CONNECT is not consumed by the parent (Connecting only handles RESET
    // + the `done` completion) → delegated to the sub: Idle -> Handshake.
    interp.dispatch("CONNECT").expect("dispatch CONNECT");
    assert_eq!(
        sub_active_leaf(&interp, "s-Device-Connecting").as_deref(),
        Some("s-Connection-Handshake"),
        "delegated CONNECT advanced the sub-instance Idle -> Handshake"
    );
    assert_eq!(
        interp.current_states(),
        vec!["s-Device-Connecting".to_string()],
        "parent stays on Connecting while the sub progresses"
    );

    // ACK → Handshake -> Established (still delegated, parent unchanged).
    interp.dispatch("ACK").expect("dispatch ACK");
    assert_eq!(
        sub_active_leaf(&interp, "s-Device-Connecting").as_deref(),
        Some("s-Connection-Established"),
        "delegated ACK advanced the sub-instance Handshake -> Established"
    );
}

// ---------------------------------------------------------------------------
// (ii) On sub-Final the parent transitions Connecting -> Online WITHOUT an
//      external event (Doc 08 §12.3 — sub completion drives the parent
//      `done ->` through the existing R1 completion path).
// ---------------------------------------------------------------------------

#[test]
fn submachine_final_drives_parent_done_without_external_event() {
    let ir = device_connection_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Device".into(),
            ..Default::default()
        })
        .expect("init");

    interp.dispatch("CONNECT").expect("CONNECT");
    interp.dispatch("ACK").expect("ACK");
    // DONE drives the sub Established -> Done (Final). The sub reaching
    // Final must, with NO further external event, fire the parent's
    // `Connecting --done--> Online`.
    let records = interp.dispatch("DONE").expect("DONE");

    assert_eq!(
        interp.current_states(),
        vec!["s-Device-Online".to_string()],
        "sub reaching Final auto-fired Connecting -> Online (no external event)"
    );
    // The sub-instance was torn down on exit from Connecting.
    assert!(
        interp
            .submachine_active_states("s-Device-Connecting")
            .is_none(),
        "exiting the ref-state must drop the sub-instance (no leak)"
    );

    // Trace fidelity: the DONE dispatch produced a delegation, a
    // sub-completion marker, and the parent completion transition.
    let kinds: Vec<StepKind> = records.iter().map(|r| r.kind).collect();
    assert!(
        kinds.contains(&StepKind::SubmachineEventDelegated),
        "DONE was delegated into the sub-instance; kinds={kinds:?}"
    );
    assert!(
        kinds.contains(&StepKind::SubmachineCompleted),
        "the sub reaching Final emitted a SubmachineCompleted record; kinds={kinds:?}"
    );
    let parent_done = records.iter().find(|r| {
        r.transition_taken
            .as_ref()
            .is_some_and(|t| t.source == "s-Device-Connecting" && t.target == "s-Device-Online")
    });
    assert!(
        parent_done.is_some(),
        "the parent `Connecting --done--> Online` transition fired; records={records:#?}"
    );
}

// ---------------------------------------------------------------------------
// (iii) Exit teardown leaves no sub-state — covered above on the `done`
//       path; here the parent leaves Connecting via its OWN transition
//       (`on RESET`), which must also tear the sub down.
// ---------------------------------------------------------------------------

#[test]
fn parent_initiated_exit_tears_down_subinstance() {
    let ir = device_connection_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Device".into(),
            ..Default::default()
        })
        .expect("init");

    interp.dispatch("CONNECT").expect("CONNECT");
    assert_eq!(
        sub_active_leaf(&interp, "s-Device-Connecting").as_deref(),
        Some("s-Connection-Handshake"),
        "sub mid-progress before the parent-initiated exit"
    );

    // RESET is a parent-level transition on Connecting (self-transition).
    // It WINS over delegating RESET to the sub (transition-wins); exiting
    // Connecting tears the sub down, re-entering re-inits it fresh.
    interp.dispatch("RESET").expect("RESET");
    assert_eq!(
        interp.current_states(),
        vec!["s-Device-Connecting".to_string()],
        "RESET self-transition keeps the parent on Connecting"
    );
    assert_eq!(
        sub_active_leaf(&interp, "s-Device-Connecting").as_deref(),
        Some("s-Connection-Idle"),
        "the sub-instance was torn down and re-inited fresh at Idle (no stale Handshake)"
    );
}

// ---------------------------------------------------------------------------
// (iv) A parent-level `on EVT` on the ref-state WINS over delegating EVT
//      to the sub (transition-wins, Doc 08 §12.1 / UML §14.2.3.9.1).
// ---------------------------------------------------------------------------

#[test]
fn parent_level_transition_wins_over_submachine_delegation() {
    // Build a template whose `Idle` *also* consumes RESET (so if the
    // parent did NOT win, RESET would advance the sub instead of resetting
    // the parent). The parent's `on RESET -> Connecting` must take it.
    let src = r#"language fsm 2.0

feature submachines

submachine Connection {
    events { RESET DONE }
    initial Idle
    state Idle { on RESET -> Established }
    state Established { on DONE -> Done }
    final Done
}

machine Device {
    events { RESET DONE }

    initial Connecting

    state Connecting is Connection {
        done -> Online
        on RESET -> Online
    }

    state Online { }
}"#;
    let pr = parse(src);
    let res = analyze(&pr);
    assert!(
        !res.diagnostics
            .iter()
            .any(|d| d.severity == fsm_diagnostics::Severity::Error),
        "transition-wins fixture must analyze clean; got: {:?}",
        res.diagnostics
    );
    let ir = res.ir.expect("ir");

    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Device".into(),
            ..Default::default()
        })
        .expect("init");
    assert_eq!(
        sub_active_leaf(&interp, "s-Device-Connecting").as_deref(),
        Some("s-Connection-Idle")
    );

    // RESET: the sub's Idle COULD consume it (Idle -> Established), but the
    // parent's `on RESET -> Online` on the ref-state must win.
    interp.dispatch("RESET").expect("RESET");
    assert_eq!(
        interp.current_states(),
        vec!["s-Device-Online".to_string()],
        "parent-level `on RESET` won over delegating RESET to the sub-instance"
    );
    assert!(
        interp
            .submachine_active_states("s-Device-Connecting")
            .is_none(),
        "and the sub-instance was torn down on the parent-initiated exit"
    );
}

// ---------------------------------------------------------------------------
// Negative / edge: re-entering the ref-state re-inits a FRESH sub-instance
// (no stale sub-state carried across an exit/enter cycle).
// ---------------------------------------------------------------------------

#[test]
fn reentering_refstate_reinits_a_fresh_subinstance() {
    let ir = device_connection_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Device".into(),
            ..Default::default()
        })
        .expect("init");

    // Drive the sub all the way to Final → parent goes Connecting -> Online.
    interp.dispatch("CONNECT").unwrap();
    interp.dispatch("ACK").unwrap();
    interp.dispatch("DONE").unwrap();
    assert_eq!(interp.current_states(), vec!["s-Device-Online".to_string()]);

    // RESET on Online → back to Connecting. The sub-instance must be a
    // brand-new one at Idle, NOT the spent (Final) one from before.
    interp.dispatch("RESET").expect("RESET back to Connecting");
    assert_eq!(
        interp.current_states(),
        vec!["s-Device-Connecting".to_string()]
    );
    assert_eq!(
        sub_active_leaf(&interp, "s-Device-Connecting").as_deref(),
        Some("s-Connection-Idle"),
        "re-entered Connecting holds a FRESH sub-instance at Idle (not stale Final)"
    );

    // And it can run the protocol again from scratch.
    interp.dispatch("CONNECT").unwrap();
    assert_eq!(
        sub_active_leaf(&interp, "s-Device-Connecting").as_deref(),
        Some("s-Connection-Handshake"),
        "the fresh sub-instance progresses normally on re-run"
    );
}

// ---------------------------------------------------------------------------
// Builder-path variant (no parser/analyzer dependency) — proves the
// runtime consumes a hand-built SubmachineRef + submachines[] exactly the
// same way, and that a parent-unconsumed event with NO matching sub event
// falls through to a normal discard (non-submachine models unaffected).
// ---------------------------------------------------------------------------

#[test]
fn builder_ir_submachine_runs_and_unknown_event_discards() {
    use fsm_ir::{MachineObject, SubmachineRef};

    // Connection template: Idle --GO--> Done(final).
    let mut idle = simple("s-conn-Idle");
    idle.transitions.push(transition(
        "t-conn-0",
        "s-conn-Idle",
        "s-conn-Done",
        "ev-conn-GO",
        TransitionKind::External,
    ));
    let conn_root = region(
        "r-conn-root",
        "ps-conn-init",
        vec![
            initial("ps-conn-init", "s-conn-Idle"),
            StateNode::Simple(idle),
            final_state("s-conn-Done"),
        ],
    );
    let mut conn = machine("Connection", conn_root);
    conn.id = "m-Connection".into();
    conn.events.push(event("ev-conn-GO", "GO"));

    // Parent: initial -> Link (ref to Connection) with `done -> Up`.
    let link = StateNode::Submachine(SubmachineRef {
        id: "s-host-Link".into(),
        stable_id: "M:Host:state:Link".into(),
        name: "Link".into(),
        submachine_id: "m-Connection".into(),
        entry_points: vec![],
        exit_points: vec![],
        transitions: vec![completion_transition(
            "t-host-0",
            "s-host-Link",
            "s-host-Up",
        )],
        loc: loc(),
    });
    let host_root = region(
        "r-host-root",
        "ps-host-init",
        vec![
            initial("ps-host-init", "s-host-Link"),
            link,
            StateNode::Simple(simple("s-host-Up")),
        ],
    );
    let mut host: MachineObject = machine("Host", host_root);
    host.id = "m-Host".into();
    host.events.push(event("ev-host-GO", "GO"));
    host.events.push(event("ev-host-NOPE", "NOPE"));
    host.submachines.push(conn);
    let ir = ir_one_machine(host);

    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Host".into(),
            ..Default::default()
        })
        .expect("init");
    assert_eq!(
        sub_active_leaf(&interp, "s-host-Link").as_deref(),
        Some("s-conn-Idle"),
        "builder-path sub-instance inits at the template initial"
    );

    // NOPE is declared on the parent but NOT on the sub template → no
    // parent transition, no delegation target → normal discard. The
    // simulator's established convention for an unconsumed dispatched
    // event is `kind=Dispatched, transitionTaken=None` (it has never
    // emitted the `Discarded` kind — see `kind_for_event`), so we assert
    // exactly that, plus that the sub-instance is untouched (the
    // non-submachine fall-through path is unaffected).
    let recs = interp.dispatch("NOPE").expect("NOPE");
    assert!(
        recs.iter().any(|r| r.kind == StepKind::Dispatched
            && r.transition_taken.is_none()
            && r.submachine.is_none()),
        "an event with no parent transition and no matching sub event is a \
         normal unconsumed dispatch (no delegation); records={recs:#?}"
    );
    assert_eq!(
        sub_active_leaf(&interp, "s-host-Link").as_deref(),
        Some("s-conn-Idle"),
        "a non-delegatable unconsumed event does not perturb the sub-instance"
    );

    // GO resolves in the sub template → delegated; sub hits Done(final) →
    // parent `done -> Up` fires with no external event.
    interp.dispatch("GO").expect("GO");
    assert_eq!(
        interp.current_states(),
        vec!["s-host-Up".to_string()],
        "delegated GO completed the sub; parent `done -> Up` fired"
    );
}
