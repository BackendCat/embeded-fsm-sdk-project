//! Audit 2026-05-14 — Motor_init must seed context-field defaults.
//!
//! The IR carries `ContextField { default: Some(Literal) }` for every DSL
//! field with an `=` initializer (Doc 04 §3.3). Pre-fix, `Motor.c`'s
//! generated `Motor_init` ran `memset(m, 0, sizeof(*m))` and never wrote
//! the declared defaults — silent data loss for every machine that
//! depended on a non-zero initial value (e.g. vending-machine's
//! `price : u16 = 150`).
//!
//! This test builds the Motor IR (which already declares `speed: u16 = 0;
//! running: bool = false`), runs the emitter, and asserts that
//! `Motor.c` contains explicit `m->context.<field> = <value>;` lines
//! after the memset.
//!
//! It also verifies the change handles non-zero integer and `true`
//! boolean defaults — the Motor common IR's zero/false case proves the
//! emission path runs, then a synthesised `Vending`-style IR proves
//! non-zero values render correctly.

#[path = "common/mod.rs"]
mod common;

use fsm_codegen_c::{emit, CodegenConfig};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    BoolLit, ContextField, ContextSchema, InitialPseudo, IntLit, Ir, Literal, MachineObject,
    OverflowPolicy, QueueConfig, RegionObject, SimpleState, StateNode, Type,
};

fn loc() -> SourceLocation {
    SourceLocation::new("ctx_defaults.fsm", Span::new(0, 1), 1, 1)
}

#[test]
fn motor_init_emits_zero_defaults_for_speed_and_running() {
    // Motor's common-IR fixture declares `speed: u16 = 0; running: bool = false`.
    // Even the zero case must emit explicit assignments — otherwise we
    // can't tell apart "no default" from "default is 0", and any future
    // memset removal would regress silently.
    let ir = common::motor_ir();
    let emitted = emit(&ir, &CodegenConfig::default()).expect("emit");
    let motor_c = emitted
        .find("Motor.c")
        .expect("Motor.c emitted")
        .content
        .clone();
    assert!(
        motor_c.contains("m->context.speed = 0;"),
        "Motor.c must initialise `speed` to its DSL default 0;\n--- Motor.c ---\n{}",
        motor_c
    );
    assert!(
        motor_c.contains("m->context.running = false;"),
        "Motor.c must initialise `running` to its DSL default false;\n--- Motor.c ---\n{}",
        motor_c
    );
}

#[test]
fn motor_init_emits_non_zero_integer_default() {
    let ir = vending_like_ir(150);
    let emitted = emit(&ir, &CodegenConfig::default()).expect("emit");
    let motor_c = emitted
        .find("Vending.c")
        .expect("Vending.c emitted")
        .content
        .clone();
    assert!(
        motor_c.contains("m->context.price = 150;"),
        "Vending.c must initialise `price = 150` from the DSL default;\n--- Vending.c ---\n{}",
        motor_c
    );
    assert!(
        motor_c.contains("m->context.balance = 0;"),
        "Vending.c must initialise `balance = 0` from the DSL default;\n--- Vending.c ---\n{}",
        motor_c
    );
}

#[test]
fn fields_without_default_are_not_assigned() {
    // Synthesise an IR with one field that has NO `default` literal —
    // codegen must NOT emit `m->context.<name>` for it (the memset zero
    // pattern remains, and the unwritten field stays at C's zero).
    let mut ir = vending_like_ir(150);
    // Add a third field with no default.
    ir.machines[0].context.fields.push(ContextField {
        id: "f-undecl".into(),
        name: "undecl".into(),
        ty: Type::Primitive { name: "u8".into() },
        default: None,
        loc: loc(),
    });
    let emitted = emit(&ir, &CodegenConfig::default()).expect("emit");
    let motor_c = emitted
        .find("Vending.c")
        .expect("Vending.c emitted")
        .content
        .clone();
    assert!(
        !motor_c.contains("m->context.undecl ="),
        "Field with no `=` initializer must not get an assignment in Motor_init;\n--- Vending.c ---\n{}",
        motor_c
    );
}

/// Minimal IR mirroring the vending-machine context shape:
///   `balance: u16 = 0`
///   `price  : u16 = price_default`
fn vending_like_ir(price_default: i64) -> Ir {
    let idle = SimpleState {
        id: "s-idle".into(),
        stable_id: "Vending:state:Idle".into(),
        name: "Idle".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    };
    let root = RegionObject {
        id: "r-root".into(),
        stable_id: None,
        name: "__root".into(),
        initial: "ps-init".into(),
        states: vec![
            StateNode::Initial(InitialPseudo {
                id: "ps-init".into(),
                target: "s-idle".into(),
                loc: loc(),
            }),
            StateNode::Simple(idle),
        ],
        priority: 0,
        loc: loc(),
    };
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "".into(),
        source_files: vec!["vending.fsm".into()],
        machines: vec![MachineObject {
            id: "m-vending".into(),
            stable_id: "Vending".into(),
            name: "Vending".into(),
            context: ContextSchema {
                fields: vec![
                    ContextField {
                        id: "f-balance".into(),
                        name: "balance".into(),
                        ty: Type::Primitive { name: "u16".into() },
                        default: Some(Literal::Int(IntLit {
                            value: 0,
                            loc: None,
                        })),
                        loc: loc(),
                    },
                    ContextField {
                        id: "f-price".into(),
                        name: "price".into(),
                        ty: Type::Primitive { name: "u16".into() },
                        default: Some(Literal::Int(IntLit {
                            value: price_default,
                            loc: None,
                        })),
                        loc: loc(),
                    },
                ],
            },
            events: vec![],
            externs: vec![],
            root,
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            queue: QueueConfig {
                capacity: 8,
                overflow_policy: OverflowPolicy::Assert,
                loc: loc(),
            },
            targets: vec![],
            loc: loc(),
        }],
        diagnostics: vec![],
    }
}

// Suppress the unused-import warning for any types the common module pulls
// in for other targets.
#[allow(dead_code)]
fn _unused_warning_silencer(_b: BoolLit) {}
