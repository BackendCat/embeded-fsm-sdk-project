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
    }

    body.push_str("\n/* ── Entry actions ────────────────────────────────────────────────── */\n");
    for rec in &ctx.index.records {
        if !rec.kind.is_active_at_rest() || rec.kind == StateRecordKind::Final {
            // Final states never carry user entry actions (they fire
            // completion immediately).
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
    }

    body.push_str("\n#ifdef __cplusplus\n}\n#endif\n\n");
    body.push_str(&format!("#endif /* {} */\n", guard));

    EmittedFile {
        path: format!("{}_impl.h", stem),
        role: FileRole::ImplHeader,
        content: format!("{}\n{}", header, body),
    }
}
