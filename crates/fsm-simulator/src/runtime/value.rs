//! Runtime [`Value`] tagged union — covers every context field type the
//! IR can model (Doc 09 §10) plus a payload-friendly `Enum` variant for
//! `enum_variant` literals (Doc 09 §9, Doc 00 §7.4 item 7).
//!
//! The simulator stores all context fields as `Value` regardless of their
//! declared IR type; the variants double as the operand domain for the
//! expression / guard evaluator (`crate::eval`).
//!
//! Arithmetic and cast helpers live here so that statement evaluation can
//! delegate operator semantics in one place. The intent is to mirror the C99
//! widening rules emitted by `fsm-codegen-c` so the equivalence gate produces
//! identical results.

use serde::{Deserialize, Serialize};

/// All concrete runtime values the interpreter can hold.
///
/// Each variant maps to one IR primitive type (Doc 09 §10). The `Enum`
/// variant carries both the qualifying enum name and the variant tag — the
/// expression evaluator uses string equality for comparisons; numeric widths
/// are honoured for integer types so overflow matches the generated C code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Value {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    String(String),
    /// Qualified enum literal `EnumName.Variant` — Doc 00 §7.4 item 7.
    Enum(String, String),
}

impl Value {
    /// Name of the IR primitive type this value represents, used to populate
    /// the `type` field of a `ContextSnapshot` (Doc 13 §11).
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Bool(_) => "bool",
            Value::I8(_) => "i8",
            Value::I16(_) => "i16",
            Value::I32(_) => "i32",
            Value::I64(_) => "i64",
            Value::U8(_) => "u8",
            Value::U16(_) => "u16",
            Value::U32(_) => "u32",
            Value::U64(_) => "u64",
            Value::F32(_) => "f32",
            Value::F64(_) => "f64",
            Value::String(_) => "string",
            Value::Enum(_, _) => "enum",
        }
    }

    /// Truthiness coercion used by `if` / `while` conditions and `!`. Booleans
    /// are passed through; numerics are non-zero-is-true; strings are
    /// non-empty-is-true. Mirrors the equivalent C99 contraction.
    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::I8(v) => *v != 0,
            Value::I16(v) => *v != 0,
            Value::I32(v) => *v != 0,
            Value::I64(v) => *v != 0,
            Value::U8(v) => *v != 0,
            Value::U16(v) => *v != 0,
            Value::U32(v) => *v != 0,
            Value::U64(v) => *v != 0,
            Value::F32(v) => *v != 0.0,
            Value::F64(v) => *v != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Enum(_, _) => true,
        }
    }

    /// Widen to `i64` for signed arithmetic. Unsigned values fit because we
    /// only model up to `u32` losslessly; `u64` falls through as a saturated
    /// `i64::MAX` to flag the corner case in the eval layer.
    pub fn to_i64(&self) -> Option<i64> {
        match self {
            Value::Bool(b) => Some(if *b { 1 } else { 0 }),
            Value::I8(v) => Some(*v as i64),
            Value::I16(v) => Some(*v as i64),
            Value::I32(v) => Some(*v as i64),
            Value::I64(v) => Some(*v),
            Value::U8(v) => Some(*v as i64),
            Value::U16(v) => Some(*v as i64),
            Value::U32(v) => Some(*v as i64),
            Value::U64(v) => i64::try_from(*v).ok(),
            Value::F32(v) => Some(*v as i64),
            Value::F64(v) => Some(*v as i64),
            Value::String(_) | Value::Enum(_, _) => None,
        }
    }

    /// Widen to `f64` for floating arithmetic.
    pub fn to_f64(&self) -> Option<f64> {
        match self {
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::I8(v) => Some(*v as f64),
            Value::I16(v) => Some(*v as f64),
            Value::I32(v) => Some(*v as f64),
            Value::I64(v) => Some(*v as f64),
            Value::U8(v) => Some(*v as f64),
            Value::U16(v) => Some(*v as f64),
            Value::U32(v) => Some(*v as f64),
            Value::U64(v) => Some(*v as f64),
            Value::F32(v) => Some(*v as f64),
            Value::F64(v) => Some(*v),
            Value::String(_) | Value::Enum(_, _) => None,
        }
    }

    /// `true` when both values are floating-point — selects float arithmetic
    /// over integer arithmetic in the evaluator.
    pub fn either_float(a: &Value, b: &Value) -> bool {
        matches!(a, Value::F32(_) | Value::F64(_)) || matches!(b, Value::F32(_) | Value::F64(_))
    }

    /// Build the default zero value for a primitive type name as found on
    /// `Type::Primitive { name }`. Unknown names fall back to `I32(0)` so the
    /// interpreter remains forgiving when given a partial IR.
    pub fn default_for_primitive(name: &str) -> Value {
        match name {
            "bool" => Value::Bool(false),
            "i8" => Value::I8(0),
            "i16" => Value::I16(0),
            "i32" => Value::I32(0),
            "i64" => Value::I64(0),
            "u8" => Value::U8(0),
            "u16" => Value::U16(0),
            "u32" => Value::U32(0),
            "u64" => Value::U64(0),
            "f32" => Value::F32(0.0),
            "f64" => Value::F64(0.0),
            "string" => Value::String(String::new()),
            _ => Value::I32(0),
        }
    }

    /// `expr as T` (Doc 00 §7.4 item 5) — truncate / widen / float-int
    /// conversion. Returns `None` for casts that cannot be expressed (e.g.
    /// `String as i32`).
    pub fn cast_to(&self, target: &str) -> Option<Value> {
        // Floats stay in float space when targeted; otherwise convert via i64.
        let as_i = self.to_i64();
        let as_f = self.to_f64();
        Some(match target {
            "bool" => Value::Bool(self.as_bool()),
            "i8" => Value::I8(as_i? as i8),
            "i16" => Value::I16(as_i? as i16),
            "i32" => Value::I32(as_i? as i32),
            "i64" => Value::I64(as_i?),
            "u8" => Value::U8(as_i? as u8),
            "u16" => Value::U16(as_i? as u16),
            "u32" => Value::U32(as_i? as u32),
            "u64" => Value::U64(as_i? as u64),
            "f32" => Value::F32(as_f? as f32),
            "f64" => Value::F64(as_f?),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cast_round_trip_signed() {
        let v = Value::I32(257);
        // i32(257) → u8 should truncate to 1.
        assert_eq!(v.cast_to("u8"), Some(Value::U8(1)));
    }

    #[test]
    fn cast_to_bool_uses_truthiness() {
        assert_eq!(Value::I32(0).cast_to("bool"), Some(Value::Bool(false)));
        assert_eq!(Value::I32(7).cast_to("bool"), Some(Value::Bool(true)));
        assert_eq!(
            Value::String(String::new()).cast_to("bool"),
            Some(Value::Bool(false))
        );
    }

    #[test]
    fn either_float_detects_mixed_arithmetic() {
        assert!(Value::either_float(&Value::F32(1.0), &Value::I32(1)));
        assert!(!Value::either_float(&Value::I32(1), &Value::I32(1)));
    }

    #[test]
    fn default_for_unknown_primitive_is_i32_zero() {
        assert_eq!(Value::default_for_primitive("widget"), Value::I32(0));
    }

    #[test]
    fn type_name_strings_match_doc_09() {
        assert_eq!(Value::U16(0).type_name(), "u16");
        assert_eq!(Value::Bool(false).type_name(), "bool");
        assert_eq!(Value::F32(0.0).type_name(), "f32");
    }
}
