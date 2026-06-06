//! DSL syntax emission — render an [`ExternObject`] back as a DSL
//! `extern` declaration line. Output must round-trip through
//! [`fsm_parser::parse`] (the test
//! `render_extern_decl_round_trips_through_the_dsl_parser` is the
//! load-bearing assertion).

use fsm_ir::{ExternObject, Type};

use super::util::is_c_ident;

/// Render one [`ExternObject`] as a DSL `extern` declaration.
///
/// `pure` is always `false` for imports (a header can't tell us purity), so
/// the `pure` keyword is never emitted — matching `finalize`. A `void`
/// return (`return_type: None`) omits the `: type` clause exactly as a
/// hand-written `extern foo()` would.
pub(super) fn render_extern_decl(ext: &ExternObject) -> String {
    let params = ext
        .params
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{} {}", type_to_dsl(&p.ty), dsl_safe_param_name(&p.name, i)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut s = format!("extern {}({})", ext.name, params);
    if let Some(rt) = &ext.return_type {
        s.push_str(&format!(" : {}", type_to_dsl(rt)));
    }
    s
}

/// A C parameter name can collide with an FSM DSL keyword (`on`, `state`,
/// `to`, `event`, …) or be empty/synthesised; either would make the
/// synthesized `extern` line fail to parse. The DSL param name is purely
/// cosmetic for codegen (linkage is by position/type — C prototypes ignore
/// parameter names), so we rename a colliding/invalid name to a safe,
/// deterministic form rather than fail the import. A non-colliding name
/// (the common case: `rpm`, `channel`, `code`, `force`) is kept verbatim so
/// the generated prototype stays readable.
pub(super) fn dsl_safe_param_name(name: &str, idx: usize) -> String {
    if name.is_empty() || !is_c_ident(name) {
        return format!("arg{idx}");
    }
    if is_dsl_keyword(name) {
        // No DSL keyword ends in `_`, so a single underscore suffix is
        // guaranteed collision-free and still echoes the original name.
        return format!("{name}_");
    }
    name.to_string()
}

/// The FSM DSL reserved-word set (must mirror the lexer keyword table). A
/// parameter name equal to one of these cannot be used verbatim in a
/// synthesized `extern` declaration.
fn is_dsl_keyword(s: &str) -> bool {
    matches!(
        s,
        "after"
            | "as"
            | "bool"
            | "cancel"
            | "choice"
            | "composite"
            | "const"
            | "context"
            | "deep_history"
            | "defer"
            | "done"
            | "else"
            | "enum"
            | "every"
            | "export"
            | "extern"
            | "false"
            | "feature"
            | "final"
            | "fork"
            | "import"
            | "initial"
            | "is"
            | "join"
            | "junction"
            | "language"
            | "machine"
            | "ms"
            | "on"
            | "opaque"
            | "parallel"
            | "priority"
            | "promote"
            | "pure"
            | "raise"
            | "region"
            | "schedule"
            | "send"
            | "shallow_history"
            | "state"
            | "submachine"
            | "target"
            | "to"
            | "true"
            // primitive type keywords are also reserved as identifiers
            | "u8" | "u16" | "u32" | "u64"
            | "i8" | "i16" | "i32" | "i64"
            | "f32" | "f64"
    )
}

/// IR [`Type`] → DSL type syntax. The extractor only ever produces
/// `Primitive` (whose names — `u8`, `i16`, `bool`, `f32`, … — are the DSL
/// type keywords verbatim) or `Opaque` (→ `opaque "C spelling"`). The
/// opaque spelling is guaranteed by `super::types::map_c_type` /
/// `super::util::normalise_pointer` to contain only `[A-Za-z0-9_ *]`, so
/// it satisfies the parser's `opaque_type_validator` (security G-02) and
/// never needs escaping.
fn type_to_dsl(t: &Type) -> String {
    match t {
        Type::Primitive { name } => name.clone(),
        Type::Opaque { c_type } => format!("opaque \"{c_type}\""),
        // The extractor cannot produce these; render defensively rather
        // than panic so a future caller can't blow up codegen.
        Type::Enum { enum_id } => enum_id.clone(),
        Type::Array { element, size } => format!("{} /*[{size}]*/", type_to_dsl(element)),
    }
}
