//! Self-transition exit/entry semantics — Doc 00 §7.7 / B-09.
//!
//! External self-transition (`S -> S`) must exit and re-enter S (firing both
//! exit and entry actions). Local self-transition (`S ~> S`) must NOT exit
//! or re-enter S.

mod common;

use common::*;
use fsm_ir::{StateNode, Statement, TransitionKind};
use fsm_simulator::{InitOptions, Interpreter};

fn ir_with_kind(kind: TransitionKind) -> fsm_ir::Ir {
    let mut s = simple("s-main");
    // Add an entry/exit action so we can observe whether it ran.
    s.entry.push(Statement::Call {
        callee: "ext_on_entry".into(),
        args: vec![],
    });
    s.exit.push(Statement::Call {
        callee: "ext_on_exit".into(),
        args: vec![],
    });
    s.transitions
        .push(transition("t-self", "s-main", "s-main", "ev-tick", kind));
    let root = region(
        "r-root",
        "ps-init",
        vec![initial("ps-init", "s-main"), StateNode::Simple(s)],
    );
    let mut m = machine("SelfDemo", root);
    m.events.push(event("ev-tick", "TICK"));
    ir_one_machine(m)
}

#[test]
fn external_self_transition_runs_exit_and_entry() {
    let ir = ir_with_kind(TransitionKind::External);
    let mut interp = Interpreter::new(&ir).unwrap();
    let init_recs = interp
        .init(InitOptions {
            machine_name: "SelfDemo".into(),
            ..Default::default()
        })
        .unwrap();
    // Init invokes entry once.
    let init_entry_calls = init_recs[0]
        .actions_executed
        .iter()
        .filter(|a| a.as_str() == "ext_on_entry")
        .count();
    assert_eq!(init_entry_calls, 1);

    let recs = interp.dispatch("TICK").unwrap();
    assert_eq!(recs.len(), 1);
    let r = &recs[0];
    // External self-transition: must record exit and entry of s-main.
    assert_eq!(r.exited_states, vec!["s-main".to_string()]);
    assert_eq!(r.entered_states, vec!["s-main".to_string()]);
    let calls = &r.actions_executed;
    assert_eq!(
        calls,
        &vec!["ext_on_exit".to_string(), "ext_on_entry".to_string()]
    );
}

#[test]
fn local_self_transition_skips_exit_and_entry() {
    let ir = ir_with_kind(TransitionKind::Local);
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "SelfDemo".into(),
            ..Default::default()
        })
        .unwrap();
    let recs = interp.dispatch("TICK").unwrap();
    assert_eq!(recs.len(), 1);
    let r = &recs[0];
    // Local self-transition: source is not exited nor re-entered.
    assert!(r.exited_states.is_empty(), "local must not exit");
    assert!(r.entered_states.is_empty(), "local must not enter");
    assert!(r.actions_executed.is_empty());
}

#[test]
fn internal_self_transition_runs_only_actions() {
    let ir = ir_with_kind(TransitionKind::Internal);
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "SelfDemo".into(),
            ..Default::default()
        })
        .unwrap();
    let recs = interp.dispatch("TICK").unwrap();
    assert_eq!(recs.len(), 1);
    let r = &recs[0];
    // Internal: no exit / entry.
    assert!(r.exited_states.is_empty());
    assert!(r.entered_states.is_empty());
}
