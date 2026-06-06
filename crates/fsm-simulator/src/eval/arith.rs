//! Operator semantics — shared between guard and expression evaluation.
//!
//! All arithmetic widens to `i64` or `f64`. The final result type is the
//! "wider" of the two operands' widths (mirroring C99 usual arithmetic
//! conversions emitted by `fsm-codegen-c`). When either operand is float,
//! the result is float; otherwise integer.
//!
//! The bitwise / shift operators only operate on integer values; on a
//! mismatch we return [`EvalError::TypeError`] rather than guess.

use fsm_ir::{BinaryOp, CmpOp, Literal, UnaryOp};

use super::expr::EvalError;
use crate::runtime::value::Value;

pub fn literal_to_value(lit: &Literal) -> Value {
    match lit {
        Literal::Int(i) => Value::I64(i.value),
        Literal::Float(f) => Value::F64(f.value),
        Literal::Bool(b) => Value::Bool(b.value),
        Literal::String(s) => Value::String(s.value.clone()),
        Literal::EnumVariant(e) => Value::Enum(e.enum_name.clone(), e.variant_name.clone()),
    }
}

pub fn apply_unary(op: UnaryOp, v: Value) -> Result<Value, EvalError> {
    match op {
        UnaryOp::Not => Ok(Value::Bool(!v.as_bool())),
        // AUDIT_2026_06_06 §2.4 P1.3 — explicit F32/F64 match removes
        // `to_f64().unwrap()` and the (unreachable, given `to_i64` is
        // saturating-Some for all numeric variants) `else if to_f64()`
        // fallback. Behaviour identical: floats stay floats, integers
        // stay integers, String/Enum are the only TypeError cases.
        UnaryOp::Neg => match v {
            Value::F32(x) => Ok(Value::F64(-(x as f64))),
            Value::F64(x) => Ok(Value::F64(-x)),
            other => match other.to_i64() {
                Some(i) => Ok(Value::I64(-i)),
                None => Err(EvalError::TypeError(format!(
                    "unary `-` on {}",
                    other.type_name()
                ))),
            },
        },
        UnaryOp::BitNot => {
            let i = v
                .to_i64()
                .ok_or_else(|| EvalError::TypeError(format!("unary `~` on {}", v.type_name())))?;
            // Preserve width when possible.
            Ok(match v {
                Value::U8(_) => Value::U8(!(i as u8)),
                Value::U16(_) => Value::U16(!(i as u16)),
                Value::U32(_) => Value::U32(!(i as u32)),
                Value::U64(_) => Value::U64(!(i as u64)),
                Value::I8(_) => Value::I8(!(i as i8)),
                Value::I16(_) => Value::I16(!(i as i16)),
                Value::I32(_) => Value::I32(!(i as i32)),
                _ => Value::I64(!i),
            })
        }
    }
}

pub fn apply_binary(op: BinaryOp, l: Value, r: Value) -> Result<Value, EvalError> {
    use BinaryOp::*;
    let use_float = Value::either_float(&l, &r);
    match op {
        Add | Sub | Mul | Div | Mod => {
            if use_float {
                let lf = l.to_f64().ok_or_else(|| ty_err(&l, op))?;
                let rf = r.to_f64().ok_or_else(|| ty_err(&r, op))?;
                let v = match op {
                    Add => lf + rf,
                    Sub => lf - rf,
                    Mul => lf * rf,
                    Div => lf / rf,
                    Mod => lf % rf,
                    _ => unreachable!(),
                };
                Ok(Value::F64(v))
            } else {
                let li = l.to_i64().ok_or_else(|| ty_err(&l, op))?;
                let ri = r.to_i64().ok_or_else(|| ty_err(&r, op))?;
                let v = match op {
                    Add => li.wrapping_add(ri),
                    Sub => li.wrapping_sub(ri),
                    Mul => li.wrapping_mul(ri),
                    Div => {
                        if ri == 0 {
                            return Err(EvalError::TypeError("division by zero".into()));
                        }
                        li.wrapping_div(ri)
                    }
                    Mod => {
                        if ri == 0 {
                            return Err(EvalError::TypeError("modulo by zero".into()));
                        }
                        li.wrapping_rem(ri)
                    }
                    _ => unreachable!(),
                };
                Ok(promote_to_lhs_width(&l, v))
            }
        }
        BitAnd | BitOr | BitXor | Shl | Shr => {
            let li = l.to_i64().ok_or_else(|| ty_err(&l, op))?;
            let ri = r.to_i64().ok_or_else(|| ty_err(&r, op))?;
            let v = match op {
                BitAnd => li & ri,
                BitOr => li | ri,
                BitXor => li ^ ri,
                Shl => li.wrapping_shl(ri as u32),
                Shr => li.wrapping_shr(ri as u32),
                _ => unreachable!(),
            };
            Ok(promote_to_lhs_width(&l, v))
        }
        LogAnd | LogOr => unreachable!("logical ops short-circuit in eval_expr"),
        Eq | NotEq | Lt | Gt | LtEq | GtEq => {
            let cmp = match op {
                Eq => CmpOp::Eq,
                NotEq => CmpOp::NotEq,
                Lt => CmpOp::Lt,
                Gt => CmpOp::Gt,
                LtEq => CmpOp::LtEq,
                GtEq => CmpOp::GtEq,
                _ => unreachable!(),
            };
            Ok(Value::Bool(apply_compare(cmp, &l, &r)?))
        }
    }
}

pub fn apply_compare(op: CmpOp, l: &Value, r: &Value) -> Result<bool, EvalError> {
    // String / enum: equality only.
    if matches!(l, Value::String(_) | Value::Enum(_, _))
        || matches!(r, Value::String(_) | Value::Enum(_, _))
    {
        let eq = l == r;
        return Ok(match op {
            CmpOp::Eq => eq,
            CmpOp::NotEq => !eq,
            _ => {
                return Err(EvalError::TypeError(
                    "ordering on string/enum not supported".into(),
                ));
            }
        });
    }
    if Value::either_float(l, r) {
        let lf = l.to_f64().ok_or_else(|| ty_err(l, BinaryOp::Eq))?;
        let rf = r.to_f64().ok_or_else(|| ty_err(r, BinaryOp::Eq))?;
        return Ok(match op {
            CmpOp::Eq => lf == rf,
            CmpOp::NotEq => lf != rf,
            CmpOp::Lt => lf < rf,
            CmpOp::Gt => lf > rf,
            CmpOp::LtEq => lf <= rf,
            CmpOp::GtEq => lf >= rf,
        });
    }
    let li = l.to_i64().ok_or_else(|| ty_err(l, BinaryOp::Eq))?;
    let ri = r.to_i64().ok_or_else(|| ty_err(r, BinaryOp::Eq))?;
    Ok(match op {
        CmpOp::Eq => li == ri,
        CmpOp::NotEq => li != ri,
        CmpOp::Lt => li < ri,
        CmpOp::Gt => li > ri,
        CmpOp::LtEq => li <= ri,
        CmpOp::GtEq => li >= ri,
    })
}

fn ty_err(v: &Value, op: BinaryOp) -> EvalError {
    EvalError::TypeError(format!("operator {:?} on {}", op, v.type_name()))
}

/// Narrow a widened-i64 result to the type of `lhs` so that overflow matches
/// the emitted C99 (which uses the LHS type). Conservative — when the LHS is
/// a `Bool` / `Enum` / `String` we fall back to `I64`.
fn promote_to_lhs_width(lhs: &Value, val: i64) -> Value {
    match lhs {
        Value::I8(_) => Value::I8(val as i8),
        Value::I16(_) => Value::I16(val as i16),
        Value::I32(_) => Value::I32(val as i32),
        Value::I64(_) => Value::I64(val),
        Value::U8(_) => Value::U8(val as u8),
        Value::U16(_) => Value::U16(val as u16),
        Value::U32(_) => Value::U32(val as u32),
        Value::U64(_) => Value::U64(val as u64),
        _ => Value::I64(val),
    }
}
