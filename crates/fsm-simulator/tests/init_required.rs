//! Audit D P1-A regression: `Interpreter` public API must not panic when a
//! caller forgets to invoke `init()`.
//!
//! Pre-fix `interpreter.rs:371, 382, 395` had three `expect("not initialized")`
//! calls reachable from `context()` / `snapshot()` / `restore()`. An embedder
//! writing a Rust-host test could abort the process by calling those before
//! `init`. Post-fix every entrypoint returns `Err(StepError::NotInitialized)`.

mod common;

use common::*;
use fsm_ir::{StateNode, TransitionKind};
use fsm_simulator::{InitOptions, Interpreter, StepError};

fn motor_ir() -> fsm_ir::Ir {
    let mut idle = simple("s-idle");
    idle.transitions.push(transition(
        "t-go",
        "s-idle",
        "s-running",
        "ev-go",
        TransitionKind::External,
    ));
    let running = simple("s-running");
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
    m.events.push(event("ev-go", "GO"));
    ir_one_machine(m)
}

#[test]
fn dispatch_before_init_returns_not_initialized() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    match interp.dispatch("GO") {
        Err(StepError::NotInitialized) => {}
        other => panic!("expected StepError::NotInitialized, got {other:?}"),
    }
}

#[test]
fn raise_before_init_returns_not_initialized() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    match interp.raise("GO") {
        Err(StepError::NotInitialized) => {}
        other => panic!("expected StepError::NotInitialized, got {other:?}"),
    }
}

#[test]
fn advance_clock_before_init_returns_not_initialized() {
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    match interp.advance_clock(100) {
        Err(StepError::NotInitialized) => {}
        other => panic!("expected StepError::NotInitialized, got {other:?}"),
    }
}

#[test]
fn context_before_init_returns_not_initialized() {
    let ir = motor_ir();
    let interp = Interpreter::new(&ir).unwrap();
    match interp.context() {
        Err(StepError::NotInitialized) => {}
        other => panic!("expected StepError::NotInitialized, got {other:?}"),
    }
}

#[test]
fn snapshot_before_init_returns_not_initialized() {
    let ir = motor_ir();
    let interp = Interpreter::new(&ir).unwrap();
    match interp.snapshot() {
        Err(StepError::NotInitialized) => {}
        other => panic!("expected StepError::NotInitialized, got {other:?}"),
    }
}

#[test]
fn after_init_dispatch_succeeds_proving_the_negative_test_is_real() {
    // Sanity check: the gate must let traffic through once init has run, so
    // the four "should error" tests above aren't passing by accident.
    let ir = motor_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .unwrap();
    interp
        .dispatch("GO")
        .expect("dispatch must work after init");
}
