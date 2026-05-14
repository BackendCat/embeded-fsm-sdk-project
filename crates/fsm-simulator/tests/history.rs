//! Shallow-history restoration — Doc 08 §8.1.
//!
//! Machine:
//! ```text
//! state Operating (composite) {
//!   initial Idle
//!   shallow history -> H (default Idle)
//!   state Idle { on START -> Working }
//!   state Working { on STOP -> Idle }
//!   state Finishing
//! }
//! state Suspended
//! Operating —SUSPEND→ Suspended
//! Suspended —RESUME→ Operating.H
//! ```

mod common;

use common::*;
use fsm_ir::{HistoryKind, StateNode, TransitionKind};
use fsm_simulator::{InitOptions, Interpreter};

fn build_ir() -> fsm_ir::Ir {
    let mut idle = simple("s-idle");
    idle.transitions.push(transition(
        "t-idle-working",
        "s-idle",
        "s-working",
        "ev-start",
        TransitionKind::External,
    ));
    let mut working = simple("s-working");
    working.transitions.push(transition(
        "t-working-idle",
        "s-working",
        "s-idle",
        "ev-stop",
        TransitionKind::External,
    ));
    let finishing = simple("s-finishing");

    let operating_region = region(
        "r-operating",
        "ps-init-operating",
        vec![
            initial("ps-init-operating", "s-idle"),
            StateNode::Simple(idle),
            StateNode::Simple(working),
            StateNode::Simple(finishing),
            StateNode::History(history("ps-history-0", HistoryKind::Shallow, "s-idle")),
        ],
    );

    let mut operating = composite("s-operating", vec![operating_region]);
    operating.history = Some(history("ps-history-0", HistoryKind::Shallow, "s-idle"));
    operating.transitions.push(transition(
        "t-operating-suspended",
        "s-operating",
        "s-suspended",
        "ev-suspend",
        TransitionKind::External,
    ));

    let mut suspended = simple("s-suspended");
    suspended.transitions.push(transition(
        "t-suspended-history",
        "s-suspended",
        "ps-history-0",
        "ev-resume",
        TransitionKind::External,
    ));

    let root = region(
        "r-root",
        "ps-init-root",
        vec![
            initial("ps-init-root", "s-operating"),
            StateNode::Composite(operating),
            StateNode::Simple(suspended),
        ],
    );

    let mut m = machine("HistoryDemo", root);
    m.events.push(event("ev-start", "START"));
    m.events.push(event("ev-stop", "STOP"));
    m.events.push(event("ev-suspend", "SUSPEND"));
    m.events.push(event("ev-resume", "RESUME"));
    ir_one_machine(m)
}

#[test]
fn first_resume_uses_default_target() {
    let ir = build_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "HistoryDemo".into(),
            ..Default::default()
        })
        .unwrap();
    // Start in Operating.Idle.
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
    // Suspend (without working) — history records Idle.
    interp.dispatch("SUSPEND").unwrap();
    assert_eq!(interp.current_states(), vec!["s-suspended".to_string()]);
    // Resume — history defaults / records Idle, so we end up in Idle.
    interp.dispatch("RESUME").unwrap();
    assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
}

#[test]
fn resume_restores_prior_substate() {
    let ir = build_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "HistoryDemo".into(),
            ..Default::default()
        })
        .unwrap();
    interp.dispatch("START").unwrap();
    assert_eq!(interp.current_states(), vec!["s-working".to_string()]);
    interp.dispatch("SUSPEND").unwrap();
    interp.dispatch("RESUME").unwrap();
    // History recorded Working as the direct child at exit.
    assert_eq!(interp.current_states(), vec!["s-working".to_string()]);
}
