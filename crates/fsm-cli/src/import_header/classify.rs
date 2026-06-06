//! Stages 3+4+6 of the header-import pipeline:
//!
//! 3. **Classify** a `;`-free chunk: is it a function prototype, a `typedef`,
//!    a `struct`/`enum`/`union` body, a function-pointer / array declarator?
//! 4. **Parameter parsing** — split, classify each param, map its type.
//! 6. **Finalise** — convert a successfully-parsed [`RawFn`] into an
//!    [`ExternObject`] with a synthesized [`SourceLocation`] keyed on the
//!    origin header.

use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{ExternObject, Param, Type};

use super::types::map_c_type;
use super::util::{
    condense_ws, is_c_ident, is_type_keyword, matching_paren, split_params, strip_attributes,
};

pub(super) enum Decl {
    Function(RawFn),
    Skip(String),
}

pub(super) struct RawFn {
    pub(super) ret: Type,
    pub(super) name: String,
    pub(super) params: Vec<Param>,
}

/// Decide what a single (comment-stripped, `;`-free) chunk is.
pub(super) fn classify(chunk: &str) -> Decl {
    // Definitions / type machinery we never model.
    if chunk.contains('{') {
        // A trailing `{` on a function declarator is a *definition*; an
        // inline definition in a header is still a real symbol, but its
        // body may reference things we can't see — and a prototype is all
        // codegen needs. We model the prototype and drop the body.
        if let Some(proto) = chunk.split('{').next() {
            if looks_like_fn(proto) {
                return classify_prototype(proto.trim());
            }
        }
        return Decl::Skip(
            "struct/enum/union body or block — not a function \
                           prototype"
                .into(),
        );
    }
    let first = chunk.split_whitespace().next().unwrap_or("");
    if first == "typedef" {
        return Decl::Skip("typedef — type aliases are not linkable symbols".into());
    }
    if matches!(first, "struct" | "enum" | "union") && !chunk.contains('(') {
        return Decl::Skip(format!("{first} forward-declaration / variable"));
    }
    if !chunk.contains('(') {
        return Decl::Skip("not a function declaration (no parameter list)".into());
    }
    classify_prototype(chunk)
}

/// Does this fragment plausibly contain a function declarator `name ( … )`?
fn looks_like_fn(s: &str) -> bool {
    if let Some(open) = s.find('(') {
        let head = &s[..open];
        return head
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|t| !t.is_empty())
            .next_back()
            .map(is_c_ident)
            .unwrap_or(false);
    }
    false
}

/// Parse `[extern] [static] [inline] RET NAME ( PARAMS )`.
fn classify_prototype(proto: &str) -> Decl {
    let proto = strip_attributes(proto);
    let open = match proto.find('(') {
        Some(p) => p,
        None => return Decl::Skip("no parameter list".into()),
    };
    let close = match matching_paren(&proto, open) {
        Some(c) => c,
        None => return Decl::Skip("unbalanced parentheses".into()),
    };
    // A function pointer / array follows the param list with more tokens
    // (`(*)(...)`, `[]`). Anything other than trailing whitespace after the
    // outermost `)` means a declarator shape we don't model.
    let tail = proto[close + 1..].trim();
    if !tail.is_empty() {
        return Decl::Skip(
            "complex declarator (function pointer / array / trailing tokens) \
             — declare it manually as an `extern` if needed"
                .into(),
        );
    }

    let head = proto[..open].trim();
    let params_src = &proto[open + 1..close];

    // Function pointer return / nested declarator: a `(` or `*` immediately
    // wrapping the name inside `head` is too complex — skip conservatively.
    if head.contains('(') {
        return Decl::Skip("function-pointer return type — not modelled".into());
    }

    // The function name is the last identifier token in `head`; everything
    // before it is the return type (including `*`, `const`, `struct X`).
    let (ret_src, name) = match split_ret_and_name(head) {
        Some(v) => v,
        None => return Decl::Skip("could not separate return type from function name".into()),
    };
    if !is_c_ident(&name) {
        return Decl::Skip(format!("`{name}` is not a valid C identifier"));
    }

    let ret = match map_c_type(ret_src) {
        Some(t) => t,
        None => {
            return Decl::Skip(format!(
                "unmappable return type `{}` for `{name}`",
                condense_ws(ret_src)
            ))
        }
    };

    let params = match parse_params(params_src) {
        Ok(p) => p,
        Err(why) => return Decl::Skip(format!("{why} (in `{name}`)")),
    };

    Decl::Function(RawFn { ret, name, params })
}

/// Split a declarator head like `const struct Foo * device_open` into
/// (`"const struct Foo *"`, `"device_open"`).
fn split_ret_and_name(head: &str) -> Option<(&str, String)> {
    // Walk from the end: skip trailing whitespace, then take a run of
    // identifier characters as the name. The byte right before that run
    // (if any) must be whitespace or `*` — otherwise the "name" is glued
    // to a token we mis-split (defensive: skip).
    let bytes = head.as_bytes();
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 {
        let b = bytes[start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' {
            start -= 1;
        } else {
            break;
        }
    }
    if start == end {
        return None;
    }
    let name = head[start..end].to_string();
    let ret = head[..start].trim();
    if ret.is_empty() {
        // `void` etc. only — no return type means this is not a function
        // declarator we can model (e.g. a bare `int;`).
        return None;
    }
    Some((ret, name))
}

fn parse_params(src: &str) -> Result<Vec<Param>, String> {
    let t = src.trim();
    if t.is_empty() || t == "void" {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for (idx, raw) in split_params(t).into_iter().enumerate() {
        let p = raw.trim();
        if p == "..." {
            // Variadic: an FSM action can only ever pass the fixed prefix,
            // so we keep the fixed params already collected and stop. The
            // caller treats this as success (the note about dropping the
            // ellipsis is implicit in the documented subset).
            break;
        }
        if p.is_empty() {
            return Err("empty parameter".into());
        }
        if p.contains('(') || p.contains('[') {
            return Err("function-pointer or array parameter".into());
        }
        // A param is `TYPE [name]`. Reuse the ret/name splitter: if the
        // last token is a bare identifier *and* is preceded by a type, that
        // token is the name; otherwise the whole thing is an unnamed type.
        let (ty_src, name) = match split_param_type_and_name(p) {
            Some((ty, nm)) => (ty, Some(nm)),
            None => (p, None),
        };
        let ty = match map_c_type(ty_src) {
            Some(t) => t,
            None => {
                return Err(format!(
                    "unmappable parameter type `{}`",
                    condense_ws(ty_src)
                ))
            }
        };
        out.push(Param {
            name: name.unwrap_or_else(|| format!("arg{idx}")),
            ty,
            id: None,
            loc: None,
        });
    }
    Ok(out)
}

/// `const uint8_t * buf` → (`"const uint8_t *"`, `"buf"`). Returns `None`
/// when the fragment is type-only (`uint8_t`, `struct X *`).
fn split_param_type_and_name(p: &str) -> Option<(&str, String)> {
    let bytes = p.as_bytes();
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 {
        let b = bytes[start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' {
            start -= 1;
        } else {
            break;
        }
    }
    if start == end || start == 0 {
        // All-identifier (e.g. `int`) → type-only, no name.
        return None;
    }
    let last = &p[start..end];
    // If the last token is itself a type keyword, there is no name.
    if is_type_keyword(last) {
        return None;
    }
    let ty = p[..start].trim();
    if ty.is_empty() {
        return None;
    }
    Some((ty, last.to_string()))
}

/// If any parameter or the return type is [`Type::Opaque`] (pointer /
/// `struct` / `size_t` / unknown typedef), return a precise skip reason.
/// `None` means the whole signature is primitive-only and safe to import.
///
/// See the call site in [`super::parse_header`] for *why* opaque-signature
/// functions cannot be imported (the DSL extern lowering path drops opaque
/// param/return types — a pre-existing pipeline limitation, not this
/// feature's bug).
pub(super) fn opaque_in_signature(f: &RawFn) -> Option<String> {
    if let Type::Opaque { c_type } = &f.ret {
        if c_type != "void" {
            return Some(format!(
                "return type `{c_type}` is a pointer/struct/opaque type — \
                 the DSL `extern` lowering does not model opaque types \
                 (params/return would be lost); declare `{}` manually as an \
                 `extern` in the .fsm if your guards/actions need it",
                f.name
            ));
        }
    }
    for p in &f.params {
        if let Type::Opaque { c_type } = &p.ty {
            return Some(format!(
                "parameter `{}: {c_type}` is a pointer/struct/opaque type — \
                 the DSL `extern` lowering does not model opaque types, and \
                 a DSL guard/action cannot construct such an argument \
                 anyway; declare `{}` manually if needed",
                p.name, f.name
            ));
        }
    }
    None
}

/// Turn a recognised raw function into an [`ExternObject`].
///
/// `pure` is `false`: a header tells us a *signature*, never whether the
/// function is side-effect-free. Treating every imported extern as
/// non-pure is the conservative choice — the analyzer only lets `pure`
/// externs appear in guards, so a falsely-pure import could silently admit
/// a side-effecting call into a guard. If the user wants an imported
/// function usable in a guard they declare it `pure extern` in the `.fsm`
/// (the DSL declaration wins on conflict — see `cmd::generate`).
///
/// A `void`-returning extern lowers to `return_type: None`, exactly as the
/// analyzer lowers a DSL extern with no `: type` clause, so codegen's
/// `emit_bare_extern_decl` prints `void name(...)` identically.
pub(super) fn finalize(f: RawFn, header_label: &str) -> ExternObject {
    let loc = SourceLocation {
        file: header_label.to_string(),
        span: Span::new(0, 0),
        line: 0,
        column: 0,
    };
    let return_type = match f.ret {
        Type::Opaque { ref c_type } if c_type == "void" => None,
        other => Some(other),
    };
    ExternObject {
        id: format!("ex-import-{}", f.name),
        stable_id: format!("H:{header_label}:extern:{}", f.name),
        name: f.name,
        pure: false,
        params: f.params,
        return_type,
        loc,
    }
}
