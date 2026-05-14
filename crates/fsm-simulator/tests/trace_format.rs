//! Trace file round-trip + `execute_trace` driver — Doc 15 §7, Doc 13 §11.

mod common;

use common::*;
use fsm_ir::{StateNode, TransitionKind};
use fsm_simulator::{
    execute_trace, parse_trace_yaml, write_trace_yaml, InitTrace, StepKind, TraceCommand, TraceFile,
};

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
        "ps-init",
        vec![
            initial("ps-init", "s-idle"),
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
fn round_trip_preserves_steps() {
    let t = TraceFile {
        machine_file: Some("motor.fsm".into()),
        description: Some("trace round-trip".into()),
        init: InitTrace {
            machine_name: Some("Motor".into()),
            ..Default::default()
        },
        steps: vec![
            TraceCommand::Dispatch {
                event: "START".into(),
                payload: None,
            },
            TraceCommand::Dispatch {
                event: "STOP".into(),
                payload: None,
            },
        ],
        expected: vec![],
    };
    let s = write_trace_yaml(&t).unwrap();
    let back = parse_trace_yaml(&s).unwrap();
    assert_eq!(t, back);
}

#[test]
fn execute_trace_produces_step_records() {
    let ir = motor_ir();
    let trace = TraceFile {
        machine_file: None,
        description: None,
        init: InitTrace {
            machine_name: Some("Motor".into()),
            ..Default::default()
        },
        steps: vec![
            TraceCommand::Dispatch {
                event: "START".into(),
                payload: None,
            },
            TraceCommand::Dispatch {
                event: "STOP".into(),
                payload: None,
            },
        ],
        expected: vec![],
    };
    let result = execute_trace(&ir, &trace).unwrap();
    // Init + START + STOP = 3 step records.
    assert_eq!(result.actual.len(), 3);
    assert_eq!(result.actual[0].kind, StepKind::Init);
    assert_eq!(result.actual[1].kind, StepKind::Dispatched);
    assert_eq!(result.actual[2].kind, StepKind::Dispatched);
    assert_eq!(
        result.actual[1].entered_states,
        vec!["s-running".to_string()]
    );
    assert!(result.matches_expected);
}

#[test]
fn execute_trace_diffs_against_expected() {
    let ir = motor_ir();
    let trace_template = TraceFile {
        machine_file: None,
        description: None,
        init: InitTrace {
            machine_name: Some("Motor".into()),
            ..Default::default()
        },
        steps: vec![TraceCommand::Dispatch {
            event: "START".into(),
            payload: None,
        }],
        expected: vec![],
    };
    let captured = execute_trace(&ir, &trace_template).unwrap();
    // Replay with the captured expected records; should match.
    let mut t2 = trace_template.clone();
    t2.expected = captured.actual.clone();
    let result = execute_trace(&ir, &t2).unwrap();
    assert!(result.matches_expected);
    assert!(result.first_mismatch.is_none());
}
