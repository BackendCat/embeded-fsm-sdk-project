//! Codegen ↔ simulator equivalence-gate smoke test (skip-friendly).
//!
//! v1.0 placeholder. The full equivalence gate requires the generated C
//! runtime to emit `StepRecord`-shaped trace events back to the host so we
//! can diff them against the simulator. `fsm-codegen-c` does not yet expose
//! the instrumented hooks needed for this (Doc 13 §11 trace shape is host-
//! side only). The test therefore:
//!
//! 1. Smoke-checks the simulator on the same IR a codegen integration test
//!    uses (a single START / STOP cycle), so the conformance harness can
//!    flip a flag to enable real diffing once codegen-c lands the hooks.
//! 2. Skips silently if `gcc` is not on PATH (so CI without a toolchain
//!    still passes).

mod common;

use std::process::Command;

use common::*;
use fsm_ir::{StateNode, TransitionKind};
use fsm_simulator::{InitOptions, Interpreter};

fn motor_ir() -> fsm_ir::Ir {
    let mut idle = simple("s-idle");
    idle.transitions.push(transition(
        "t-start",
        "s-idle",
        "s-running",
        "ev-start",
        TransitionKind::External,
    ));
    let mut running = simple("s-running");
    running.transitions.push(transition(
        "t-stop",
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
fn simulator_run_matches_codegen_when_gcc_available() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .unwrap();
    let r1 = interp.dispatch("START").unwrap();
    let r2 = interp.dispatch("STOP").unwrap();
    assert_eq!(r1[0].entered_states, vec!["s-running".to_string()]);
    assert_eq!(r2[0].entered_states, vec!["s-idle".to_string()]);

    // The codegen-side half of the equivalence gate is TODO — see
    // `tests/codegen_equivalence_smoke.rs` module docstring. When that
    // arrives, the block below should compile the generated C, run it with
    // the same event sequence, and assert the simulator's StepRecord list
    // matches the C runtime's emitted trace.
    let gcc_ok = Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !gcc_ok {
        eprintln!("skipping: gcc not on PATH (this is expected on CI without a C toolchain)");
    }
    // For v1.0 we only assert the simulator side; the codegen-hooks land
    // post-v1.0 — see Doc 23 §4.
}
