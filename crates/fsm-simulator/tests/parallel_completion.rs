//! Parallel completion semantics — Doc 00 §7.6 / B-08.
//!
//! Machine:
//! ```text
//! parallel Monitor {
//!   region Sensors {
//!     state Active {
//!       on FINISH_A -> SensorsDone
//!     }
//!     final SensorsDone
//!   }
//!   region Output {
//!     state On {
//!       on FINISH_B -> OutputDone
//!     }
//!     final OutputDone
//!   }
//! }
//! state Idle (initial)
//! Idle —GO→ Monitor
//! Monitor —done→ Stopped
//! state Stopped
//! ```
//!
//! Completion of Monitor fires only when BOTH regions reach Final.

mod common;

use common::*;
use fsm_ir::{StateNode, TransitionKind};
use fsm_simulator::{InitOptions, Interpreter};

fn build_ir() -> fsm_ir::Ir {
    // Region A
    let mut active = simple("s-active");
    active.transitions.push(transition(
        "t-active-done",
        "s-active",
        "s-sensors-done",
        "ev-finish-a",
        TransitionKind::External,
    ));
    let region_sensors = region(
        "r-sensors",
        "ps-init-sensors",
        vec![
            initial("ps-init-sensors", "s-active"),
            StateNode::Simple(active),
            final_state("s-sensors-done"),
        ],
    );

    // Region B
    let mut on = simple("s-on");
    on.transitions.push(transition(
        "t-on-done",
        "s-on",
        "s-output-done",
        "ev-finish-b",
        TransitionKind::External,
    ));
    let region_output = region(
        "r-output",
        "ps-init-output",
        vec![
            initial("ps-init-output", "s-on"),
            StateNode::Simple(on),
            final_state("s-output-done"),
        ],
    );

    let mut monitor = parallel("s-monitor", vec![region_sensors, region_output]);
    // Completion transition: when both regions are final, Monitor completes.
    monitor.transitions.push(completion_transition(
        "t-monitor-done",
        "s-monitor",
        "s-stopped",
    ));

    let mut idle = simple("s-idle");
    idle.transitions.push(transition(
        "t-idle-monitor",
        "s-idle",
        "s-monitor",
        "ev-go",
        TransitionKind::External,
    ));
    let stopped = simple("s-stopped");

    let root = region(
        "r-root",
        "ps-init-root",
        vec![
            initial("ps-init-root", "s-idle"),
            StateNode::Simple(idle),
            StateNode::Parallel(monitor),
            StateNode::Simple(stopped),
        ],
    );

    let mut m = machine("ParallelDemo", root);
    m.events.push(event("ev-go", "GO"));
    m.events.push(event("ev-finish-a", "FINISH_A"));
    m.events.push(event("ev-finish-b", "FINISH_B"));
    ir_one_machine(m)
}

#[test]
fn entering_parallel_activates_all_region_initials() {
    let ir = build_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "ParallelDemo".into(),
            ..Default::default()
        })
        .unwrap();
    interp.dispatch("GO").unwrap();
    let mut active = interp.current_states();
    active.sort();
    assert_eq!(
        active,
        vec!["s-active".to_string(), "s-on".to_string()],
        "both region initials must be active when Monitor is entered"
    );
}

#[test]
fn one_region_final_does_not_fire_parent_completion() {
    let ir = build_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "ParallelDemo".into(),
            ..Default::default()
        })
        .unwrap();
    interp.dispatch("GO").unwrap();
    interp.dispatch("FINISH_A").unwrap();
    // Sensors done; Output still on `On`. Monitor must NOT have completed.
    let mut active = interp.current_states();
    active.sort();
    assert_eq!(
        active,
        vec!["s-on".to_string(), "s-sensors-done".to_string()],
        "Monitor must not complete while Output is non-final"
    );
}

#[test]
fn both_regions_final_fires_parent_completion() {
    let ir = build_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "ParallelDemo".into(),
            ..Default::default()
        })
        .unwrap();
    interp.dispatch("GO").unwrap();
    interp.dispatch("FINISH_A").unwrap();
    interp.dispatch("FINISH_B").unwrap();
    // Now both regions final → Monitor completion fires → Stopped.
    assert_eq!(interp.current_states(), vec!["s-stopped".to_string()]);
}
