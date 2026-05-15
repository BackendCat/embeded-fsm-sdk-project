//! `Motor.h` — public API header.
//!
//! Doc 11 §3 — §5. Declares `M_StateId_t`, `M_EventId_t`, payload structs,
//! the event tagged union, the context struct, the runtime struct, and the
//! four public function prototypes.

use crate::expr::primitive_to_c;
use crate::state_index::StateRecordKind;

use super::license::header_block;
use super::{EmittedFile, FileRole, MachineEmitCtx};

pub fn emit(ctx: &MachineEmitCtx<'_>) -> EmittedFile {
    let macro_prefix = ctx.macro_prefix();
    let stem = ctx.file_stem();
    let guard = format!("{}_H", macro_prefix);

    let header = header_block(ctx.config, Some(&format!("{}.fsm (public API)", stem)));

    let mut body = String::new();
    body.push_str(&format!("#ifndef {}\n#define {}\n\n", guard, guard));
    body.push_str("#include <stdint.h>\n#include <stdbool.h>\n");
    body.push_str(&format!("#include \"{}_conf.h\"\n", stem));
    // v1.1-W2d: a `state X is Sub` ref-state nests a `Sub_t` value member
    // (Doc 08 §12 / Doc 09 §4.11). The template is its own codegen unit
    // (emitted first by `emit_machine_recursive`); include its header so the
    // nested member is a complete type. De-duplicated by include name so
    // two ref-states pointing at the same template include it once.
    {
        let mut included: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for sr in super::submachine::collect_sub_refs(ctx) {
            if included.insert(sr.sub_type.clone()) {
                body.push_str(&format!("#include \"{}.h\"\n", sr.sub_type));
            }
        }
    }
    // v1.1-W4: portable branch-prediction macros. Emitted before the
    // `extern "C"` block (macros have no linkage) so the dispatch code in
    // the `.c` can wrap hinted transition conditions. A machine with no
    // `likely`/`rare` hint still emits these (a few unused object-like
    // #defines, zero codegen/behaviour impact on existing FSMs — the
    // generated `.c` is byte-identical except at intended hint sites
    // because the dispatch only references the macro when a hint is set).
    body.push_str(&emit_branch_hint_macros(ctx));

    body.push_str("\n#ifdef __cplusplus\nextern \"C\" {\n#endif\n\n");

    body.push_str(&emit_state_enum(ctx));
    body.push_str("\n");
    body.push_str(&emit_event_enum(ctx));
    body.push_str("\n");
    body.push_str(&emit_payload_structs(ctx));
    body.push_str(&emit_event_union(ctx));
    body.push_str("\n");
    body.push_str(&emit_context_struct(ctx));
    body.push_str("\n");
    body.push_str(&emit_machine_struct(ctx));
    body.push_str("\n");
    body.push_str(&emit_api_prototypes(ctx));
    body.push_str("\n#ifdef __cplusplus\n}\n#endif\n\n");
    body.push_str(&format!("#endif /* {} */\n", guard));

    EmittedFile {
        path: format!("{}.h", stem),
        role: FileRole::Header,
        content: format!("{}\n{}", header, body),
    }
}

/// v1.1-W4 — emit the portable `<PREFIX>_LIKELY` / `<PREFIX>_UNLIKELY`
/// branch-prediction macros (Doc 11 §28).
///
/// `__builtin_expect` is a GCC/Clang extension, not ISO C. To keep the
/// generated firmware portable (the heap-free / no-extension discipline —
/// it must build on any conforming C99 compiler), the macros expand to
/// `__builtin_expect` only under `__GNUC__` / `__clang__` and fall back to
/// the bare condition elsewhere. The `!!(x)` double-negation normalises any
/// scalar to `0`/`1` (`__builtin_expect`'s args are `long`) and is the
/// standard `-Wpedantic`-safe idiom — it compiles clean under
/// `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`. Semantics are identical
/// in both arms (the macro never changes the value of `x`, only the
/// optimiser's block-placement hint), so behaviour is unchanged on non-GNU
/// toolchains — only the hot/cold layout differs.
fn emit_branch_hint_macros(ctx: &MachineEmitCtx<'_>) -> String {
    let p = ctx.macro_prefix();
    format!(
        r#"/* ── Branch-prediction hints (v1.1-W4, Doc 11 §28) ──────────────────────
 * `likely`/`rare` transition prefixes lower to these. __builtin_expect is
 * a GCC/Clang extension; the #else keeps generated firmware buildable on
 * any conforming C99 compiler (the (x) fallback is a pure no-op hint).
 * Define {p}_NO_BUILTIN_EXPECT before including this header to force the
 * portable fallback even on GCC/Clang (exotic toolchains / measurement).
 * Pure layout optimization — ZERO semantic effect. */
#if !defined({p}_NO_BUILTIN_EXPECT) && (defined(__GNUC__) || defined(__clang__))
#  define {p}_LIKELY(x)   (__builtin_expect(!!(x), 1))
#  define {p}_UNLIKELY(x) (__builtin_expect(!!(x), 0))
#else
#  define {p}_LIKELY(x)   (x)
#  define {p}_UNLIKELY(x) (x)
#endif
"#,
        p = p,
    )
}

fn emit_state_enum(ctx: &MachineEmitCtx<'_>) -> String {
    let macro_prefix = ctx.macro_prefix();
    let prefix = ctx.type_prefix();
    let mut s = String::from("/* ── State IDs ──────────────────────────────────────────────────────── */\ntypedef enum {\n");
    for (i, rec) in ctx.index.records.iter().enumerate() {
        s.push_str(&format!(
            "    {macro}_STATE_{name} = {i},\n",
            macro = macro_prefix,
            name = rec.c_name,
            i = i,
        ));
    }
    s.push_str(&format!(
        "    {macro}_STATE__COUNT\n}} {prefix}_StateId_t;\n",
        macro = macro_prefix,
        prefix = prefix,
    ));
    s
}

fn emit_event_enum(ctx: &MachineEmitCtx<'_>) -> String {
    let macro_prefix = ctx.macro_prefix();
    let prefix = ctx.type_prefix();
    let mut s = String::from("/* ── Event IDs ──────────────────────────────────────────────────────── */\ntypedef enum {\n");
    for (i, ev) in ctx.machine.events.iter().enumerate() {
        let event_c = crate::state_index::c_ident(&ev.name);
        s.push_str(&format!(
            "    {macro}_EVENT_{name} = {i},\n",
            macro = macro_prefix,
            name = event_c,
            i = i,
        ));
    }
    // Internal completion event slot — reserved before the timer block so
    // its enum value stays stable (Doc 11 §15).
    s.push_str(&format!(
        "    {macro}_EVENT__COMPLETION,\n",
        macro = macro_prefix
    ));
    // P0-4: per-timer events. Each `after` / `every` / `every_internal`
    // declaration gets its own variant so the transition's trigger is
    // distinguishable from `done` completion. These are synthesized by
    // `Motor_advance_clock` only — public callers do not push them.
    for t in super::timer::collect_timers(ctx) {
        s.push_str(&format!(
            "    {macro}_EVENT_{esuffix},\n",
            macro = macro_prefix,
            esuffix = t.event_suffix,
        ));
    }
    s.push_str(&format!(
        "    {macro}_EVENT__COUNT\n}} {prefix}_EventId_t;\n",
        macro = macro_prefix,
        prefix = prefix,
    ));
    s
}

fn emit_payload_structs(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::new();
    let has_any = ctx.machine.events.iter().any(|e| !e.payload.is_empty());
    if !has_any {
        return s;
    }
    s.push_str("/* ── Payload types ─────────────────────────────────────────────────── */\n");
    for ev in &ctx.machine.events {
        if ev.payload.is_empty() {
            continue;
        }
        let ev_c = crate::state_index::c_ident(&ev.name);
        s.push_str(&format!("typedef struct {{\n"));
        for p in &ev.payload {
            let ty = match &p.ty {
                fsm_ir::Type::Primitive { name } => primitive_to_c(name),
                _ => "int",
            };
            s.push_str(&format!("    {ty} {name};\n", ty = ty, name = p.name));
        }
        s.push_str(&format!(
            "}} {prefix}_{ev}Payload_t;\n\n",
            prefix = prefix,
            ev = ev_c,
        ));
    }
    s
}

fn emit_event_union(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::from(
        "/* ── Event tagged union ────────────────────────────────────────────── */\n",
    );
    // Carry the event id at offset 0 so reading `ev->id` is always safe per
    // C99 §6.7.2.1 (Doc 11 §5).
    s.push_str(&format!(
        "typedef struct {{\n    {prefix}_EventId_t id;\n    union {{\n",
        prefix = prefix,
    ));
    for ev in &ctx.machine.events {
        if ev.payload.is_empty() {
            continue;
        }
        let ev_c = crate::state_index::c_ident(&ev.name);
        s.push_str(&format!(
            "        {prefix}_{ev}Payload_t {field};\n",
            prefix = prefix,
            ev = ev_c,
            field = ev.name,
        ));
    }
    // Ensure the union is non-empty even when no event carries a payload —
    // an empty union is undefined in C99.
    if !ctx.machine.events.iter().any(|e| !e.payload.is_empty()) {
        s.push_str("        uint8_t __empty;\n");
    }
    s.push_str(&format!(
        "    }} __payload;\n}} {prefix}_Event_t;\n",
        prefix = prefix,
    ));
    s
}

fn emit_context_struct(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::from(
        "/* ── User context (declared in DSL `context { ... }`) ──────────────── */\n",
    );
    s.push_str(&format!("typedef struct {{\n"));
    for f in &ctx.machine.context.fields {
        let ty = match &f.ty {
            fsm_ir::Type::Primitive { name } => primitive_to_c(name).to_owned(),
            fsm_ir::Type::Opaque { c_type } => c_type.clone(),
            fsm_ir::Type::Enum { .. } => "int".to_owned(),
            fsm_ir::Type::Array { element, size } => {
                let inner = match element.as_ref() {
                    fsm_ir::Type::Primitive { name } => primitive_to_c(name).to_owned(),
                    _ => "int".to_owned(),
                };
                return format!(
                    "typedef struct {{\n    {inner} {name}[{size}];\n",
                    inner = inner,
                    name = f.name,
                    size = size
                );
            }
        };
        s.push_str(&format!("    {ty} {name};\n", ty = ty, name = f.name));
    }
    if ctx.machine.context.fields.is_empty() {
        s.push_str("    uint8_t __empty;\n");
    }
    s.push_str(&format!("}} {prefix}_Context_t;\n", prefix = prefix));
    s
}

fn emit_machine_struct(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::from(
        "/* ── Machine struct — user allocates this in stack/BSS/static ──────── */\n",
    );
    s.push_str(&format!("typedef struct {{\n"));
    s.push_str(&format!(
        "    {prefix}_Context_t context;\n",
        prefix = prefix
    ));
    // B-11 active-leaf array: slot 0 holds the singleton non-parallel
    // leaf; slots 1..N hold per-region leaves inside parallel states.
    // `_active_count` tracks how many slots are currently valid (1 in
    // non-parallel mode, region-count when a parallel state is active).
    s.push_str(&format!(
        "    {prefix}_StateId_t _active[{macro}_MAX_PARALLEL_REGIONS];\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    s.push_str("    uint8_t _active_count;\n");
    // History slots.
    for (slot_idx, rec) in ctx
        .index
        .records
        .iter()
        .filter(|r| r.history_pseudo.is_some())
        .enumerate()
    {
        s.push_str(&format!(
            "    {prefix}_StateId_t _history_{name}; /* slot {slot} */\n",
            prefix = prefix,
            name = rec.c_name,
            slot = slot_idx,
        ));
    }
    // Timer slots.
    let timer_names = crate::emit::source::collect_timer_names(ctx.machine);
    for name in &timer_names {
        s.push_str(&format!(
            "    uint32_t _timer_{name}_remaining_ms;\n",
            name = name,
        ));
    }
    // Event queue.
    s.push_str(&format!(
        "    {prefix}_Event_t _queue[{macro}_QUEUE_CAPACITY];\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    s.push_str("    uint8_t _queue_head;\n");
    s.push_str("    uint8_t _queue_tail;\n");
    s.push_str("    uint8_t _queue_count;\n");
    s.push_str("    uint8_t _completion_depth; /* per-instance watchdog */\n");
    // Deferred-event buffer (Doc 08 §10). Always present so the struct
    // layout is stable regardless of whether this machine uses `defer`;
    // `Motor_init`'s memset zeroes it, and the defer apparatus is only
    // emitted (and only touches these) when the machine declares a
    // `defer`. A bounded array — heap-free per Doc 02 G2.
    s.push_str(&format!(
        "    {prefix}_Event_t _deferred[{macro}_DEFER_CAPACITY];\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    s.push_str("    uint8_t _deferred_count;\n");
    // v1.1-W2d: submachine ref-state sub-instances. Each `state X is Sub`
    // owns a nested `Sub_t` value member (Doc 08 §12, heap-free per Doc 02
    // G2 — exactly how `_active[]`/timers/defer buffer nest). Always
    // present so the struct layout is stable; `Sub_init` is called on
    // ref-state entry (and re-entry resets it fresh). One member per
    // ref-state (a template referenced twice ⇒ two independent instances).
    for sr in super::submachine::collect_sub_refs(ctx) {
        s.push_str(&format!(
            "    {ty} {member}; /* `{name}` submachine sub-instance (Doc 08 §12) */\n",
            ty = sr.member_type(),
            member = sr.member,
            name = sr.parent_state_c,
        ));
    }
    s.push_str(&format!("}} {prefix}_t;\n", prefix = prefix));
    s
}

fn emit_api_prototypes(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    format!(
        "/* ── Public API ────────────────────────────────────────────────────── */\n\
         void  {prefix}_init          ({prefix}_t *m);\n\
         void  {prefix}_dispatch      ({prefix}_t *m, const {prefix}_Event_t *ev);\n\
         void  {prefix}_post          ({prefix}_t *m, const {prefix}_Event_t *ev);\n\
         bool  {prefix}_dequeue       ({prefix}_t *m, {prefix}_Event_t *out);\n\
         void  {prefix}_advance_clock ({prefix}_t *m, uint32_t elapsed_ms);\n\
         {prefix}_StateId_t {prefix}_current_state(const {prefix}_t *m);\n",
        prefix = prefix,
    )
}

// Workaround: keep `StateRecordKind` import live (used elsewhere in the
// emit tree).
const _: Option<StateRecordKind> = None;
