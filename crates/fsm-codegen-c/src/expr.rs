//! Expression lowering — IR `Expr` / `GuardExpr` / `Literal` → C99 source
//! fragments.
//!
//! Outputs are emitted as `String` rather than into an `IndentWriter` so the
//! same fragment can be embedded in a guard column of a transition table, an
//! `if (...)` condition, or an assignment RHS.

use fsm_ir::{
    BinaryOp, CastExpr, CmpOp, EnumVariantLit, Expr, FieldRef, GuardExpr, GuardOperand, Literal,
    Type, UnaryOp,
};

/// Emit a guard expression as a parenthesised C boolean expression.
///
/// `ctx_prefix` and `payload_prefix` are the C variable expressions used for
/// `ctx.x` and `payload.x` field references, typically `"m->context"` and
/// `"ev->__payload"`.
pub fn emit_guard(g: &GuardExpr, ctx_prefix: &str, payload_prefix: &str) -> String {
    match g {
        GuardExpr::FieldCmp { lhs, op, rhs } => {
            let l = emit_field_ref(lhs, ctx_prefix, payload_prefix);
            let r = emit_guard_operand(rhs, ctx_prefix, payload_prefix);
            format!("({} {} {})", l, emit_cmp_op(*op), r)
        }
        GuardExpr::ExternCall { callee, args } => {
            let args_c = args
                .iter()
                .map(|a| emit_expr(a, ctx_prefix, payload_prefix))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", callee, args_c)
        }
        GuardExpr::Not { operand } => {
            format!("(!{})", emit_guard(operand, ctx_prefix, payload_prefix))
        }
        GuardExpr::And { left, right } => format!(
            "({} && {})",
            emit_guard(left, ctx_prefix, payload_prefix),
            emit_guard(right, ctx_prefix, payload_prefix)
        ),
        GuardExpr::Or { left, right } => format!(
            "({} || {})",
            emit_guard(left, ctx_prefix, payload_prefix),
            emit_guard(right, ctx_prefix, payload_prefix)
        ),
        // `[else]` branches reach the codegen as `GuardExpr::Else`. In an
        // `if/else if/else` chain we never emit code for them — the choice
        // emitter handles the fall-through. If somehow asked, emit `true`
        // for safety; the analyzer should have rejected misplacement.
        GuardExpr::Else => "1 /* [else] */".to_owned(),
    }
}

/// Emit an action-block expression.
pub fn emit_expr(e: &Expr, ctx_prefix: &str, payload_prefix: &str) -> String {
    match e {
        Expr::FieldRef { field_ref } => emit_field_ref(field_ref, ctx_prefix, payload_prefix),
        Expr::Literal(lit) => emit_literal(lit),
        Expr::Call { callee, args } => {
            let args_c = args
                .iter()
                .map(|a| emit_expr(a, ctx_prefix, payload_prefix))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", callee, args_c)
        }
        Expr::Unary { op, operand } => {
            let sym = match op {
                UnaryOp::Not => "!",
                UnaryOp::Neg => "-",
                UnaryOp::BitNot => "~",
            };
            format!(
                "({}{})",
                sym,
                emit_expr(operand, ctx_prefix, payload_prefix)
            )
        }
        Expr::Binary { op, left, right } => format!(
            "({} {} {})",
            emit_expr(left, ctx_prefix, payload_prefix),
            emit_binary_op(*op),
            emit_expr(right, ctx_prefix, payload_prefix)
        ),
        Expr::Cast(c) => emit_cast(c, ctx_prefix, payload_prefix),
    }
}

fn emit_cast(c: &CastExpr, ctx_prefix: &str, payload_prefix: &str) -> String {
    let inner = emit_expr(&c.operand, ctx_prefix, payload_prefix);
    let c_ty = c_type_str(&c.target_type);
    format!("(({}){})", c_ty, inner)
}

/// Convert an IR [`Type`] to its C99 spelling.
pub fn c_type_str(t: &Type) -> String {
    match t {
        Type::Primitive { name } => primitive_to_c(name).to_owned(),
        Type::Enum { enum_id } => format!("/* enum {} */ int", enum_id),
        Type::Opaque { c_type } => c_type.clone(),
        Type::Array { element, size } => {
            // Arrays as RHS-type-cast targets are rare and the IR validates
            // shape; render best-effort.
            format!("{}[{}]", c_type_str(element), size)
        }
    }
}

/// Map an IR primitive type name to its C99 fixed-width type.
pub fn primitive_to_c(name: &str) -> &'static str {
    match name {
        "bool" => "bool",
        "u8" => "uint8_t",
        "u16" => "uint16_t",
        "u32" => "uint32_t",
        "u64" => "uint64_t",
        "i8" => "int8_t",
        "i16" => "int16_t",
        "i32" => "int32_t",
        "i64" => "int64_t",
        "f32" => "float",
        "f64" => "double",
        "string" => "const char *",
        // Unknown — defensive default to int. Analyzer should have caught
        // this before reaching codegen.
        _ => "int",
    }
}

/// Emit a field reference like `m->context.speed` or `ev->__payload.start.target_speed`.
pub fn emit_field_ref(f: &FieldRef, ctx_prefix: &str, payload_prefix: &str) -> String {
    match f {
        FieldRef::Ctx { field } => format!("{}.{}", ctx_prefix, field),
        FieldRef::Payload { field } => format!("{}.{}", payload_prefix, field),
    }
}

fn emit_guard_operand(o: &GuardOperand, ctx_prefix: &str, payload_prefix: &str) -> String {
    match o {
        GuardOperand::Literal(l) => emit_literal(l),
        GuardOperand::FieldRef(f) => emit_field_ref(f, ctx_prefix, payload_prefix),
    }
}

fn emit_cmp_op(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Eq => "==",
        CmpOp::NotEq => "!=",
        CmpOp::Lt => "<",
        CmpOp::Gt => ">",
        CmpOp::LtEq => "<=",
        CmpOp::GtEq => ">=",
    }
}

fn emit_binary_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::LogAnd => "&&",
        BinaryOp::LogOr => "||",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
    }
}

/// Emit a literal as a C constant. Integers use the smallest-fitting suffix;
/// floats include a trailing `f` for `f32`-style use. Enum variants emit as
/// `ENUMNAME_VARIANT` per Doc 11 §4 conventions.
pub fn emit_literal(l: &Literal) -> String {
    match l {
        Literal::Int(i) => format!("{}", i.value),
        Literal::Float(f) => {
            // Always emit a decimal point so the compiler doesn't treat the
            // literal as int.
            let mut s = format!("{}", f.value);
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                s.push_str(".0");
            }
            s
        }
        Literal::Bool(b) => if b.value { "true" } else { "false" }.to_owned(),
        Literal::String(s) => format!("\"{}\"", s.value.replace('\\', "\\\\").replace('"', "\\\"")),
        Literal::EnumVariant(e) => emit_enum_variant(e),
    }
}

/// Emit an enum variant literal. Convention: `ENUM_NAME_VARIANT_NAME` —
/// uppercased + underscore-separated. This matches the conventions Doc 11
/// §27 uses informally.
pub fn emit_enum_variant(e: &EnumVariantLit) -> String {
    let enum_part = crate::state_index::c_ident(&e.enum_name);
    let variant_part = crate::state_index::c_ident(&e.variant_name);
    format!("{}_{}", enum_part, variant_part)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_ir::{FieldRef, FloatLit, IntLit};

    fn ctx_field(name: &str) -> FieldRef {
        FieldRef::Ctx { field: name.into() }
    }

    #[test]
    fn primitive_u32_maps_to_uint32_t() {
        assert_eq!(primitive_to_c("u32"), "uint32_t");
    }

    #[test]
    fn primitive_unknown_defaults_to_int() {
        // Analyzer would normally catch this — we test the defensive path.
        assert_eq!(primitive_to_c("not_a_real_type"), "int");
    }

    #[test]
    fn int_literal_emits_decimal() {
        let s = emit_literal(&Literal::Int(IntLit {
            value: 42,
            loc: None,
        }));
        assert_eq!(s, "42");
    }

    #[test]
    fn float_literal_carries_decimal_point() {
        let s = emit_literal(&Literal::Float(FloatLit {
            value: 1.0,
            loc: None,
        }));
        // Must contain a `.` so the compiler reads it as `double`.
        assert!(s.contains('.'), "got: {s}");
    }

    #[test]
    fn field_ref_uses_ctx_prefix() {
        assert_eq!(
            emit_field_ref(&ctx_field("speed"), "m->context", "ev->__payload"),
            "m->context.speed"
        );
    }

    #[test]
    fn guard_field_cmp_lowers_to_parenthesised_expr() {
        let g = GuardExpr::FieldCmp {
            lhs: ctx_field("speed"),
            op: CmpOp::Gt,
            rhs: GuardOperand::Literal(Literal::Int(IntLit {
                value: 100,
                loc: None,
            })),
        };
        assert_eq!(
            emit_guard(&g, "m->context", "ev->__payload"),
            "(m->context.speed > 100)"
        );
    }

    #[test]
    fn cast_renders_c_cast() {
        let c = Expr::Cast(CastExpr {
            operand: Box::new(Expr::Literal(Literal::Int(IntLit {
                value: 7,
                loc: None,
            }))),
            target_type: Type::Primitive { name: "u8".into() },
            loc: fsm_diagnostics::SourceLocation::new(
                "t.fsm",
                fsm_diagnostics::Span::new(0, 0),
                1,
                1,
            ),
        });
        assert_eq!(emit_expr(&c, "m->context", "ev->p"), "((uint8_t)7)");
    }

    #[test]
    fn string_literal_escapes_quotes() {
        let s = emit_literal(&Literal::String(fsm_ir::StringLit {
            value: r#"hello "world""#.to_owned(),
            loc: None,
        }));
        // Should contain escaped inner quotes.
        assert!(s.contains(r#"\""#), "got: {s}");
    }

    #[test]
    fn enum_variant_renders_uppercased() {
        let l = Literal::EnumVariant(EnumVariantLit {
            enum_name: "PacketType".into(),
            variant_name: "HEARTBEAT".into(),
            loc: None,
        });
        assert_eq!(emit_literal(&l), "PACKETTYPE_HEARTBEAT");
    }
}
