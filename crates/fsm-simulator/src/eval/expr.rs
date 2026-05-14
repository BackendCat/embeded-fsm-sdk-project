//! Pure evaluation of guard and action expressions over the runtime context.
//!
//! Guard expressions go through [`eval_guard`]; action-language expressions
//! through [`eval_expr`]. The two languages overlap heavily so they share
//! arithmetic helpers in [`super::arith`].

use std::collections::BTreeMap;

use fsm_ir::{BinaryOp, CmpOp, Expr, FieldRef, GuardExpr, GuardOperand, UnaryOp};
use thiserror::Error;

use super::arith::{apply_binary, apply_compare, apply_unary, literal_to_value};
use super::extern_registry::ExternRegistry;
use crate::runtime::value::Value;

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("undefined context field: {0}")]
    UndefinedField(String),
    #[error("undefined payload field: {0}")]
    UndefinedPayloadField(String),
    #[error("payload not available in this scope")]
    PayloadUnavailable,
    #[error("type error: {0}")]
    TypeError(String),
}

/// Read-only view of mutable context the evaluator can see. Both context
/// fields and the optional current event payload are passed by reference so
/// guard evaluation cannot mutate them (Doc 08 §4.3 — guards MUST NOT have
/// side effects).
///
/// Maps are `BTreeMap` so that any caller-side iteration during eval is in
/// deterministic key order — matches the wire-format determinism contract
/// enforced for snapshot / trace serialisation.
pub struct EvalCtx<'a> {
    pub context: &'a BTreeMap<String, Value>,
    pub payload: Option<&'a BTreeMap<String, Value>>,
    pub externs: &'a ExternRegistry,
}

/// Resolve a `FieldRef` to its current value.
pub fn eval_field_ref(field: &FieldRef, ctx: &EvalCtx) -> Result<Value, EvalError> {
    match field {
        FieldRef::Ctx { field } => ctx
            .context
            .get(field)
            .cloned()
            .ok_or_else(|| EvalError::UndefinedField(field.clone())),
        FieldRef::Payload { field } => {
            let payload = ctx.payload.ok_or(EvalError::PayloadUnavailable)?;
            payload
                .get(field)
                .cloned()
                .ok_or_else(|| EvalError::UndefinedPayloadField(field.clone()))
        }
    }
}

/// Evaluate an `Expr` from the action language (Doc 09 §9).
pub fn eval_expr(expr: &Expr, ctx: &EvalCtx) -> Result<Value, EvalError> {
    match expr {
        Expr::FieldRef { field_ref } => eval_field_ref(field_ref, ctx),
        Expr::Literal(lit) => Ok(literal_to_value(lit)),
        Expr::Call { callee, args } => {
            let mut argv = Vec::with_capacity(args.len());
            for a in args {
                argv.push(eval_expr(a, ctx)?);
            }
            Ok(ctx.externs.invoke(callee, &argv, false))
        }
        Expr::Unary { op, operand } => {
            let v = eval_expr(operand, ctx)?;
            apply_unary(*op, v)
        }
        Expr::Binary { op, left, right } => {
            // Logical operators short-circuit per Doc 04 §8 — match C semantics.
            match op {
                BinaryOp::LogAnd => {
                    let l = eval_expr(left, ctx)?;
                    if !l.as_bool() {
                        return Ok(Value::Bool(false));
                    }
                    Ok(Value::Bool(eval_expr(right, ctx)?.as_bool()))
                }
                BinaryOp::LogOr => {
                    let l = eval_expr(left, ctx)?;
                    if l.as_bool() {
                        return Ok(Value::Bool(true));
                    }
                    Ok(Value::Bool(eval_expr(right, ctx)?.as_bool()))
                }
                _ => {
                    let l = eval_expr(left, ctx)?;
                    let r = eval_expr(right, ctx)?;
                    apply_binary(*op, l, r)
                }
            }
        }
        Expr::Cast(c) => {
            let inner = eval_expr(&c.operand, ctx)?;
            let target = match &c.target_type {
                fsm_ir::Type::Primitive { name } => name.as_str(),
                _ => {
                    return Err(EvalError::TypeError(
                        "cast target must be a primitive type".into(),
                    ));
                }
            };
            inner
                .cast_to(target)
                .ok_or_else(|| EvalError::TypeError(format!("cannot cast to {target}")))
        }
    }
}

/// Evaluate a [`GuardExpr`] to a boolean (Doc 04 §8.5).
pub fn eval_guard(g: &GuardExpr, ctx: &EvalCtx) -> Result<bool, EvalError> {
    match g {
        GuardExpr::FieldCmp { lhs, op, rhs } => {
            let l = eval_field_ref(lhs, ctx)?;
            let r = match rhs {
                GuardOperand::Literal(lit) => literal_to_value(lit),
                GuardOperand::FieldRef(f) => eval_field_ref(f, ctx)?,
            };
            apply_compare(*op, &l, &r)
        }
        GuardExpr::ExternCall { callee, args } => {
            let mut argv = Vec::with_capacity(args.len());
            for a in args {
                argv.push(eval_expr(a, ctx)?);
            }
            let v = ctx.externs.invoke(callee, &argv, true);
            Ok(v.as_bool())
        }
        GuardExpr::Not { operand } => Ok(!eval_guard(operand, ctx)?),
        GuardExpr::And { left, right } => Ok(eval_guard(left, ctx)? && eval_guard(right, ctx)?),
        GuardExpr::Or { left, right } => Ok(eval_guard(left, ctx)? || eval_guard(right, ctx)?),
        GuardExpr::Else => Ok(true),
    }
}

/// Apply a unary operator (re-export for stmt evaluator convenience).
pub use super::arith::{apply_binary as apply_binary_expr, apply_unary as apply_unary_expr};

#[allow(dead_code)]
fn unused_anchor(_: UnaryOp, _: CmpOp) {}
