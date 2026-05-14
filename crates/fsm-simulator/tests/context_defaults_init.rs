//! Audit 2026-05-14 — context-field defaults must be applied at `init()`.
//!
//! Before the fix, the IR carried `ContextField { default: Some(Literal::Int(150)) }`
//! but `Interpreter::init` left the field at the runtime zero default unless
//! the caller supplied an `initial_context` override. That silently dropped
//! every `balance: u16 = 0` / `price: u16 = 150` annotation declared in the
//! DSL. The fix walks `machine.context.fields` at init time and seeds each
//! field's runtime value from its `default` literal.
//!
//! This file exercises three flavours of default:
//! - integer (`price: u16 = 150`)
//! - boolean (`enabled: bool = true`)
//! - the explicit absence-of-default case (zero-initialise)
//!
//! …and confirms that the post-init context() reflects them. The vending
//! machine's full lifecycle is covered by `cli_test` /
//! `vending_machine_gcc` — this test stays at the unit-test layer.

mod common;

use common::*;
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{BoolLit, ContextField, ContextSchema, IntLit, Literal, StateNode, Type};
use fsm_simulator::{InitOptions, Interpreter, Value};

fn loc_lit() -> Option<SourceLocation> {
    Some(SourceLocation::new("test.fsm", Span::new(0, 1), 1, 1))
}

fn ir_with_context_defaults() -> fsm_ir::Ir {
    let idle = simple("s-idle");
    let root = region(
        "r-root",
        "ps-init",
        vec![initial("ps-init", "s-idle"), StateNode::Simple(idle)],
    );
    let mut m = machine("CtxDefaults", root);
    m.context = ContextSchema {
        fields: vec![
            ContextField {
                id: "f-balance".into(),
                name: "balance".into(),
                ty: Type::Primitive { name: "u16".into() },
                default: Some(Literal::Int(IntLit {
                    value: 0,
                    loc: loc_lit(),
                })),
                loc: loc(),
            },
            ContextField {
                id: "f-price".into(),
                name: "price".into(),
                ty: Type::Primitive { name: "u16".into() },
                default: Some(Literal::Int(IntLit {
                    value: 150,
                    loc: loc_lit(),
                })),
                loc: loc(),
            },
            ContextField {
                id: "f-enabled".into(),
                name: "enabled".into(),
                ty: Type::Primitive {
                    name: "bool".into(),
                },
                default: Some(Literal::Bool(BoolLit {
                    value: true,
                    loc: loc_lit(),
                })),
                loc: loc(),
            },
            ContextField {
                id: "f-counter".into(),
                name: "counter".into(),
                ty: Type::Primitive { name: "u32".into() },
                default: None, // No DSL initializer — must be zero-initialised.
                loc: loc(),
            },
        ],
    };
    ir_one_machine(m)
}

#[test]
fn init_applies_integer_default() {
    let ir = ir_with_context_defaults();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "CtxDefaults".into(),
            ..Default::default()
        })
        .unwrap();
    let ctx = interp.context().expect("context after init");
    let price = ctx.get("price").expect("price field present");
    // Literal::Int lowers to Value::I64 — use to_i64 for tolerance.
    let n = price.to_i64().expect("price should be integer-like");
    assert_eq!(n, 150, "DSL default `price: u16 = 150` must seed runtime");
}

#[test]
fn init_applies_bool_default() {
    let ir = ir_with_context_defaults();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "CtxDefaults".into(),
            ..Default::default()
        })
        .unwrap();
    let ctx = interp.context().expect("context after init");
    match ctx.get("enabled").expect("enabled field present") {
        Value::Bool(b) => assert!(*b, "DSL default `enabled: bool = true` must seed runtime"),
        other => panic!("unexpected variant for enabled: {:?}", other),
    }
}

#[test]
fn init_default_zero_when_no_literal_declared() {
    let ir = ir_with_context_defaults();
    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "CtxDefaults".into(),
            ..Default::default()
        })
        .unwrap();
    let ctx = interp.context().expect("context after init");
    let counter = ctx.get("counter").expect("counter field present");
    let n = counter.to_i64().expect("counter should be integer-like");
    assert_eq!(
        n, 0,
        "field with no `=` initializer must be zero-initialised"
    );
}

#[test]
fn explicit_initial_context_overrides_default() {
    let ir = ir_with_context_defaults();
    let mut interp = Interpreter::new(&ir).unwrap();
    let mut overrides = std::collections::BTreeMap::new();
    overrides.insert("price".into(), Value::I64(99));
    interp
        .init(InitOptions {
            machine_name: "CtxDefaults".into(),
            initial_context: Some(overrides),
            ..Default::default()
        })
        .unwrap();
    let ctx = interp.context().expect("context after init");
    let n = ctx
        .get("price")
        .expect("price field present")
        .to_i64()
        .expect("price should be integer-like");
    assert_eq!(n, 99, "caller-supplied override must win");
}
