//! `Motor_impl.h` — user contract declarations.
//!
//! Doc 11 §7. The user MUST provide implementations for every symbol
//! declared here. The linker fails if any are missing.
//!
//! Contents:
//!  - Guard functions (named per IR `pure` externs).
//!  - Per-state entry / exit handlers (one pair per active-at-rest state).
//!  - Transition action externs (named per IR externs / transition action
//!    statements that call `extern`).

use crate::state_index::StateRecordKind;

use super::license::header_block;
use super::{EmittedFile, FileRole, MachineEmitCtx};
use crate::expr::{c_type_str, primitive_to_c};
use fsm_ir::Type;

pub fn emit(ctx: &MachineEmitCtx<'_>) -> EmittedFile {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let stem = ctx.file_stem();
    let guard = format!("{}_IMPL_H", macro_prefix);

    let header = header_block(
        ctx.config,
        Some(&format!("{}.fsm (user-implemented externs)", stem)),
    );

    let mut body = String::new();
    body.push_str(&format!("#ifndef {}\n#define {}\n\n", guard, guard));
    body.push_str(&format!("#include \"{}.h\"\n\n", stem));
    body.push_str("#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n");

    // Guard externs — declared at the top so callers can grep for them. The
    // analyzer enforces `pure` on guard externs, but the C contract just
    // needs the signature.
    //
    // Two parallel forms are emitted: the historical `<Prefix>_guard_<name>`
    // wrapper (kept for older user code that targets it directly) and the
    // bare-name form (`bool can_start(void);`) the action-block / guard
    // emitters use after P0-1 lowering. The bare form mirrors the DSL
    // declaration verbatim so user-authored extern bodies link with no
    // glue layer.
    body.push_str("/* ── Guards (pure — no side effects, MUST NOT mutate context) ─────── */\n");
    for ext in &ctx.machine.externs {
        if !ext.pure {
            continue;
        }
        body.push_str(&format!(
            "bool {prefix}_guard_{name}(const {prefix}_t *m, const {prefix}_Event_t *ev);\n",
            prefix = prefix,
            name = ext.name,
        ));
        body.push_str(&emit_bare_extern_decl(ext));
    }

    body.push_str("\n/* ── Entry actions ────────────────────────────────────────────────── */\n");
    for rec in &ctx.index.records {
        if !rec.kind.is_active_at_rest() || rec.kind == StateRecordKind::Final {
            // Final states never carry user entry actions (they fire
            // completion immediately).
            continue;
        }
        // v1.1-W2d: a submachine ref-state (`state X is Sub`) has NO user
        // entry/exit action — the `is Sub { … }` grammar carries only
        // transitions, and the sub-instance lifecycle (init on entry, fresh
        // re-init on self-transition) is codegen's job, NOT a user extern.
        // This mirrors the merged W2c simulator, which runs no ref-state
        // entry action and instead builds the sub-`RuntimeState`. Emitting
        // a `_entry_X` prototype would force the user to implement a
        // meaningless stub and risk a link error.
        if super::submachine::is_submachine_record(rec.kind) {
            continue;
        }
        body.push_str(&format!(
            "void {prefix}_entry_{name}({prefix}_t *m);\n",
            prefix = prefix,
            name = rec.c_name,
        ));
    }

    body.push_str("\n/* ── Exit actions ─────────────────────────────────────────────────── */\n");
    for rec in &ctx.index.records {
        if !rec.kind.is_active_at_rest() || rec.kind == StateRecordKind::Final {
            continue;
        }
        if super::submachine::is_submachine_record(rec.kind) {
            continue;
        }
        body.push_str(&format!(
            "void {prefix}_exit_{name}({prefix}_t *m);\n",
            prefix = prefix,
            name = rec.c_name,
        ));
    }

    body.push_str("\n/* ── Transition actions ───────────────────────────────────────────── */\n");
    for ext in &ctx.machine.externs {
        if ext.pure {
            continue;
        }
        body.push_str(&format!(
            "void {prefix}_action_{name}({prefix}_t *m, const {prefix}_Event_t *ev);\n",
            prefix = prefix,
            name = ext.name,
        ));
        body.push_str(&emit_bare_extern_decl(ext));
    }

    body.push_str("\n#ifdef __cplusplus\n}\n#endif\n\n");
    body.push_str(&format!("#endif /* {} */\n", guard));

    EmittedFile {
        path: format!("{}_impl.h", stem),
        role: FileRole::ImplHeader,
        content: format!("{}\n{}", header, body),
    }
}

/// Emit a bare-name extern prototype matching the DSL declaration. Pure
/// externs return `bool` by default; non-pure externs return `void` unless
/// the DSL specifies otherwise. Parameter names are preserved from the IR
/// when available so the generated header reads like the source.
fn emit_bare_extern_decl(ext: &fsm_ir::ExternObject) -> String {
    let return_ty = match &ext.return_type {
        Some(t) => c_type_str(t),
        None => {
            if ext.pure {
                "bool".to_string()
            } else {
                "void".to_string()
            }
        }
    };
    let params = if ext.params.is_empty() {
        "void".to_string()
    } else {
        ext.params
            .iter()
            .map(|p| {
                let c_ty = match &p.ty {
                    Type::Primitive { name } => primitive_to_c(name).to_string(),
                    other => c_type_str(other),
                };
                if p.name.is_empty() {
                    c_ty
                } else {
                    format!("{} {}", c_ty, p.name)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!("{} {}({});\n", return_ty, ext.name, params)
}
