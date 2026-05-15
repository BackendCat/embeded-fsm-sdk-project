//! `--import-header` — derive `extern` declarations from a C header.
//!
//! ## Why this exists
//!
//! Users with an existing embedded C codebase (HAL, drivers) otherwise have
//! to hand-transcribe every function they want to call from a guard/action
//! into a DSL `extern` declaration. This module reads a `.h` file, extracts
//! the function declarations it can confidently model into
//! [`fsm_ir::ExternObject`]s, and renders them back as DSL `extern` text
//! ([`ImportedHeader::to_dsl_externs`]) which `cmd::generate` splices into
//! the `.fsm` source *before parsing* — so an imported extern flows through
//! the entire normal pipeline and is indistinguishable downstream from one
//! declared in the `.fsm` (ROADMAP v1.1 W5, Doc 18). See
//! [`ImportedHeader::to_dsl_externs`] for why source-synthesis (not
//! post-IR injection) is the correct injection point.
//!
//! ## Design constraints (Doc 23 §4 dependency rules)
//!
//! C-header parsing is a CLI-input convenience, so it lives in `fsm-cli`, NOT
//! a core pipeline crate. We deliberately do **not** shell out to a real C
//! compiler or link libclang: that would add a heavy, non-deterministic
//! system dependency to a compile-time tool. Instead this is a focused,
//! hand-rolled tokenizer + declaration matcher covering the common forms in
//! realistic embedded headers.
//!
//! ## Resilience over completeness
//!
//! A header line we cannot *confidently* turn into an extern is **skipped
//! with an `fsm-W`-level note**, never a hard error and never a misparse. A
//! gnarly real-world header full of macros, typedefs, struct bodies and
//! attributes must not crash the tool or produce a broken `extern`. The
//! supported subset is documented in Doc 18 §5 `fsm generate`.
//!
//! ## Supported C subset
//!
//! - Function declarations: `ret name(params);` and `extern ret name(params);`
//! - Return / parameter base types: `void`, `bool`/`_Bool`, `char`,
//!   `signed/unsigned char/short/int/long/long long`, `float`, `double`,
//!   and the fixed-width `<stdint.h>` set (`uint8_t` … `int64_t`,
//!   `size_t`, `uintptr_t`/`intptr_t`).
//! - One level of pointer (`T *`, `const T *`) → `opaque "T *"`.
//! - `struct X` / `struct X *` / `enum X` / `union X` → `opaque` verbatim.
//! - `const` qualifiers (ignored for IR type purposes; preserved in opaque
//!   spellings).
//! - `void` parameter list (`f(void)`) → zero params.
//! - Unnamed parameters (`int f(int);`) → synthesised `arg0`, `arg1`, ….
//! - Variadic `...` → the variadic marker is dropped; the fixed prefix
//!   params are still imported (an FSM action can only pass the fixed
//!   args anyway; see skip note).
//!
//! Everything else — `#define`/`#if` and other preprocessor lines,
//! `typedef`s, standalone `struct`/`enum`/`union` *definitions* (with a
//! body), `__attribute__((...))`, `__declspec`, K&R-style declarations,
//! function-pointer parameters, array parameters, multi-declarator lines —
//! is skipped with a note. Comments (`//` and `/* */`) and string/char
//! literals are stripped before matching so a `;` inside them never
//! terminates a phantom declaration.

use std::path::Path;

use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{ExternObject, Param, Type};

/// One skipped construct, surfaced to the user as an `fsm-W`-level note so
/// they know *why* a function they expected did not become an extern.
#[derive(Debug, Clone, PartialEq)]
pub struct SkipNote {
    /// The (trimmed, comment-stripped) source fragment we declined to model.
    pub fragment: String,
    /// Human-readable reason.
    pub reason: String,
}

/// Result of importing one header: the externs we could model + the notes
/// for everything we deliberately skipped.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ImportedHeader {
    pub externs: Vec<ExternObject>,
    pub skipped: Vec<SkipNote>,
}

/// Parse a C header file's text into importable externs.
///
/// `header_label` is used only for the synthesized [`SourceLocation::file`]
/// so diagnostics/`emit` can attribute an imported extern to its origin
/// header rather than the `.fsm`.
pub fn parse_header(src: &str, header_label: &str) -> ImportedHeader {
    let mut out = ImportedHeader::default();
    let stripped = strip_comments_and_preproc(src, &mut out.skipped);

    // Split on top-level `;` and `}` so each candidate is a single
    // declaration. Brace depth is tracked so a struct/enum body's inner
    // `;`s don't fragment it — the whole `struct {...} ;` lands as one
    // chunk and is then recognised as a definition we skip.
    for chunk in split_top_level_decls(&stripped) {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        match classify(trimmed) {
            Decl::Function(f) => {
                // Pre-existing pipeline reality (verified, NOT this
                // feature's regression): the DSL `extern` lowering path
                // (`fsm-analyzer` `lower_type_ref`) and the extern-param
                // grammar (`can_start_type`) only model the primitive type
                // keywords — an `opaque "T"` param or return on an extern
                // is silently dropped even when hand-written in a `.fsm`.
                // Emitting a synthesized extern with a pointer/struct in
                // its signature would therefore produce a *wrong* prototype
                // (params silently lost), so we SKIP such a function with a
                // precise note instead. This is correct in practice: a DSL
                // guard/action has no way to construct a `struct *`/pointer
                // argument anyway, so a pointer-param HAL function is not
                // callable from FSM logic regardless. Scalar-only functions
                // — the overwhelming majority used from guards/actions —
                // import perfectly. (Resilience > completeness; documented
                // in Doc 18 §5. Reported as a STOP-worthy pipeline gap.)
                if let Some(why) = opaque_in_signature(&f) {
                    out.skipped.push(SkipNote {
                        fragment: condense_ws(trimmed),
                        reason: why,
                    });
                } else {
                    out.externs.push(finalize(f, header_label));
                }
            }
            Decl::Skip(reason) => out.skipped.push(SkipNote {
                fragment: condense_ws(trimmed),
                reason,
            }),
        }
    }
    out
}

/// Convenience: read + parse a header file from disk.
pub fn parse_header_file(path: &Path) -> std::io::Result<ImportedHeader> {
    let src = std::fs::read_to_string(path)?;
    let label = path.to_string_lossy().into_owned();
    Ok(parse_header(&src, &label))
}

impl ImportedHeader {
    /// Render the imported externs as DSL `extern` declaration lines.
    ///
    /// ## Why source synthesis (the injection-point decision)
    ///
    /// Extern *name resolution* (`FSM-E0102` "unknown extern") runs in the
    /// analyzer's `checks::name_resolution`, which queries the **symbol
    /// table built from the AST** — not the IR. Injecting `ExternObject`s
    /// into the post-analysis IR is therefore too late: the analyzer would
    /// already have rejected a guard/action that calls an imported function.
    ///
    /// To make an imported extern *truly* indistinguishable from a DSL one
    /// (the brief's hard requirement) it must flow through the **entire**
    /// normal pipeline — parser → symbol table → name resolution → lowering
    /// → codegen. The faithful, zero-core-change way to do that from
    /// `fsm-cli` is to emit DSL `extern` text and prepend it to the source
    /// before `parse`. The synthesized lines re-parse into exactly the same
    /// AST/IR shape a hand-written `extern` would, so every downstream stage
    /// (including E0102) treats them identically. No lexer/parser/analyzer/
    /// ir/codegen logic is touched — only `fsm-cli` string assembly.
    ///
    /// `skip_names` are extern names already declared in the `.fsm` source;
    /// they are NOT re-emitted (the DSL declaration wins, and re-emitting
    /// would trip the analyzer's own duplicate-extern check `FSM-E0024`).
    /// Returns the rendered block plus the names that were suppressed for
    /// being DSL-shadowed (so `cmd::generate` can note the conflict once).
    pub fn to_dsl_externs(&self, skip_names: &[String]) -> (String, Vec<String>) {
        let mut block = String::new();
        let mut shadowed = Vec::new();
        for ext in &self.externs {
            if skip_names.iter().any(|n| n == &ext.name) {
                shadowed.push(ext.name.clone());
                continue;
            }
            block.push_str(&render_extern_decl(ext));
            block.push('\n');
        }
        (block, shadowed)
    }
}

/// Render one [`ExternObject`] as a DSL `extern` declaration.
///
/// `pure` is always `false` for imports (a header can't tell us purity), so
/// the `pure` keyword is never emitted — matching `finalize`. A `void`
/// return (`return_type: None`) omits the `: type` clause exactly as a
/// hand-written `extern foo()` would.
fn render_extern_decl(ext: &ExternObject) -> String {
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
fn dsl_safe_param_name(name: &str, idx: usize) -> String {
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
/// opaque spelling is guaranteed by `map_c_type`/`normalise_pointer` to
/// contain only `[A-Za-z0-9_ *]`, so it satisfies the parser's
/// `opaque_type_validator` (security G-02) and never needs escaping.
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

// ---------------------------------------------------------------------------
// Stage 1 — strip comments, string/char literals, and preprocessor lines.
// ---------------------------------------------------------------------------

/// Remove `//` + `/* */` comments and string/char literals (replaced by a
/// space so token boundaries survive), and drop preprocessor directives
/// (any line whose first non-space char is `#`, honouring `\`-continuation).
/// Preprocessor lines are noted as skipped only when they look like a macro
/// that *might* have been a function the user expected (`#define X(...)`),
/// keeping the note list signal-rich rather than spamming every `#include`.
fn strip_comments_and_preproc(src: &str, skipped: &mut Vec<SkipNote>) -> String {
    // Pass A: comment + literal blanking (character state machine).
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    #[derive(PartialEq)]
    enum S {
        Code,
        Line,
        Block,
        Str,
        Chr,
    }
    let mut st = S::Code;
    while i < bytes.len() {
        let c = bytes[i];
        let n = bytes.get(i + 1).copied().unwrap_or(0);
        match st {
            S::Code => {
                if c == b'/' && n == b'/' {
                    st = S::Line;
                    i += 2;
                } else if c == b'/' && n == b'*' {
                    st = S::Block;
                    out.push(' ');
                    i += 2;
                } else if c == b'"' {
                    st = S::Str;
                    out.push(' ');
                    i += 1;
                } else if c == b'\'' {
                    st = S::Chr;
                    out.push(' ');
                    i += 1;
                } else {
                    out.push(c as char);
                    i += 1;
                }
            }
            S::Line => {
                if c == b'\n' {
                    st = S::Code;
                    out.push('\n');
                }
                i += 1;
            }
            S::Block => {
                if c == b'*' && n == b'/' {
                    st = S::Code;
                    out.push(' ');
                    i += 2;
                } else {
                    if c == b'\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            S::Str => {
                if c == b'\\' {
                    i += 2;
                } else if c == b'"' {
                    st = S::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            S::Chr => {
                if c == b'\\' {
                    i += 2;
                } else if c == b'\'' {
                    st = S::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
        }
    }

    // Pass B: drop preprocessor lines (with backslash continuation).
    let mut result = String::with_capacity(out.len());
    let mut cont_pp = false;
    for line in out.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let is_pp_start = body.trim_start().starts_with('#');
        if cont_pp || is_pp_start {
            // Note function-like macros — a `#define FOO(x) ...` is the most
            // common "why didn't my function import" surprise.
            let t = body.trim_start();
            if is_pp_start {
                if let Some(rest) = t.strip_prefix('#') {
                    let rest = rest.trim_start();
                    if let Some(def) = rest.strip_prefix("define") {
                        let def = def.trim_start();
                        if def.contains('(')
                            && def
                                .split('(')
                                .next()
                                .map(|n| is_c_ident(n.trim()))
                                .unwrap_or(false)
                        {
                            skipped.push(SkipNote {
                                fragment: condense_ws(body.trim()),
                                reason: "function-like macro (preprocessor) — \
                                         not a linkable symbol; declare it as \
                                         an `extern` in the .fsm if you need it"
                                    .into(),
                            });
                        }
                    }
                }
            }
            cont_pp = body.trim_end().ends_with('\\');
            result.push('\n'); // keep line numbering roughly aligned
        } else {
            result.push_str(body);
            result.push('\n');
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Stage 2 — split into top-level declaration chunks.
// ---------------------------------------------------------------------------

/// Yield each top-level declaration (terminated by `;` or a closing `}` at
/// brace-depth 0). Parens and brackets are depth-tracked too so a `;`
/// inside a (hypothetical) initialiser or a `,` does not split mid-decl.
fn split_top_level_decls(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut brace = 0i32;
    let mut paren = 0i32;
    for ch in s.chars() {
        match ch {
            '{' => {
                brace += 1;
                cur.push(ch);
            }
            '}' => {
                brace -= 1;
                cur.push(ch);
                if brace <= 0 && paren == 0 {
                    out.push(std::mem::take(&mut cur));
                    brace = 0;
                }
            }
            '(' => {
                paren += 1;
                cur.push(ch);
            }
            ')' => {
                paren -= 1;
                cur.push(ch);
            }
            ';' if brace == 0 && paren == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

// ---------------------------------------------------------------------------
// Stage 3 — classify one declaration chunk.
// ---------------------------------------------------------------------------

enum Decl {
    Function(RawFn),
    Skip(String),
}

struct RawFn {
    ret: Type,
    name: String,
    params: Vec<Param>,
}

/// Decide what a single (comment-stripped, `;`-free) chunk is.
fn classify(chunk: &str) -> Decl {
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

// ---------------------------------------------------------------------------
// Stage 4 — parameter list.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Stage 5 — C type → IR [`Type`].
// ---------------------------------------------------------------------------

/// Map a C type spelling to an IR [`Type`].
///
/// Scalars map to IR primitives so codegen emits the natural fixed-width C
/// type. Pointers / `struct` / `enum` / `union` / unknown-but-identifier
/// types map to [`Type::Opaque`] carrying a normalised C spelling — this is
/// exactly how a DSL `opaque "T"` extern param behaves, so downstream is
/// identical. Genuinely unmappable spellings return `None` (the caller
/// skips that function with a note rather than emit a broken extern).
fn map_c_type(raw: &str) -> Option<Type> {
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

/// Collapse internal whitespace runs to single spaces; trim ends.
fn condense_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `const  uint8_t   *` → `const uint8_t *`; `char**` → `char **`.
fn normalise_pointer(s: &str) -> String {
    let mut spaced = String::new();
    for ch in s.chars() {
        if ch == '*' {
            if !spaced.ends_with(' ') && !spaced.is_empty() {
                spaced.push(' ');
            }
            spaced.push('*');
        } else {
            spaced.push(ch);
        }
    }
    condense_ws(&spaced)
}

// ---------------------------------------------------------------------------
// Small lexical helpers.
// ---------------------------------------------------------------------------

fn is_c_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_type_keyword(s: &str) -> bool {
    matches!(
        s,
        "void"
            | "bool"
            | "_Bool"
            | "char"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "signed"
            | "unsigned"
            | "const"
            | "volatile"
            | "int8_t"
            | "int16_t"
            | "int32_t"
            | "int64_t"
            | "uint8_t"
            | "uint16_t"
            | "uint32_t"
            | "uint64_t"
            | "size_t"
            | "ssize_t"
            | "ptrdiff_t"
            | "uintptr_t"
            | "intptr_t"
    )
}

/// Reserved words that are NOT types — if one of these turns up where a
/// type belongs the fragment is not a function declaration we can model.
fn is_reserved_nontype(s: &str) -> bool {
    matches!(
        s,
        "return"
            | "if"
            | "else"
            | "for"
            | "while"
            | "do"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "goto"
            | "sizeof"
            | "typedef"
            | "extern"
            | "static"
            | "inline"
            | "register"
            | "auto"
            | "default"
    )
}

/// Strip leading storage/specifier keywords and GCC/MSVC attributes from a
/// prototype so the type/name splitter sees a clean `RET NAME(...)`.
fn strip_attributes(proto: &str) -> String {
    let mut s = condense_ws(proto);
    // Remove `__attribute__((...))`, `__declspec(...)`, `__asm__(...)`
    // wherever they appear (balanced-paren aware).
    for kw in ["__attribute__", "__declspec", "__asm__", "__asm"] {
        loop {
            let Some(at) = s.find(kw) else { break };
            let after = at + kw.len();
            let rest = s[after..].trim_start();
            if !rest.starts_with('(') {
                break;
            }
            let popen = s[after..].find('(').unwrap() + after;
            let Some(pclose) = matching_paren(&s, popen) else {
                break;
            };
            s.replace_range(at..pclose + 1, "");
            s = condense_ws(&s);
        }
    }
    // Leading storage-class / inline specifiers.
    loop {
        let head = s.split_whitespace().next().unwrap_or("");
        if matches!(
            head,
            "extern" | "static" | "inline" | "__inline" | "__inline__" | "_Noreturn"
        ) {
            s = s[head.len()..].trim_start().to_string();
        } else {
            break;
        }
    }
    s
}

/// Index of the `)` matching the `(` at `open` (byte indices). Quotes were
/// already blanked upstream so no string-awareness is needed here.
fn matching_paren(s: &str, open: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split a parameter list body on top-level commas (depth-aware so a
/// pointer-to-array or nested paren — which we'll skip anyway — doesn't
/// shatter).
fn split_params(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' | '[' => {
                depth += 1;
                cur.push(ch);
            }
            ')' | ']' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

// ---------------------------------------------------------------------------
// Stage 6 — finalise into an IR ExternObject.
// ---------------------------------------------------------------------------

/// If any parameter or the return type is [`Type::Opaque`] (pointer /
/// `struct` / `size_t` / unknown typedef), return a precise skip reason.
/// `None` means the whole signature is primitive-only and safe to import.
///
/// See the call site in [`parse_header`] for *why* opaque-signature
/// functions cannot be imported (the DSL extern lowering path drops opaque
/// param/return types — a pre-existing pipeline limitation, not this
/// feature's bug).
fn opaque_in_signature(f: &RawFn) -> Option<String> {
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
fn finalize(f: RawFn, header_label: &str) -> ExternObject {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn names(h: &ImportedHeader) -> Vec<&str> {
        h.externs.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn dsl_keyword_param_names_are_made_safe_for_synthesis() {
        // C param names `on` / `state` collide with DSL keywords; `2bad`
        // isn't a valid ident; `rpm` is fine. The synthesized DSL line must
        // be parseable, so colliding/invalid names are rewritten while a
        // clean name is kept verbatim.
        assert_eq!(dsl_safe_param_name("on", 0), "on_");
        assert_eq!(dsl_safe_param_name("state", 1), "state_");
        assert_eq!(dsl_safe_param_name("u8", 2), "u8_");
        assert_eq!(dsl_safe_param_name("", 3), "arg3");
        assert_eq!(dsl_safe_param_name("2bad", 4), "arg4");
        assert_eq!(dsl_safe_param_name("rpm", 5), "rpm");
    }

    #[test]
    fn render_extern_decl_round_trips_through_the_dsl_parser() {
        // The synthesized text MUST re-parse as a valid DSL extern (this is
        // the contract the whole feature rests on). Build one with a
        // keyword-colliding param + a void return + a scalar return and
        // feed it back through `fsm_parser::parse`.
        let h = parse_header(
            "void relay_set(bool on);\nuint8_t crc(uint8_t seed);\n",
            "t.h",
        );
        let (block, _) = h.to_dsl_externs(&[]);
        let src = format!("language fsm 2.0\n{block}\nmachine M {{ initial S state S {{}} }}");
        let pr = fsm_parser::parse(&src);
        assert!(
            pr.errors.is_empty(),
            "synthesized extern block did not re-parse cleanly:\n{block}\nerrors: {:#?}",
            pr.errors
        );
    }

    #[test]
    fn plain_void_function_imports_with_no_params() {
        let h = parse_header("void hal_reset(void);", "t.h");
        assert_eq!(names(&h), ["hal_reset"]);
        assert!(h.externs[0].params.is_empty());
        assert!(h.externs[0].return_type.is_none());
        assert!(!h.externs[0].pure);
    }

    #[test]
    fn extern_keyword_and_stdint_types_map_to_primitives() {
        let h = parse_header("extern uint16_t adc_read(uint8_t channel);", "t.h");
        assert_eq!(names(&h), ["adc_read"]);
        assert_eq!(
            h.externs[0].return_type,
            Some(Type::Primitive { name: "u16".into() })
        );
        assert_eq!(h.externs[0].params.len(), 1);
        assert_eq!(
            h.externs[0].params[0].ty,
            Type::Primitive { name: "u8".into() }
        );
        assert_eq!(h.externs[0].params[0].name, "channel");
    }

    #[test]
    fn pointer_param_function_is_skipped_with_opaque_note() {
        // The DSL extern lowering can't model opaque param types, so a
        // function with a pointer param is skipped (not misparsed, not
        // emitted with the pointer silently dropped).
        let h = parse_header("int uart_write(const uint8_t *buf, uint32_t len);", "t.h");
        assert!(h.externs.is_empty(), "must not emit a broken extern");
        assert_eq!(h.skipped.len(), 1);
        assert!(
            h.skipped[0].reason.contains("pointer/struct/opaque"),
            "reason was: {}",
            h.skipped[0].reason
        );
        assert!(h.skipped[0].reason.contains("buf"));
    }

    #[test]
    fn struct_pointer_param_function_is_skipped() {
        let h = parse_header("bool dev_open(struct device *d);", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
        assert!(h.skipped[0].reason.contains("struct device *"));
    }

    #[test]
    fn opaque_return_function_is_skipped() {
        let h = parse_header("struct ctx *ctx_new(uint8_t id);", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
        assert!(
            h.skipped[0].reason.contains("return type"),
            "reason was: {}",
            h.skipped[0].reason
        );
    }

    #[test]
    fn unnamed_params_get_synthesised_names() {
        let h = parse_header("int add(int, int);", "t.h");
        assert_eq!(h.externs[0].params[0].name, "arg0");
        assert_eq!(h.externs[0].params[1].name, "arg1");
    }

    #[test]
    fn variadic_keeps_fixed_prefix_drops_ellipsis() {
        // Fixed prefix is scalar so the function imports; the ellipsis is
        // dropped (a DSL action passes only the fixed args).
        let h = parse_header("int dbg_logf(uint8_t level, ...);", "t.h");
        assert_eq!(names(&h), ["dbg_logf"]);
        assert_eq!(h.externs[0].params.len(), 1);
        assert_eq!(h.externs[0].params[0].name, "level");
        assert_eq!(
            h.externs[0].params[0].ty,
            Type::Primitive { name: "u8".into() }
        );
    }

    #[test]
    fn variadic_with_pointer_prefix_is_skipped() {
        // `const char *fmt` is opaque ⇒ the whole function is skipped.
        let h = parse_header("int dbg_printf(const char *fmt, ...);", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
    }

    #[test]
    fn typedef_is_skipped_not_misparsed() {
        let h = parse_header("typedef unsigned int handle_t;", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
        assert!(h.skipped[0].reason.contains("typedef"));
    }

    #[test]
    fn struct_definition_body_is_skipped() {
        let h = parse_header("struct point { int x; int y; };", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
    }

    #[test]
    fn function_like_macro_is_noted_as_skipped() {
        let h = parse_header("#define SQUARE(x) ((x)*(x))\nvoid go(void);", "t.h");
        assert_eq!(names(&h), ["go"]);
        assert!(h.skipped.iter().any(|s| s.reason.contains("macro")));
    }

    #[test]
    fn attribute_is_stripped_function_still_imported() {
        let h = parse_header(
            "void __attribute__((noreturn)) panic(uint32_t code);",
            "t.h",
        );
        assert_eq!(names(&h), ["panic"]);
        assert_eq!(
            h.externs[0].params[0].ty,
            Type::Primitive { name: "u32".into() }
        );
    }

    #[test]
    fn multi_line_declaration_is_joined() {
        let src = "int\nfoo(\n  uint8_t a,\n  uint8_t b\n);";
        let h = parse_header(src, "t.h");
        assert_eq!(names(&h), ["foo"]);
        assert_eq!(h.externs[0].params.len(), 2);
    }

    #[test]
    fn comments_do_not_terminate_declarations() {
        let src = "/* a ; semicolon in a comment */ \
                   void clk_init(void); // trailing\n";
        let h = parse_header(src, "t.h");
        assert_eq!(names(&h), ["clk_init"]);
    }

    #[test]
    fn function_pointer_typedef_and_param_are_skipped() {
        let h = parse_header(
            "typedef void (*cb_t)(int);\nvoid reg(void (*cb)(int));",
            "t.h",
        );
        // typedef skipped; the fn-pointer *param* makes `reg` unmodellable.
        assert!(h.externs.is_empty());
        assert!(h.skipped.len() >= 2);
    }

    #[test]
    fn gnarly_header_does_not_panic_and_skips_cleanly() {
        // A deliberately hostile mix: nested macros, conditional blocks,
        // attributes, a typedef'd fn pointer, an enum body, a struct with a
        // bitfield, string/char literals containing `;` and `)`, and one
        // genuinely importable function buried in the noise.
        let src = r#"
#ifndef DRIVER_H
#define DRIVER_H
#include <stdint.h>
#include <stdbool.h>

#define DRIVER_VERSION "1.2;3)"   /* literal with ; and ) */
#define MAX(a,b) ((a)>(b)?(a):(b))
#define REG(x) (*(volatile uint32_t*)(x))

typedef enum { MODE_A = 0, MODE_B } mode_t;
typedef struct {
    uint32_t a : 4;
    uint32_t b : 28;
    char     tag[8];
} packed_t;
typedef int (*isr_t)(void *ctx, uint32_t flags);

struct opaque_handle;

extern volatile uint32_t g_ticks;

/* Opaque param ⇒ skipped (DSL can't model it). */
__attribute__((weak)) bool sensor_ready(const struct sensor *s, uint16_t timeout_ms);

/* The one scalar-only good one — survives the noise. */
uint16_t sensor_read(uint8_t channel);

int (*get_isr(void))(int);            /* fn-ptr return — skip */
void apply(int arr[static 4]);        /* array param — skip */

#if defined(__cplusplus)
extern "C" { }
#endif
#endif
"#;
        let h = parse_header(src, "driver.h");
        // Exactly one importable symbol survives the noise; the opaque-param
        // `sensor_ready` is correctly skipped, not misparsed.
        assert_eq!(names(&h), ["sensor_read"]);
        let e = &h.externs[0];
        assert_eq!(e.return_type, Some(Type::Primitive { name: "u16".into() }));
        assert_eq!(e.params[0].ty, Type::Primitive { name: "u8".into() });
        // And it skipped a healthy pile without panicking — including the
        // opaque-param `sensor_ready`.
        assert!(h.skipped.len() >= 3, "skipped={:#?}", h.skipped);
        assert!(
            h.skipped
                .iter()
                .any(|s| s.fragment.contains("sensor_ready")),
            "sensor_ready should be skip-noted: {:#?}",
            h.skipped
        );
    }

    #[test]
    fn long_long_and_signedness_map_correctly() {
        let h = parse_header(
            "unsigned long long big(long long a, unsigned char b, signed char c);",
            "t.h",
        );
        let e = &h.externs[0];
        assert_eq!(e.return_type, Some(Type::Primitive { name: "u64".into() }));
        assert_eq!(e.params[0].ty, Type::Primitive { name: "i64".into() });
        assert_eq!(e.params[1].ty, Type::Primitive { name: "u8".into() });
        assert_eq!(e.params[2].ty, Type::Primitive { name: "i8".into() });
    }

    #[test]
    fn size_t_and_pointer_function_is_skipped() {
        // `size_t` is passed through opaque (its width is platform-
        // dependent — forcing an IR primitive would be a silent truncation
        // lie); combined with the `void *` return, the whole function is
        // skipped rather than emitted with a wrong/lossy signature.
        let h = parse_header("void *alloc(size_t n);", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
        assert!(
            h.skipped[0].reason.contains("pointer/struct/opaque"),
            "reason was: {}",
            h.skipped[0].reason
        );
    }

    #[test]
    fn empty_header_yields_nothing() {
        let h = parse_header("\n\n  \n", "t.h");
        assert!(h.externs.is_empty());
        assert!(h.skipped.is_empty());
    }

    #[test]
    fn project_typedef_return_function_is_skipped() {
        // `my_handle_t` is an unknown typedef → mapped opaque internally;
        // since the DSL extern lowering can't carry opaque returns, the
        // function is skipped with a precise note (not emitted lossily).
        let h = parse_header("my_handle_t open_port(uint8_t id);", "t.h");
        assert!(h.externs.is_empty());
        assert_eq!(h.skipped.len(), 1);
        assert!(h.skipped[0].reason.contains("my_handle_t"));
    }

    #[test]
    fn map_c_type_still_classifies_opaque_internally() {
        // The C→IR mapper itself continues to recognise pointers/structs as
        // Opaque (used for the precise skip-note message); only the
        // *function-level* import is gated.
        assert_eq!(
            map_c_type("const uint8_t *"),
            Some(Type::Opaque {
                c_type: "const uint8_t *".into()
            })
        );
        assert_eq!(
            map_c_type("struct device *"),
            Some(Type::Opaque {
                c_type: "struct device *".into()
            })
        );
        assert_eq!(
            map_c_type("uint16_t"),
            Some(Type::Primitive { name: "u16".into() })
        );
    }

    #[test]
    fn inline_definition_keeps_prototype_drops_body() {
        let h = parse_header(
            "static inline uint8_t clamp(uint8_t v) { return v > 7 ? 7 : v; }",
            "t.h",
        );
        assert_eq!(names(&h), ["clamp"]);
        assert_eq!(
            h.externs[0].return_type,
            Some(Type::Primitive { name: "u8".into() })
        );
    }
}
