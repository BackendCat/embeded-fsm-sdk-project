//! C type spelling → IR [`Type`]. Scalars map to IR primitives so codegen
//! emits the natural fixed-width C type. Pointers / `struct` / `enum` /
//! `union` / unknown-but-identifier types map to [`Type::Opaque`] carrying
//! a normalised C spelling — this is exactly how a DSL `opaque "T"` extern
//! param behaves, so downstream is identical. Genuinely unmappable
//! spellings return `None` (the caller skips that function with a note
//! rather than emit a broken extern).

use fsm_ir::Type;

use super::util::{condense_ws, is_c_ident, is_reserved_nontype, normalise_pointer};

pub(super) fn map_c_type(raw: &str) -> Option<Type> {
    let norm = condense_ws(raw);
    // Pointer → opaque verbatim (one or more `*`). Normalise spacing to a
    // single `T *` form so the emitted C is clean.
    if norm.contains('*') {
        let cleaned = normalise_pointer(&norm);
        return Some(Type::Opaque { c_type: cleaned });
    }
    // Drop qualifiers that don't change the IR mapping.
    let toks: Vec<&str> = norm
        .split_whitespace()
        .filter(|t| !matches!(*t, "const" | "volatile" | "register" | "extern" | "static"))
        .collect();
    if toks.is_empty() {
        return None;
    }
    let joined = toks.join(" ");
    let prim = match joined.as_str() {
        "void" => {
            return Some(Type::Opaque {
                c_type: "void".into(),
            })
        }
        "bool" | "_Bool" => "bool",
        "char" | "signed char" => "i8",
        "unsigned char" => "u8",
        "short" | "short int" | "signed short" | "signed short int" => "i16",
        "unsigned short" | "unsigned short int" => "u16",
        "int" | "signed" | "signed int" => "i32",
        "unsigned" | "unsigned int" => "u32",
        "long" | "long int" | "signed long" | "signed long int" => "i32",
        "unsigned long" | "unsigned long int" => "u32",
        "long long" | "long long int" | "signed long long" | "signed long long int" => "i64",
        "unsigned long long" | "unsigned long long int" => "u64",
        "float" => "f32",
        "double" | "long double" => "f64",
        // Fixed-width <stdint.h>.
        "int8_t" => "i8",
        "int16_t" => "i16",
        "int32_t" => "i32",
        "int64_t" => "i64",
        "uint8_t" => "u8",
        "uint16_t" => "u16",
        "uint32_t" => "u32",
        "uint64_t" => "u64",
        // Address-width integers: keep them honest by passing the C
        // spelling through opaquely (their width is platform-dependent;
        // forcing a fixed IR primitive would be a silent truncation lie).
        "size_t" | "ssize_t" | "ptrdiff_t" | "uintptr_t" | "intptr_t" => {
            return Some(Type::Opaque { c_type: joined })
        }
        // `struct X` / `enum X` / `union X` (no pointer) — pass verbatim.
        other => {
            if other.starts_with("struct ")
                || other.starts_with("enum ")
                || other.starts_with("union ")
            {
                return Some(Type::Opaque {
                    c_type: other.to_string(),
                });
            }
            // A lone identifier that isn't a known keyword: most likely a
            // project typedef (`my_handle_t`). We can't know its width, but
            // an opaque pass-through lets the user's own header define it —
            // identical to a DSL `opaque "my_handle_t"`.
            if is_c_ident(other) && !is_reserved_nontype(other) {
                return Some(Type::Opaque {
                    c_type: other.to_string(),
                });
            }
            return None;
        }
    };
    Some(Type::Primitive { name: prim.into() })
}
