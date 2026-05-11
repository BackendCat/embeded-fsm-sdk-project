//! Statement lowering — IR `Statement` → C99 source lines.
//!
//! Statements include assignments, control flow (if/while/for), `raise`,
//! `send`, `defer`, and direct extern calls. Each emit produces an indented
//! sequence of C lines ready to splice into a generated function body.

use std::fmt::Write;

use fsm_ir::Statement;

use crate::expr::{emit_expr, emit_field_ref};

/// Configuration knobs for the per-machine statement emitter. The
/// machine-name prefix is used to emit calls like `Motor_raise(m, ...)`.
pub struct StmtContext<'a> {
    /// Prefix for emitted helper functions: e.g. `"Motor"` →
    /// `Motor_raise(m, EVT)`.
    pub machine_prefix: &'a str,
    /// C expression that yields the context root, e.g. `"m->context"`.
    pub ctx_prefix: &'a str,
    /// C expression that yields the payload root, e.g. `"ev->__payload"`.
    pub payload_prefix: &'a str,
}

/// Emit a statement list at the given indent level. Each line is preceded
/// by `indent` spaces and terminated by a newline.
pub fn emit_stmts(stmts: &[Statement], ctx: &StmtContext<'_>, indent: usize) -> String {
    let mut out = String::new();
    for s in stmts {
        emit_one(s, ctx, indent, &mut out);
    }
    out
}

fn emit_one(s: &Statement, ctx: &StmtContext<'_>, indent: usize, out: &mut String) {
    match s {
        Statement::Assign { target, value } => {
            let lhs = emit_field_ref(target, ctx.ctx_prefix, ctx.payload_prefix);
            let rhs = emit_expr(value, ctx.ctx_prefix, ctx.payload_prefix);
            writeln!(out, "{:indent$}{} = {};", "", lhs, rhs, indent = indent).unwrap();
        }
        Statement::If {
            condition,
            then,
            else_,
        } => {
            let cond = emit_expr(condition, ctx.ctx_prefix, ctx.payload_prefix);
            writeln!(out, "{:indent$}if ({}) {{", "", cond, indent = indent).unwrap();
            out.push_str(&emit_stmts(then, ctx, indent + 4));
            if !else_.is_empty() {
                writeln!(out, "{:indent$}}} else {{", "", indent = indent).unwrap();
                out.push_str(&emit_stmts(else_, ctx, indent + 4));
            }
            writeln!(out, "{:indent$}}}", "", indent = indent).unwrap();
        }
        Statement::While { condition, body } => {
            let cond = emit_expr(condition, ctx.ctx_prefix, ctx.payload_prefix);
            writeln!(out, "{:indent$}while ({}) {{", "", cond, indent = indent).unwrap();
            out.push_str(&emit_stmts(body, ctx, indent + 4));
            writeln!(out, "{:indent$}}}", "", indent = indent).unwrap();
        }
        Statement::For {
            init,
            condition,
            update,
            body,
        } => {
            // `for` lowers to an explicit while because the init/update
            // sub-statements can be assignments. Emitting a true C `for`
            // header would require flattening — the while form is simpler
            // and matches the formal-semantics §6.4 reduction.
            writeln!(out, "{:indent$}{{ /* for */", "", indent = indent).unwrap();
            emit_one(init, ctx, indent + 4, out);
            let cond = emit_expr(condition, ctx.ctx_prefix, ctx.payload_prefix);
            writeln!(
                out,
                "{:indent$}    while ({}) {{",
                "",
                cond,
                indent = indent
            )
            .unwrap();
            out.push_str(&emit_stmts(body, ctx, indent + 8));
            emit_one(update, ctx, indent + 8, out);
            writeln!(out, "{:indent$}    }}", "", indent = indent).unwrap();
            writeln!(out, "{:indent$}}}", "", indent = indent).unwrap();
        }
        Statement::Call { callee, args } => {
            let args_c = args
                .iter()
                .map(|a| emit_expr(a, ctx.ctx_prefix, ctx.payload_prefix))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                out,
                "{:indent$}{}({});",
                "",
                callee,
                args_c,
                indent = indent
            )
            .unwrap();
        }
        Statement::Raise { event_id, args: _ } => {
            // The event_id at IR time is the stable id; the emitter looks up
            // the C enum value through the machine event table. For the
            // codegen surface we render `Motor_raise(m, MOTOR_EVENT_X)` and
            // expect the outer emitter to bind `event_id` → enum name.
            writeln!(
                out,
                "{:indent$}{}_raise(m, /* event */ {});",
                "",
                ctx.machine_prefix,
                event_id,
                indent = indent
            )
            .unwrap();
        }
        Statement::Send {
            event_id,
            args: _,
            machine_id,
        } => {
            writeln!(
                out,
                "{:indent$}{}_send_to({}, /* event */ {});",
                "",
                ctx.machine_prefix,
                machine_id,
                event_id,
                indent = indent
            )
            .unwrap();
        }
        Statement::Defer { event_id } => {
            writeln!(
                out,
                "{:indent$}{}_defer(m, /* event */ {});",
                "",
                ctx.machine_prefix,
                event_id,
                indent = indent
            )
            .unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_ir::{Expr, FieldRef, IntLit, Literal, Statement};

    fn ctx<'a>() -> StmtContext<'a> {
        StmtContext {
            machine_prefix: "Motor",
            ctx_prefix: "m->context",
            payload_prefix: "ev->__payload",
        }
    }

    #[test]
    fn assignment_lowers_to_c_assign() {
        let s = Statement::Assign {
            target: FieldRef::Ctx {
                field: "speed".into(),
            },
            value: Expr::Literal(Literal::Int(IntLit {
                value: 1500,
                loc: None,
            })),
        };
        let out = emit_stmts(&[s], &ctx(), 4);
        assert!(out.contains("m->context.speed = 1500;"), "got: {out}");
    }

    #[test]
    fn if_block_renders_with_braces() {
        let s = Statement::If {
            condition: Expr::Literal(Literal::Bool(fsm_ir::BoolLit {
                value: true,
                loc: None,
            })),
            then: vec![Statement::Call {
                callee: "Motor_buzz".into(),
                args: vec![],
            }],
            else_: vec![],
        };
        let out = emit_stmts(&[s], &ctx(), 4);
        assert!(out.contains("if (true)"), "got: {out}");
        assert!(out.contains("Motor_buzz();"), "got: {out}");
    }

    #[test]
    fn raise_routes_through_machine_helper() {
        let s = Statement::Raise {
            event_id: "MOTOR_EVENT_FAULT".into(),
            args: vec![],
        };
        let out = emit_stmts(&[s], &ctx(), 4);
        assert!(out.contains("Motor_raise(m,"), "got: {out}");
        assert!(out.contains("MOTOR_EVENT_FAULT"), "got: {out}");
    }
}
