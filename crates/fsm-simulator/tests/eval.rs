//! Guard / expression evaluation — Doc 09 §7, §9.

mod common;

use common::*;
use fsm_ir::{
    BinaryOp, CmpOp, ContextField, ContextSchema, Expr, FieldRef, GuardExpr, GuardOperand, IntLit,
    Literal, StateNode, Statement, TransitionKind, Type,
};
use fsm_simulator::{eval, runtime::Value, InitOptions, Interpreter};
use std::collections::HashMap;

#[test]
fn field_compare_with_literal() {
    let g = GuardExpr::FieldCmp {
        lhs: FieldRef::Ctx {
            field: "speed".into(),
        },
        op: CmpOp::Gt,
        rhs: GuardOperand::Literal(Literal::Int(IntLit {
            value: 5,
            loc: None,
        })),
    };
    let mut ctx = HashMap::new();
    ctx.insert("speed".to_string(), Value::I32(7));
    let externs = eval::ExternRegistry::new();
    let evctx = eval::EvalCtx {
        context: &ctx,
        payload: None,
        externs: &externs,
    };
    assert!(eval::eval_guard(&g, &evctx).unwrap());
}

#[test]
fn extern_call_in_guard_uses_registry() {
    let g = GuardExpr::ExternCall {
        callee: "isSpeedValid".into(),
        args: vec![],
    };
    let mut ctx = HashMap::new();
    let mut externs = eval::ExternRegistry::new();
    externs.register("isSpeedValid", |_| Value::Bool(true));
    let evctx = eval::EvalCtx {
        context: &ctx,
        payload: None,
        externs: &externs,
    };
    assert!(eval::eval_guard(&g, &evctx).unwrap());

    // Unknown extern defaults to Bool(false) in guard context.
    let g2 = GuardExpr::ExternCall {
        callee: "missing".into(),
        args: vec![],
    };
    assert!(!eval::eval_guard(&g2, &evctx).unwrap());
    ctx.insert("unused".to_string(), Value::Bool(true));
}

#[test]
fn and_short_circuits_and_combines() {
    let g = GuardExpr::And {
        left: Box::new(GuardExpr::FieldCmp {
            lhs: FieldRef::Ctx { field: "x".into() },
            op: CmpOp::Gt,
            rhs: GuardOperand::Literal(Literal::Int(IntLit {
                value: 0,
                loc: None,
            })),
        }),
        right: Box::new(GuardExpr::ExternCall {
            callee: "always_true".into(),
            args: vec![],
        }),
    };
    let mut ctx = HashMap::new();
    ctx.insert("x".to_string(), Value::I32(5));
    let mut externs = eval::ExternRegistry::new();
    externs.register("always_true", |_| Value::Bool(true));
    let evctx = eval::EvalCtx {
        context: &ctx,
        payload: None,
        externs: &externs,
    };
    assert!(eval::eval_guard(&g, &evctx).unwrap());
}

#[test]
fn assign_then_guard_uses_updated_value() {
    // Build a machine: state Idle { entry: ctx.x = 7; on TICK [ctx.x > 5] -> Idle }
    let mut idle = simple("s-idle");
    idle.entry.push(Statement::Assign {
        target: FieldRef::Ctx { field: "x".into() },
        value: Expr::Literal(Literal::Int(IntLit {
            value: 7,
            loc: None,
        })),
    });
    let mut t = transition(
        "t-tick",
        "s-idle",
        "s-idle",
        "ev-tick",
        TransitionKind::External,
    );
    t.guard = Some(GuardExpr::FieldCmp {
        lhs: FieldRef::Ctx { field: "x".into() },
        op: CmpOp::Gt,
        rhs: GuardOperand::Literal(Literal::Int(IntLit {
            value: 5,
            loc: None,
        })),
    });
    idle.transitions.push(t);

    let root = region(
        "r-root",
        "ps-init",
        vec![initial("ps-init", "s-idle"), StateNode::Simple(idle)],
    );
    let mut m = machine("EvalDemo", root);
    m.context = ContextSchema {
        fields: vec![ContextField {
            id: "cf-x".into(),
            name: "x".into(),
            ty: Type::Primitive { name: "i32".into() },
            default: Some(Literal::Int(IntLit {
                value: 0,
                loc: None,
            })),
            loc: loc(),
        }],
    };
    m.events.push(event("ev-tick", "TICK"));
    let ir = ir_one_machine(m);

    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "EvalDemo".into(),
            ..Default::default()
        })
        .unwrap();
    // The entry action assigned x=7, so guard `x > 5` is true → transition fires.
    let recs = interp.dispatch("TICK").unwrap();
    assert!(
        recs[0].transition_taken.is_some(),
        "guard should have passed"
    );
    assert_eq!(interp.context().get("x"), Some(&Value::I32(7)));
}

#[test]
fn binary_arithmetic_widens_signed() {
    let ctx = HashMap::new();
    let externs = eval::ExternRegistry::new();
    let evctx = eval::EvalCtx {
        context: &ctx,
        payload: None,
        externs: &externs,
    };
    let e = Expr::Binary {
        op: BinaryOp::Add,
        left: Box::new(Expr::Literal(Literal::Int(IntLit {
            value: 100,
            loc: None,
        }))),
        right: Box::new(Expr::Literal(Literal::Int(IntLit {
            value: 200,
            loc: None,
        }))),
    };
    let v = eval::eval_expr(&e, &evctx).unwrap();
    assert_eq!(v.to_i64(), Some(300));
}

/// Regression for P0-1 (AUDIT_2026_05_14 §P0-1): a transition whose guard
/// evaluates to `false` MUST NOT fire. Pre-fix, every guard was `None` and
/// every transition fired unconditionally — the audit's whole point.
#[test]
fn guarded_transition_is_gated_by_extern_return() {
    // state Idle { on TICK [can_start] -> Running }   state Running {}
    let mut idle = simple("s-idle");
    let mut t = transition(
        "t-tick",
        "s-idle",
        "s-running",
        "ev-tick",
        TransitionKind::External,
    );
    t.guard = Some(GuardExpr::ExternCall {
        callee: "can_start".into(),
        args: vec![],
    });
    idle.transitions.push(t);
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
    let mut m = machine("GuardDemo", root);
    m.events.push(event("ev-tick", "TICK"));
    let ir = ir_one_machine(m);

    // Pass 1: extern returns false → state stays in Idle.
    {
        let mut interp = Interpreter::new(&ir).unwrap();
        interp
            .externs_mut()
            .register("can_start", |_| Value::Bool(false));
        interp
            .init(InitOptions {
                machine_name: "GuardDemo".into(),
                ..Default::default()
            })
            .unwrap();
        let recs = interp.dispatch("TICK").unwrap();
        assert!(
            recs.iter().all(|r| r.transition_taken.is_none()),
            "guard returned false — transition must not fire: {:?}",
            recs
        );
        assert_eq!(interp.current_states(), vec!["s-idle".to_string()]);
    }
    // Pass 2: extern returns true → state moves to Running.
    {
        let mut interp = Interpreter::new(&ir).unwrap();
        interp
            .externs_mut()
            .register("can_start", |_| Value::Bool(true));
        interp
            .init(InitOptions {
                machine_name: "GuardDemo".into(),
                ..Default::default()
            })
            .unwrap();
        let recs = interp.dispatch("TICK").unwrap();
        assert!(
            recs.iter().any(|r| r.transition_taken.is_some()),
            "guard returned true — transition must fire: {:?}",
            recs
        );
        assert_eq!(interp.current_states(), vec!["s-running".to_string()]);
    }
}

/// Regression for P0-1: a transition's action statements MUST mutate the
/// running context. Asserts the lowerer-produced `Statement::Assign` is
/// actually executed during the RTC step. The companion DSL line is
/// `on START [can_start] -> Running : ctx.count = ctx.count + 1`.
#[test]
fn transition_action_mutates_context() {
    let mut idle = simple("s-idle");
    let mut t = transition(
        "t-incr",
        "s-idle",
        "s-running",
        "ev-tick",
        TransitionKind::External,
    );
    t.actions.push(Statement::Assign {
        target: FieldRef::Ctx { field: "n".into() },
        value: Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::FieldRef {
                field_ref: FieldRef::Ctx { field: "n".into() },
            }),
            right: Box::new(Expr::Literal(Literal::Int(IntLit {
                value: 1,
                loc: None,
            }))),
        },
    });
    idle.transitions.push(t);
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
    let mut m = machine("ActionDemo", root);
    m.context = ContextSchema {
        fields: vec![ContextField {
            id: "cf-n".into(),
            name: "n".into(),
            ty: Type::Primitive { name: "i32".into() },
            default: Some(Literal::Int(IntLit {
                value: 0,
                loc: None,
            })),
            loc: loc(),
        }],
    };
    m.events.push(event("ev-tick", "TICK"));
    let ir = ir_one_machine(m);

    let mut interp = Interpreter::new(&ir).unwrap();
    interp
        .init(InitOptions {
            machine_name: "ActionDemo".into(),
            ..Default::default()
        })
        .unwrap();
    interp.dispatch("TICK").unwrap();
    assert_eq!(
        interp.context().get("n"),
        Some(&Value::I32(1)),
        "action must run: n=0+1=1 expected"
    );
}
