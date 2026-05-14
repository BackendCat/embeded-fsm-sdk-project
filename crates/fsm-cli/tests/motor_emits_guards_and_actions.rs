//! Regression for P0-1 (AUDIT_2026_05_14 §P0-1).
//!
//! Pre-fix, the AST → IR lowerer hard-coded `guard: None`, `actions: []` for
//! every transition kind, so the generated C compiled cleanly but contained
//! no calls to user externs and never updated the context. This test parses
//! `examples/motor/motor.fsm`, lowers through `fsm-analyzer`, runs `emit()`
//! from `fsm-codegen-c`, and asserts every user-authored guard / action /
//! payload reference shows up verbatim in the generated `Motor.c`.
//!
//! These string-level assertions deliberately overlap with the
//! `golden_simulator_runs_motor` test in this same target — the simulator
//! variant proves behavioural correctness (state advances, ctx mutates),
//! this variant proves the generated C is structurally faithful, and
//! together they pin both halves of the analyzer↔codegen + analyzer↔sim
//! contracts.

use std::fs;

use fsm_analyzer::analyze_with_source;
use fsm_codegen_c::{emit, CodegenConfig};
use fsm_parser::parse;

const RICH_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/motor/motor.fsm"
);

#[test]
fn motor_emits_guard_call_for_can_start() {
    let c = emit_motor_c();
    assert!(
        c.contains("can_start"),
        "Motor.c must reference the `can_start` guard extern; got:\n{c}"
    );
    assert!(
        c.contains("if (!can_start())"),
        "guard must lower to a real conditional, not a no-op:\n{c}"
    );
}

#[test]
fn motor_emits_set_speed_with_literal_arg() {
    let c = emit_motor_c();
    assert!(
        c.contains("set_speed(100)"),
        "Motor.c must emit `set_speed(100)` from the START action; got:\n{c}"
    );
    assert!(
        c.contains("set_speed(0)"),
        "Motor.c must emit `set_speed(0)` from the STOP action; got:\n{c}"
    );
}

#[test]
fn motor_emits_count_increment_assignment() {
    let c = emit_motor_c();
    // The Pratt parser wraps binary expressions in parens at emit time, so
    // we accept both `count + 1` and `(count + 1)` shapes.
    let direct = c.contains("m->context.count = m->context.count + 1");
    let parenthesised = c.contains("m->context.count = (m->context.count + 1)");
    assert!(
        direct || parenthesised,
        "Motor.c must assign `count = count + 1` from the START action; got:\n{c}"
    );
}

#[test]
fn motor_emits_reset_link_call_from_fault_action() {
    let c = emit_motor_c();
    assert!(
        c.contains("reset_link()"),
        "Motor.c must emit `reset_link()` from the FAULT action; got:\n{c}"
    );
}

#[test]
fn motor_emits_payload_assignment_into_context() {
    let c = emit_motor_c();
    // `ctx.last_fault_code = payload.code` lowers to a ctx-LHS / payload-RHS
    // assignment. We tolerate either the bare-union or event-tagged payload
    // form so the assertion survives later codegen refinements.
    assert!(
        c.contains("m->context.last_fault_code"),
        "Motor.c must write to `last_fault_code`; got:\n{c}"
    );
    assert!(
        c.contains("ev->__payload.FAULT.code") || c.contains("ev->__payload.code"),
        "Motor.c must read the FAULT payload `code` field; got:\n{c}"
    );
}

fn emit_motor_c() -> String {
    let src = fs::read_to_string(RICH_FIXTURE).expect("read examples/motor/motor.fsm");
    let pr = parse(&src);
    let result = analyze_with_source(&pr, RICH_FIXTURE, &src);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == fsm_diagnostics::Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "analyzer reported errors on examples/motor/motor.fsm: {:?}",
        errors
    );
    let ir = result.ir.expect("ir produced");
    let out = emit(&ir, &CodegenConfig::default()).expect("emit");
    out.find("Motor.c")
        .expect("Motor.c emitted")
        .content
        .clone()
}
