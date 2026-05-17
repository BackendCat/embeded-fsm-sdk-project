//! `#ifdef FSM_TRACE` runtime trace-emit hook — the W1 R7 host-trace
//! differential (Doc 32 §1 W1).
//!
//! ## What this is
//!
//! A **compile-time-gated** instrumentation hook. Every emitter site that
//! calls into this module wraps its output in `#ifdef FSM_TRACE … #endif`,
//! so a default `fsm generate` (no `-DFSM_TRACE`) produces **byte-identical**
//! C to before this module existed — the generated *file text* always
//! contains the hook, but the C **preprocessor removes every trace line**
//! unless the consumer compiles with `-DFSM_TRACE`. This is the established
//! append-only-instrumentation discipline (the v1.x `submachine`-field
//! precedent): the production build is unchanged; the differential build
//! opts in.
//!
//! ## The keystone (Doc 32 §2 / §6 H2 — the rule W6 re-derives)
//!
//! This hook is a **trace tap, not a second interpreter**. It emits *what
//! the generated C actually did* — the event it processed, the transition
//! it selected **via the generated dispatch's own leaf-to-root walk** (the
//! `stable_id`/`source`/`target` are compile-time literals of the
//! transition the C *itself* fired), and a snapshot of the C runtime's own
//! `_active[]` configuration. It does **not** consult, re-derive, or
//! re-implement the `fsm_simulator` step/transition-selection/guard
//! semantics. The differential harness drives the **shipped**
//! `fsm_simulator::execute_trace` (the unforked oracle) and byte-diffs its
//! `StepRecord` projection against this hook's stdout. If the generated C's
//! behaviour diverges from the simulator, the byte-diff goes RED — that
//! divergence signal is the entire point; it is not a fork.
//!
//! ## The sink (`fsm_trace_emit`) — W2-ready by construction
//!
//! Every record is one canonical line passed to a single sink:
//!
//! ```c
//! void fsm_trace_emit(const char *line);
//! ```
//!
//! declared **weak** (`__attribute__((weak))` on GNU/Clang, a plain extern
//! fallback elsewhere) with a default host implementation that `fputs`es to
//! `stdout`. The host differential harness links the default (stdout
//! capture); **W2's on-target lane provides a semihosting `fsm_trace_emit`
//! with NO codegen change** — the seam is the point. The codegen never
//! needs to know whether it runs on the host or an MCU.
//!
//! ## The canonical line format (a stable, sorted, deterministic projection)
//!
//! One line per `StepRecord` boundary (`crates/fsm-simulator/src/trace.rs`):
//!
//! ```text
//! STEP\x1fkind=<k>\x1fclk=<u32>\x1fevt=<name|->\x1ftr=<stableId|->\
//!     \x1fsrc=<ir|->\x1fdst=<ir|->\x1fcfgB=<csv>\x1fcfgA=<csv>\
//!     \x1fent=<csv>\x1fext=<csv>\n
//! ```
//!
//! Field separator is US (`\x1f`, 0x1F) — a byte that cannot occur in an IR
//! id, an event name, or a state-kind keyword, so the projection is
//! unambiguous and needs no escaping. `cfgB`/`cfgA` (config before/after)
//! are the C's own `_active[]` mapped to IR ids and **sorted** (the
//! deterministic projection — `_active[]` slot order is not the simulator's
//! `active_states` push order, so both sides sort). `ent`/`ext`
//! (entered/exited) are the generated dispatch's *own* entry/exit-set
//! literals, **sorted** for the same reason. `tr`/`src`/`dst` mirror
//! `StepRecord.transition_taken`; `evt` mirrors `event_received.name`
//! (`__timer__:<id>` / `__completion__:<id>` for synthetic events, exactly
//! the simulator's form). This is a *projection of the existing StepRecord
//! wire shape* (Doc 13 §11 determinism discipline) — not a new wire form;
//! `trace_id` (a pure monotone counter) and `actions_executed` (a derived
//! call-log not observable from `_active[]`) are intentionally **not** in
//! the projection — see the harness module-doc for the disclosed rationale.

use crate::state_index::StateRecordKind;

use super::MachineEmitCtx;

/// US (unit separator, 0x1F) — the canonical-line field delimiter. Chosen
/// because it cannot appear in an IR id, an event name, or a kind keyword,
/// so the projection is parse-free and escape-free on both sides.
pub const FIELD_SEP: char = '\u{1f}';

/// Emit the shared `#ifdef FSM_TRACE` preamble for the `.c` translation
/// unit: the weak `fsm_trace_emit` sink + a default host implementation +
/// the per-machine state-id→IR-id table + the line-builder helper. All of
/// it is inside one `#ifdef FSM_TRACE`, so `#ifndef FSM_TRACE` ⇒ zero
/// bytes survive the preprocessor (the production build is byte-identical).
pub fn emit_trace_preamble(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let sep = FIELD_SEP as u32;

    let mut s = String::new();
    s.push_str("#ifdef FSM_TRACE\n");
    s.push_str("/* ── W1 R7 host-trace differential hook (compile-time-gated) ─────────\n");
    s.push_str(" * Doc 32 §1 W1 / §2 keystone. This block exists in the generated\n");
    s.push_str(" * file text unconditionally but the C preprocessor strips ALL of it\n");
    s.push_str(" * unless the consumer compiles with -DFSM_TRACE, so the default\n");
    s.push_str(" * `fsm generate` output is byte-identical (append-only-instrumentation\n");
    s.push_str(" * discipline). It is a TRACE TAP, not a second interpreter: it emits\n");
    s.push_str(" * what THIS generated C did; the shipped fsm_simulator remains the\n");
    s.push_str(" * sole semantic oracle the host harness byte-diffs against. */\n");
    s.push_str("#include <stdio.h>\n");
    s.push_str("#include <string.h>\n\n");

    // The weak sink. The host harness / W2 semihosting override it with a
    // strong definition; absent an override the default writes to stdout.
    s.push_str("/* Weak trace sink. The host differential harness links the default\n");
    s.push_str(" * (stdout). W2's on-target lane provides a semihosting strong\n");
    s.push_str(" * override with ZERO codegen change — that seam is the point. */\n");
    s.push_str("#if defined(__GNUC__) || defined(__clang__)\n");
    s.push_str("__attribute__((weak))\n");
    s.push_str("#endif\n");
    s.push_str("void fsm_trace_emit(const char *line);\n");
    s.push_str("#if defined(__GNUC__) || defined(__clang__)\n");
    s.push_str("__attribute__((weak))\n");
    s.push_str("void fsm_trace_emit(const char *line) {\n");
    s.push_str("    fputs(line, stdout);\n");
    s.push_str("}\n");
    s.push_str("#endif\n\n");

    // State-id → IR-id table. The C runtime tracks state by the
    // `<MACHINE>_STATE_*` enum; the simulator's StepRecord uses the IR id
    // string (e.g. `s-Motor-Idle`). This table is the *only* bridge; it is
    // generated data, not logic.
    s.push_str("/* StateId enum → IR-id string (the StepRecord wire id form). */\n");
    s.push_str(&format!(
        "static const char *const {prefix}_trace_ir_id[{macro}_STATE__COUNT] = {{\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    for (i, rec) in ctx.index.records.iter().enumerate() {
        // The root sentinel never appears in `_active[]` at rest; emit an
        // empty string so the array is total and indexable by any StateId.
        let ir = if rec.kind == StateRecordKind::Root {
            ""
        } else {
            rec.ir_id.as_str()
        };
        s.push_str(&format!(
            "    [{i}] = {lit}, /* {dsl} */\n",
            i = i,
            lit = c_string_literal(ir),
            dsl = rec.dsl_name,
        ));
    }
    s.push_str("};\n\n");

    // CSV accumulation + sorted-emit helpers (machine-agnostic; `static`
    // so each TU has its own copy, no linkage clash). `csv_append` grows a
    // comma-list (used by the transition recorder to accumulate the
    // entered/exited union across every transition fired in a step — the
    // simulator's `entered_all`/`exited_all` aggregate). `emit_sorted_csv`
    // tokenises a comma-list, insertion-sorts the tokens, and appends them
    // comma-joined to the line buffer — the C analogue of the Rust
    // projection's `sorted_csv` (BOTH sides sort: neither engine's emission
    // order is canonical; behaviour is the SET). Pure string ops.
    s.push_str("static void fsm_trace_csv_append(char *buf, size_t cap, const char *item) {\n");
    s.push_str("    size_t l = strlen(buf);\n");
    s.push_str("    if (l > 0 && l + 1 < cap) { buf[l++] = ','; buf[l] = '\\0'; }\n");
    s.push_str("    while (*item && l + 1 < cap) { buf[l++] = *item++; }\n");
    s.push_str("    buf[l] = '\\0';\n");
    s.push_str("}\n\n");

    s.push_str("static void fsm_trace_emit_sorted_csv(char *line, size_t cap, size_t *len, const char *csv) {\n");
    s.push_str("    /* tokenise on ',' into a local copy (csv is small — id\n");
    s.push_str("     * lists per step are tiny), insertion-sort, re-emit. */\n");
    s.push_str("    char tmp[256];\n");
    s.push_str("    const char *toks[32];\n");
    s.push_str("    int nt = 0;\n");
    s.push_str("    size_t i = 0, j = 0;\n");
    s.push_str("    if (csv[0] == '\\0') return;\n");
    s.push_str("    while (csv[i] && j + 1 < sizeof(tmp)) { tmp[j++] = csv[i++]; }\n");
    s.push_str("    tmp[j] = '\\0';\n");
    s.push_str("    {\n");
    s.push_str("        int started = 0;\n");
    s.push_str("        size_t k;\n");
    s.push_str("        for (k = 0; k < j; k++) {\n");
    s.push_str("            if (!started) { if (nt < 32) toks[nt++] = &tmp[k]; started = 1; }\n");
    s.push_str("            if (tmp[k] == ',') { tmp[k] = '\\0'; started = 0; }\n");
    s.push_str("        }\n");
    s.push_str("    }\n");
    s.push_str("    {\n");
    s.push_str("        int a, b;\n");
    s.push_str("        for (a = 1; a < nt; a++) {\n");
    s.push_str("            const char *key = toks[a];\n");
    s.push_str("            b = a;\n");
    s.push_str("            while (b > 0 && strcmp(toks[b - 1], key) > 0) { toks[b] = toks[b - 1]; b--; }\n");
    s.push_str("            toks[b] = key;\n");
    s.push_str("        }\n");
    s.push_str("    }\n");
    s.push_str("    {\n");
    s.push_str("        int a;\n");
    s.push_str("        for (a = 0; a < nt; a++) {\n");
    s.push_str("            const char *p = toks[a];\n");
    s.push_str("            if (a > 0 && *len < cap) line[(*len)++] = ',';\n");
    s.push_str("            while (*p && *len < cap) line[(*len)++] = *p++;\n");
    s.push_str("        }\n");
    s.push_str("    }\n");
    s.push_str("}\n\n");

    // Scratch trace state on the machine struct is declared in the header
    // (also #ifdef FSM_TRACE). Here we emit the per-step line builder.
    //
    // The builder appends the C's OWN observed config (`_active[]` mapped
    // through the table above, sorted) — a pure snapshot, no semantics.
    s.push_str("/* Append the C runtime's own _active[] config (mapped to IR ids,\n");
    s.push_str(" * sorted) to `buf`. Sorting makes the projection deterministic:\n");
    s.push_str(" * _active[] slot order is NOT the simulator's active_states push\n");
    s.push_str(" * order, so BOTH sides sort (the canonical projection). This reads\n");
    s.push_str(" * state the C already tracked — it derives nothing. The active\n");
    s.push_str(" * array + count are PARAMETERS (the caller passes m->_active /\n");
    s.push_str(" * m->_active_count) so this single definition is correct from\n");
    s.push_str(" * every call site. */\n");
    s.push_str(&format!(
        "static void {prefix}_trace_append_config(const {prefix}_StateId_t *active, uint8_t active_count, char *buf, size_t cap, size_t *len) {{\n",
        prefix = prefix,
    ));
    s.push_str(&format!(
        "    const char *ids[{macro}_MAX_PARALLEL_REGIONS];\n",
        macro = macro_prefix,
    ));
    s.push_str("    uint8_t n = 0;\n");
    s.push_str("    uint8_t i;\n");
    s.push_str("    for (i = 0; i < active_count && i < ");
    s.push_str(&format!("{macro}_MAX_PARALLEL_REGIONS", macro = macro_prefix));
    s.push_str("; i++) {\n");
    s.push_str(&format!(
        "        ids[n++] = {prefix}_trace_ir_id[active[i]];\n",
        prefix = prefix,
    ));
    s.push_str("    }\n");
    s.push_str("    /* insertion sort by strcmp — n is tiny (region count) */\n");
    s.push_str("    {\n");
    s.push_str("        uint8_t a, b;\n");
    s.push_str("        for (a = 1; a < n; a++) {\n");
    s.push_str("            const char *key = ids[a];\n");
    s.push_str("            b = a;\n");
    s.push_str("            while (b > 0 && strcmp(ids[b - 1], key) > 0) {\n");
    s.push_str("                ids[b] = ids[b - 1];\n");
    s.push_str("                b--;\n");
    s.push_str("            }\n");
    s.push_str("            ids[b] = key;\n");
    s.push_str("        }\n");
    s.push_str("    }\n");
    s.push_str("    for (i = 0; i < n; i++) {\n");
    s.push_str("        if (i > 0 && *len < cap) buf[(*len)++] = ',';\n");
    s.push_str("        {\n");
    s.push_str("            const char *p = ids[i];\n");
    s.push_str("            while (*p && *len < cap) buf[(*len)++] = *p++;\n");
    s.push_str("        }\n");
    s.push_str("    }\n");
    s.push_str("}\n\n");

    // The line emitter. Takes the already-observed fields and formats the
    // canonical projection. `m_trace_active` / `m_trace_active_count` are
    // macro aliases the call sites set just before calling this (a snapshot
    // of `m->_active[]` / `m->_active_count`). This function is pure
    // string-building over values the C already computed.
    s.push_str("/* Build + emit ONE canonical projection line. Every argument is a\n");
    s.push_str(" * value the generated C already observed (event it processed, the\n");
    s.push_str(" * transition IT fired, its own config). Pure formatting. */\n");
    s.push_str(&format!(
        "static void {prefix}_trace_step(\n",
        prefix = prefix,
    ));
    s.push_str(&format!(
        "        const {prefix}_StateId_t *active, uint8_t active_count,\n",
        prefix = prefix,
    ));
    s.push_str("        const char *kind, const char *evt,\n");
    s.push_str("        const char *tr, const char *src, const char *dst,\n");
    s.push_str("        const char *cfg_before,\n");
    s.push_str("        const char *ent, const char *ext) {\n");
    s.push_str("    char buf[1024];\n");
    s.push_str("    size_t len = 0;\n");
    s.push_str("    const size_t cap = sizeof(buf) - 2;\n");
    s.push_str(&format!(
        "    #define {macro}_TF(s) do {{ const char *q=(s); while(*q && len<cap) buf[len++]=*q++; }} while(0)\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "    #define {macro}_TS() do {{ if (len<cap) buf[len++]=(char){sep}; }} while(0)\n",
        macro = macro_prefix,
        sep = sep,
    ));
    s.push_str(&format!("    {macro}_TF(\"STEP\");\n", macro = macro_prefix));
    s.push_str(&format!(
        "    {macro}_TS(); {macro}_TF(\"kind=\"); {macro}_TF(kind);\n",
        macro = macro_prefix,
    ));
    // NOTE: `clk` (the simulator's StepRecord.virtual_clock_ms) is
    // deliberately NOT in the projection — see the harness module-doc
    // disclosed scope. The generated runtime has NO codegen-modeled virtual
    // clock; time is HAL-delegated (host-supplied). The simulator's
    // expiry-clamped `virtual_clock_ms` on a synthetic timer step is a
    // simulator-internal artifact NOT reproducible without forking timer
    // arithmetic into the host driver — so it is excluded (same class as
    // `trace_id`). The behavioural fingerprint below is unaffected.
    s.push_str(&format!(
        "    {macro}_TS(); {macro}_TF(\"evt=\"); {macro}_TF(evt);\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "    {macro}_TS(); {macro}_TF(\"tr=\"); {macro}_TF(tr);\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "    {macro}_TS(); {macro}_TF(\"src=\"); {macro}_TF(src);\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "    {macro}_TS(); {macro}_TF(\"dst=\"); {macro}_TF(dst);\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "    {macro}_TS(); {macro}_TF(\"cfgB=\"); {macro}_TF(cfg_before);\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "    {macro}_TS(); {macro}_TF(\"cfgA=\");\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "    {prefix}_trace_append_config(active, active_count, buf, cap, &len);\n",
        prefix = prefix,
    ));
    // ent/ext are RAW accumulated comma-lists (the union across every
    // transition fired this step). Sort them HERE (the C analogue of the
    // Rust projection's `sorted_csv`) — both sides sort because neither
    // engine's emission order is canonical; behaviour is the SET.
    s.push_str(&format!(
        "    {macro}_TS(); {macro}_TF(\"ent=\");\n",
        macro = macro_prefix,
    ));
    s.push_str("    fsm_trace_emit_sorted_csv(buf, cap, &len, ent);\n");
    s.push_str(&format!(
        "    {macro}_TS(); {macro}_TF(\"ext=\");\n",
        macro = macro_prefix,
    ));
    s.push_str("    fsm_trace_emit_sorted_csv(buf, cap, &len, ext);\n");
    s.push_str("    buf[len++] = '\\n';\n");
    s.push_str("    buf[len] = '\\0';\n");
    s.push_str("    fsm_trace_emit(buf);\n");
    s.push_str(&format!("    #undef {macro}_TF\n", macro = macro_prefix));
    s.push_str(&format!("    #undef {macro}_TS\n", macro = macro_prefix));
    s.push_str("}\n");
    s.push_str("#endif /* FSM_TRACE */\n");
    s
}

/// Emit the `#ifdef FSM_TRACE` scratch fields for the machine struct
/// (header.rs). These hold the "transition the C fired this step" literals
/// + the config-before snapshot, set by the dispatch body, read by the
/// step emitter. `#ifndef FSM_TRACE` ⇒ the struct is byte-identical.
pub fn emit_trace_struct_fields() -> String {
    let mut s = String::new();
    s.push_str("#ifdef FSM_TRACE\n");
    s.push_str("    /* W1 trace-hook scratch (compile-time-gated). Holds the\n");
    s.push_str("     * transition THIS C fired this step + the config-before\n");
    s.push_str("     * snapshot. Set by the dispatch body, read by the step\n");
    s.push_str("     * emitter. Zero-width when FSM_TRACE is undefined. */\n");
    // tr/src/dst = the PRIMARY transition. LAST-write-wins across the
    // dispatch walk yields the lowest-slot region's transition = the
    // simulator's `primary = selected[0]` (see emit_trace_record_transition
    // for the slot-order reasoning). For a single-transition step,
    // last == only.
    s.push_str("    const char *_trace_tr;\n");
    s.push_str("    const char *_trace_src;\n");
    s.push_str("    const char *_trace_dst;\n");
    // ent/ext ACCUMULATE the union across every transition fired in the
    // step (the simulator's `entered_all`/`exited_all` aggregate over the
    // `selected` loop). Comma-separated; the projection sorts both sides.
    s.push_str("    char _trace_ent[256];\n");
    s.push_str("    char _trace_ext[256];\n");
    s.push_str("    char _trace_cfg_before[256];\n");
    // Set true by the defer-RELEASE drain immediately before it
    // re-dispatches a previously-held event, so the step emitter tags it
    // `event_redispatched` (the simulator's `StepKind::EventRedispatched`)
    // rather than `dispatched`. Captured + cleared at step-begin so a
    // NESTED completion dispatch within the redispatch is NOT mis-tagged.
    // This records an observed fact (the C IS releasing a deferred event),
    // not a re-derivation of defer semantics.
    s.push_str("    bool _trace_redispatch;\n");
    s.push_str("    bool _trace_is_redispatch;\n");
    s.push_str("#endif /* FSM_TRACE */\n");
    s
}

/// Emit (inside the dispatch body, gated) the reset of the per-step trace
/// scratch + the config-before snapshot. Called at the very top of
/// `<M>_dispatch`, before the leaf-to-root walk.
pub fn emit_trace_step_begin(ctx: &MachineEmitCtx<'_>, indent: &str) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::new();
    s.push_str(&format!("{indent}#ifdef FSM_TRACE\n", indent = indent));
    s.push_str(&format!(
        "{indent}m->_trace_tr = \"-\"; m->_trace_src = \"-\"; m->_trace_dst = \"-\";\n",
        indent = indent,
    ));
    s.push_str(&format!(
        "{indent}m->_trace_ent[0] = '\\0'; m->_trace_ext[0] = '\\0';\n",
        indent = indent,
    ));
    // Capture-and-consume the redispatch flag: THIS step is the released
    // deferred event (tag it `event_redispatched`); a nested completion
    // dispatch it triggers must NOT inherit the tag.
    s.push_str(&format!(
        "{indent}m->_trace_is_redispatch = m->_trace_redispatch;\n",
        indent = indent,
    ));
    s.push_str(&format!(
        "{indent}m->_trace_redispatch = false;\n",
        indent = indent,
    ));
    // Snapshot config-before: map the C's OWN _active[] → IR ids, sorted,
    // into the scratch buffer. The active array + count are passed as
    // arguments (no macro trick — the helper has one correct definition).
    s.push_str(&format!(
        "{indent}{{\n{indent}    size_t _tl = 0;\n",
        indent = indent,
    ));
    s.push_str(&format!(
        "{indent}    {prefix}_trace_append_config(m->_active, m->_active_count, m->_trace_cfg_before, sizeof(m->_trace_cfg_before) - 1, &_tl);\n",
        prefix = prefix,
        indent = indent,
    ));
    s.push_str(&format!(
        "{indent}    m->_trace_cfg_before[_tl] = '\\0';\n",
        indent = indent,
    ));
    s.push_str(&format!("{indent}}}\n", indent = indent));
    s.push_str(&format!(
        "{indent}#endif /* FSM_TRACE */\n",
        indent = indent
    ));
    s
}

/// Emit (inside `emit_transition_body`, gated, at the `return true` site)
/// the record of *which transition the generated C just fired*. The
/// `stable_id`/`source`/`target` are the compile-time literals of the
/// transition the C's own dispatch selected — its own decision, captured,
/// not re-derived.
pub fn emit_trace_record_transition(
    t: &fsm_ir::TransitionObject,
    entered_ir: &[String],
    exited_ir: &[String],
    indent: &str,
) -> String {
    let mut s = String::new();
    s.push_str(&format!("{indent}#ifdef FSM_TRACE\n", indent = indent));
    // Primary transition = the simulator's `primary = selected[0]`, i.e.
    // the transition for `active_states[0]` (region 0 / the lowest slot —
    // region 0 is entered first so it is first in the simulator's
    // `active_states`). The generated dispatch walks regions HIGH-slot →
    // low-slot (`for r = active_count-1 downto 0`), so slot 0 is processed
    // LAST. Therefore **LAST-write-wins** yields exactly the lowest-slot
    // region's transition = the simulator's `selected[0]`. For the common
    // single-transition step, last == only (identical). `_trace_has` still
    // records "a transition fired this step" for the emitter.
    s.push_str(&format!(
        "{indent}m->_trace_tr = {tr};\n",
        indent = indent,
        tr = c_string_literal(&t.stable_id),
    ));
    s.push_str(&format!(
        "{indent}m->_trace_src = {src};\n",
        indent = indent,
        src = c_string_literal(&t.source),
    ));
    s.push_str(&format!(
        "{indent}m->_trace_dst = {dst};\n",
        indent = indent,
        dst = c_string_literal(&t.target),
    ));
    // ACCUMULATE entered/exited (the simulator aggregates `entered_all`/
    // `exited_all` across the `selected` loop). Raw comma-append here; the
    // step emitter sorts the whole union before printing (mirroring the
    // Rust projection's `sorted_csv`). These IR ids are THIS code's own
    // `entry_path`/`exit_path` decisions — the cross-check target, not a
    // re-derivation of the simulator.
    let prefix = "TRACE"; // helper is machine-agnostic; named per macro below
    let _ = prefix;
    for id in entered_ir {
        s.push_str(&format!(
            "{indent}fsm_trace_csv_append(m->_trace_ent, sizeof(m->_trace_ent), {lit});\n",
            indent = indent,
            lit = c_string_literal(id),
        ));
    }
    for id in exited_ir {
        s.push_str(&format!(
            "{indent}fsm_trace_csv_append(m->_trace_ext, sizeof(m->_trace_ext), {lit});\n",
            indent = indent,
            lit = c_string_literal(id),
        ));
    }
    s.push_str(&format!(
        "{indent}#endif /* FSM_TRACE */\n",
        indent = indent
    ));
    s
}

/// Emit (inside the dispatch body, gated) the step-line emission, AFTER the
/// leaf-to-root walk and BEFORE `<M>_handle_completion` (so the ordering
/// matches the simulator: this event's record, then completion is a
/// separate queued event → a separate record). `kind_expr` and `evt_expr`
/// are C expressions deriving the StepKind/event-name from the event id the
/// C is processing (a projection of the event the C received, not a
/// re-derivation of semantics).
pub fn emit_trace_step_emit(ctx: &MachineEmitCtx<'_>, indent: &str) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!("{indent}#ifdef FSM_TRACE\n", indent = indent));
    // Derive kind + event-name from the event id the C is processing. This
    // is a projection of WHICH event the C received (it already has `ev`),
    // mirroring the simulator's `kind_for_event`/`event_received_for`.
    s.push_str(&format!(
        "{indent}{{\n{indent}    const char *_k; const char *_e;\n",
        indent = indent,
    ));
    s.push_str(&format!(
        "{indent}    if (ev->id == {macro}_EVENT__COMPLETION) {{ _k = \"completion\"; _e = \"__completion__\"; }}\n",
        indent = indent,
        macro = macro_prefix,
    ));
    // Timer events → timer_fired + __timer__:<timer_id>. Each timer's
    // distinct event variant maps to its IR timer id (the simulator uses
    // `__timer__:<timer_id>`).
    let timers = super::timer::collect_timers(ctx);
    for tm in &timers {
        s.push_str(&format!(
            "{indent}    else if (ev->id == {macro}_EVENT_{esuf}) {{ _k = \"timer_fired\"; _e = \"__timer__:{tid}\"; }}\n",
            indent = indent,
            macro = macro_prefix,
            esuf = tm.event_suffix,
            tid = tm.timer_id,
        ));
    }
    // Otherwise a normal dispatched external event. The simulator's
    // `kind_for_event` checks `ev.redispatched` FIRST (a released-deferred
    // event reprocesses as `EventRedispatched`, Doc 08 §10.2) — and that
    // flag is only ever set on dispatched/raised events, never timer/
    // completion. Mirror exactly: a redispatch (the C's defer-release
    // drain set `_trace_is_redispatch`) → `event_redispatched`; else
    // `dispatched`. Event name unchanged either way.
    s.push_str(&format!(
        "{indent}    else {{ _k = m->_trace_is_redispatch ? \"event_redispatched\" : \"dispatched\"; _e = {prefix}_trace_event_name(ev->id); }}\n",
        indent = indent,
        prefix = prefix,
    ));
    // Emit the line. cfgA is the C's OWN _active[] snapshot taken NOW
    // (passed as args), cfgB the snapshot taken at step-begin.
    s.push_str(&format!(
        "{indent}    {prefix}_trace_step(m->_active, m->_active_count, _k, _e,\n",
        indent = indent,
        prefix = prefix,
    ));
    s.push_str(&format!(
        "{indent}        m->_trace_tr, m->_trace_src, m->_trace_dst,\n",
        indent = indent,
    ));
    s.push_str(&format!(
        "{indent}        m->_trace_cfg_before,\n",
        indent = indent,
    ));
    s.push_str(&format!(
        "{indent}        m->_trace_ent, m->_trace_ext);\n",
        indent = indent,
    ));
    s.push_str(&format!("{indent}}}\n", indent = indent));
    s.push_str(&format!(
        "{indent}#endif /* FSM_TRACE */\n",
        indent = indent
    ));
    s
}

/// Emit the gated `init` step line at the end of `<M>_init` (after the
/// entry sequence, before `<M>_handle_completion` — matching the
/// simulator's order: the init record, then the internal queue drains).
pub fn emit_trace_init_emit(
    ctx: &MachineEmitCtx<'_>,
    entered_ir: &[String],
    indent: &str,
) -> String {
    let prefix = ctx.type_prefix();
    let mut ent_sorted: Vec<&str> = entered_ir.iter().map(String::as_str).collect();
    ent_sorted.sort_unstable();

    let mut s = String::new();
    s.push_str(&format!("{indent}#ifdef FSM_TRACE\n", indent = indent));
    s.push_str(&format!(
        "{indent}{prefix}_trace_step(m->_active, m->_active_count, \"init\", \"-\",\n",
        indent = indent,
        prefix = prefix,
    ));
    s.push_str(&format!(
        "{indent}    \"-\", \"-\", \"-\",\n",
        indent = indent,
    ));
    // configBefore for the init record is empty (the simulator's init
    // record has `config_before: Vec::new()`).
    s.push_str(&format!("{indent}    \"\",\n", indent = indent,));
    s.push_str(&format!(
        "{indent}    {ent}, \"\");\n",
        indent = indent,
        ent = c_string_literal(&ent_sorted.join(",")),
    ));
    s.push_str(&format!(
        "{indent}#endif /* FSM_TRACE */\n",
        indent = indent
    ));
    s
}

/// Emit the gated `event_deferred` step line (Doc 08 §10.1). Used at the
/// defer-hold site, just before the held event's early `return;`. Mirrors
/// the simulator's `StepKind::EventDeferred` record: the event was held,
/// no transition fired, the configuration is unchanged
/// (`configBefore == configAfter`). The event name is derived from the
/// event the C is holding (`ev`) — a projection of the held event, not a
/// re-derivation of defer semantics (the C already decided to defer; this
/// only records that observed fact).
pub fn emit_trace_deferred_emit(ctx: &MachineEmitCtx<'_>, indent: &str) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::new();
    s.push_str(&format!("{indent}#ifdef FSM_TRACE\n", indent = indent));
    s.push_str(&format!(
        "{indent}{prefix}_trace_step(m->_active, m->_active_count, \"event_deferred\",\n",
        indent = indent,
        prefix = prefix,
    ));
    s.push_str(&format!(
        "{indent}    {prefix}_trace_event_name(ev->id), \"-\", \"-\", \"-\",\n",
        indent = indent,
        prefix = prefix,
    ));
    s.push_str(&format!(
        "{indent}    m->_trace_cfg_before, \"\", \"\");\n",
        indent = indent,
    ));
    s.push_str(&format!(
        "{indent}#endif /* FSM_TRACE */\n",
        indent = indent
    ));
    s
}

/// Emit the gated event-id → name lookup helper (`<M>_trace_event_name`)
/// for the `.c`. Maps the C `EventId` enum back to the DSL event name (the
/// simulator's `event_received.name` for a dispatched/raised event). Pure
/// generated data, no semantics.
pub fn emit_trace_event_name_fn(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str("#ifdef FSM_TRACE\n");
    s.push_str(&format!(
        "static const char *{prefix}_trace_event_name({prefix}_EventId_t id) {{\n",
        prefix = prefix,
    ));
    s.push_str("    switch (id) {\n");
    for ev in &ctx.machine.events {
        let ev_c = crate::state_index::c_ident(&ev.name);
        s.push_str(&format!(
            "    case {macro}_EVENT_{ec}: return {lit};\n",
            macro = macro_prefix,
            ec = ev_c,
            lit = c_string_literal(&ev.name),
        ));
    }
    s.push_str("    default: return \"?\";\n");
    s.push_str("    }\n");
    s.push_str("}\n");
    s.push_str("#endif /* FSM_TRACE */\n");
    s
}

/// Render a Rust string as a C99 double-quoted string literal with the
/// minimal escaping the generated ids/names need (`\`, `"`). IR ids and
/// event names are `[A-Za-z0-9:_-]`-shaped so this is conservative but
/// total.
pub(crate) fn c_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_string_literal_escapes_minimally() {
        assert_eq!(c_string_literal("s-Motor-Idle"), "\"s-Motor-Idle\"");
        assert_eq!(c_string_literal(""), "\"\"");
        assert_eq!(c_string_literal("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn field_sep_is_unit_separator() {
        assert_eq!(FIELD_SEP as u32, 0x1f);
    }
}
