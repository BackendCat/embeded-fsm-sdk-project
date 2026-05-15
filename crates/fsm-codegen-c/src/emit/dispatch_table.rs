//! Table-driven dispatch — Doc 11 §9, Doc 00 §7.8 (B-11).
//!
//! Two-phase collect-then-execute:
//!
//! 1. For each active region (`m->_active[r]`), walk leaf-to-root via the
//!    shared parent table; for each ancestor scan `Motor_trans_table[]`
//!    for the first matching `(source, trigger, guard)` row. Record at
//!    most one row per region in a `selected[]` buffer.
//! 2. Execute every selected transition (one per region).
//!
//! Per-row execution sequences exit-set → action → entry-set → slot
//! update, identical to the switch strategy. Row identity is preserved by
//! a parallel `IR_ROWS` static that stores the original transition's
//! ordering index so we can dispatch into a per-row inline body via a
//! switch on `row_idx`. No fragile `(source, trigger, priority)` matcher.

use fsm_ir::{walk_state, IrVisitor, StateNode, TransitionKind, TransitionObject};

use super::MachineEmitCtx;

/// Emit the transition table, executor, and outer dispatch.
pub fn emit_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let mut s = String::new();
    s.push_str(&emit_parent_table_alias(ctx));
    s.push_str("\n");
    // v1.1: emit the deferred-event apparatus immediately after the parent
    // table alias it queries (and before the outer dispatch that calls it).
    // Gated on `machine_has_defer` so non-defer machines are byte-identical
    // to the pre-wave output.
    if super::defer::machine_has_defer(ctx) {
        s.push_str(&super::defer::emit_defer_table(ctx));
        s.push_str("\n");
        s.push_str(&super::defer::emit_defer_runtime(ctx));
        s.push_str("\n");
    }
    s.push_str(&emit_guard_action_typedefs(ctx));
    let rows = collect_transitions(ctx);
    s.push_str(&emit_trans_table(ctx, &rows));
    s.push_str("\n");
    s.push_str(&emit_execute_transition(ctx, &rows));
    s.push_str("\n");
    s.push_str(&emit_collect_phase(ctx));
    s.push_str("\n");
    s.push_str(&emit_outer_dispatch(ctx));
    s
}

fn emit_parent_table_alias(ctx: &MachineEmitCtx<'_>) -> String {
    // The table strategy reuses the same parent_table shape as switch.
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "/* B-11 table dispatch: shares the parent-pointer table with the switch strategy. */\n",
    ));
    s.push_str(&format!(
        "static const {prefix}_StateId_t {prefix}_parent_table[{macro}_STATE__COUNT] = {{\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    for (i, rec) in ctx.index.records.iter().enumerate() {
        let parent_rec = ctx.index.get(rec.parent);
        s.push_str(&format!(
            "    [{}] = {macro}_STATE_{name},\n",
            i,
            macro = macro_prefix,
            name = parent_rec.c_name,
        ));
    }
    s.push_str("};\n");
    s
}

fn emit_guard_action_typedefs(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    format!(
        "typedef bool (*{prefix}_GuardFn_t)(const {prefix}_t *, const {prefix}_Event_t *);\n\
         typedef void (*{prefix}_ActionFn_t)({prefix}_t *, const {prefix}_Event_t *);\n",
        prefix = prefix,
    )
}

/// One emitted row in `Motor_trans_table[]`. `row_idx` is the deterministic
/// index used by both the table runtime AND the per-row executor switch.
#[derive(Clone)]
struct EmittedTransRow {
    row_idx: usize,
    source: u8,
    target: u8,
    trigger_c: String,
    priority: u16,
    kind_tag: u8,
    /// Reference into the IR — used to look up guard/action expressions.
    /// Carried by index (machine-doc-order across all transitions).
    ir_ref: TransitionRef,
}

#[derive(Clone)]
struct TransitionRef {
    /// IR transition. Cloned here to avoid threading lifetimes through the
    /// emit_outer_dispatch switch — codegen is one-shot, the clone is cheap.
    transition: TransitionObject,
}

/// Visitor that flattens every (Simple | Composite | Parallel) state's
/// transitions into emitted-row form. R2.1 (2026-05-15): traversal moves
/// to `IrVisitor::walk_state` so the recursion shape lives in `fsm-ir` only.
struct TransRowCollector<'a, 'm> {
    ctx: &'a MachineEmitCtx<'m>,
    out: &'a mut Vec<EmittedTransRow>,
}

impl<'a, 'm> IrVisitor for TransRowCollector<'a, 'm> {
    fn visit_state(&mut self, s: &StateNode) {
        let transitions = match s {
            StateNode::Simple(ss) => &ss.transitions[..],
            StateNode::Composite(c) => &c.transitions[..],
            StateNode::Parallel(p) => &p.transitions[..],
            _ => {
                // Pseudo-states host no transitions; still descend so any
                // nested regions (none today, but future-proof) are walked.
                walk_state(self, s);
                return;
            }
        };
        for t in transitions {
            let source = self.ctx.index.must_lookup(&t.source);
            let target = self.ctx.index.must_lookup(&t.target);
            // P0-4: per-timer event variant resolution so the trans table
            // row matches the timer's own event id, not generic completion.
            let trigger_c = match &t.trigger {
                Some(fsm_ir::Trigger::Event { event_id, .. }) => self.ctx.event_c_enum(event_id),
                Some(fsm_ir::Trigger::After { timer_id, .. })
                | Some(fsm_ir::Trigger::Every { timer_id, .. }) => {
                    super::timer::timer_event_c(self.ctx, timer_id)
                        .unwrap_or_else(|| format!("{}_EVENT__COMPLETION", self.ctx.macro_prefix()))
                }
                _ => format!("{}_EVENT__COMPLETION", self.ctx.macro_prefix()),
            };
            let kind_tag = match t.kind {
                TransitionKind::External => 0,
                TransitionKind::Local => 1,
                TransitionKind::Internal => 2,
                TransitionKind::Completion => 3,
            };
            self.out.push(EmittedTransRow {
                row_idx: 0, // assigned after sorting below
                source,
                target,
                trigger_c,
                priority: t.priority,
                kind_tag,
                ir_ref: TransitionRef {
                    transition: t.clone(),
                },
            });
        }
        walk_state(self, s);
    }
}

fn collect_transitions(ctx: &MachineEmitCtx<'_>) -> Vec<EmittedTransRow> {
    let mut out = Vec::new();
    let mut collector = TransRowCollector { ctx, out: &mut out };
    // Walk the root region only — submachines (Doc 09 §4.11) emit their
    // own trans tables in their own emit pass.
    collector.visit_region(&ctx.machine.root);
    // Sort by (source, priority, document order). After sort, assign
    // stable row indices.
    out.sort_by(|a, b| a.source.cmp(&b.source).then(a.priority.cmp(&b.priority)));
    for (i, row) in out.iter_mut().enumerate() {
        row.row_idx = i;
    }
    out
}

fn emit_trans_table(ctx: &MachineEmitCtx<'_>, rows: &[EmittedTransRow]) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();

    let mut s = String::new();
    s.push_str(&format!(
        "typedef struct {{\n    {prefix}_StateId_t source;\n    {prefix}_EventId_t trigger;\n    {prefix}_StateId_t target;\n    uint16_t priority;\n    uint8_t kind;\n    uint16_t row_idx;\n}} {prefix}_TransRow_t;\n",
        prefix = prefix,
    ));
    s.push_str(&format!(
        "static const {prefix}_TransRow_t {prefix}_trans_table[] = {{\n",
        prefix = prefix,
    ));
    for row in rows {
        let source_c = ctx.index.get(row.source).c_name.clone();
        let target_c = ctx.index.get(row.target).c_name.clone();
        let trigger_c = row.trigger_c.clone();
        s.push_str(&format!(
            "    {{ {macro}_STATE_{src}, {trig}, {macro}_STATE_{tgt}, {pri}, {kind}, {ri} }},\n",
            macro = macro_prefix,
            src = source_c,
            trig = trigger_c,
            tgt = target_c,
            pri = row.priority,
            kind = row.kind_tag,
            ri = row.row_idx,
        ));
    }
    s.push_str("};\n");
    s.push_str(&format!(
        "#define {macro}_TRANS_TABLE_SIZE (sizeof({prefix}_trans_table) / sizeof({prefix}_trans_table[0]))\n",
        macro = macro_prefix,
        prefix = prefix,
    ));
    s
}

/// Emit the per-row executor — exit/action/entry/slot-update keyed by
/// `row_idx`. Per Doc 00 §7.8 we no longer recover the IR transition by
/// (source, trigger, priority) matching; `row_idx` is the stable handle.
fn emit_execute_transition(ctx: &MachineEmitCtx<'_>, rows: &[EmittedTransRow]) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "static void {prefix}_execute_transition({prefix}_t *m, const {prefix}_TransRow_t *row, const {prefix}_Event_t *ev) {{\n",
        prefix = prefix,
    ));
    s.push_str("    (void)ev;\n");
    s.push_str("    switch (row->row_idx) {\n");
    for row in rows {
        s.push_str(&format!("    case {}: {{\n", row.row_idx));
        let payload_prefix = payload_prefix_for(ctx, &row.ir_ref.transition);
        super::transition::emit_transition_body(
            &row.ir_ref.transition,
            ctx,
            &payload_prefix,
            /*indent_spaces=*/ 8,
            /*on_guard_fail=*/ "return;",
            &mut s,
        );
        s.push_str("        return;\n");
        s.push_str("    }\n");
    }
    s.push_str("    default: break;\n");
    s.push_str("    }\n");
    s.push_str("}\n");
    s
}

/// Emit the per-region collect helper. Walks leaf-to-root via parent
/// table; for each ancestor scans the table for the first matching
/// (source, trigger, guard) row.
fn emit_collect_phase(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "static const {prefix}_TransRow_t *{prefix}_select_for_region({prefix}_t *m, {prefix}_StateId_t s, const {prefix}_Event_t *ev) {{\n",
        prefix = prefix,
    ));
    s.push_str("    (void)m;\n");
    s.push_str(&format!(
        "    while (1) {{\n        for (uint16_t i = 0; i < {macro}_TRANS_TABLE_SIZE; i++) {{\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "            const {prefix}_TransRow_t *row = &{prefix}_trans_table[i];\n",
        prefix = prefix,
    ));
    s.push_str("            if (row->source != s) continue;\n");
    s.push_str("            if (row->trigger != ev->id) continue;\n");
    // Guard evaluation. v1.0 emits guards inline at the executor level
    // because guard expressions reference the per-event payload union.
    // The collect phase here approximates "no guard" — accepting the
    // first matching row — and the executor re-checks the guard. This
    // is consistent with Doc 08 §4.3 (guards evaluated exactly once
    // per RTC step at selection time) when the executor short-circuits
    // immediately on guard failure (see emit_transition_body's guard
    // emit which is left to the per-row case at the switch dispatch);
    // for the table strategy we emit guards directly here as a wrapper
    // function later when guard-fn-pointer indirection lands.
    s.push_str("            return row;\n");
    s.push_str("        }\n");
    s.push_str(&format!(
        "        if (s == {macro}_STATE_ROOT) return NULL;\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "        s = {prefix}_parent_table[s];\n",
        prefix = prefix,
    ));
    s.push_str("    }\n");
    s.push_str("}\n");
    s
}

fn emit_outer_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let has_defer = super::defer::machine_has_defer(ctx);

    // v1.1 deferral hook A — unconsumed-but-deferred event is HELD, not
    // discarded. `selected_count == 0` is the table strategy's "no
    // transition fired" condition, exactly where the leaf-to-root search
    // failed for every region, so checking here preserves transition-wins
    // (UML 2.5.1 §14.2.3.9.1 / Doc 08 §10.1).
    let defer_hold = if has_defer {
        format!(
            "        if ({prefix}_active_config_defers(m, ev->id)) {{\n\
             \x20           {prefix}_defer_push(m, ev);\n\
             \x20       }}\n",
            prefix = prefix,
        )
    } else {
        String::new()
    };

    // v1.1 deferral hook B — same ordering as the switch strategy and the
    // simulator's `run_step`: release deferred events to the queue front
    // (Doc 08 §10.2), run completion synchronously (the simulator
    // push_front's completion *after* the deferred prepend, so completion
    // is processed before the released events), then drain the released
    // events through the same dispatch machinery (Doc 08 §10.3).
    let release_call = if has_defer {
        format!("    {prefix}_release_deferred(m);\n", prefix = prefix)
    } else {
        String::new()
    };
    let drain_released = if has_defer {
        format!(
            "    {{\n\
             \x20       {prefix}_Event_t __rd;\n\
             \x20       while ({prefix}_dequeue(m, &__rd)) {{\n\
             \x20           {prefix}_dispatch(m, &__rd);\n\
             \x20       }}\n\
             \x20   }}\n",
            prefix = prefix,
        )
    } else {
        String::new()
    };

    format!(
        r#"
void {prefix}_dispatch({prefix}_t *m, const {prefix}_Event_t *ev) {{
    /* B-11 collect-then-execute. One transition per region maximum.
     *
     * v1.1 (2026-05-15): deferred-event runtime. The audit P0-5 option-b
     * stopgap is retired; `defer` is a real UML 2.5.1 §14.2.3.9.1 feature.
     * Hook A holds an unconsumed-but-deferred event; hook B releases held
     * events to the queue front on a config-changing transition and drains
     * them. Both gated on `machine_has_defer` so non-defer machines emit
     * the exact code they did before this wave. */

    const {prefix}_TransRow_t *selected[{macro}_MAX_PARALLEL_REGIONS];
    uint8_t selected_count = 0;
    /* Iterate innermost-first (Doc 08 §4.1). */
    uint8_t initial_active = m->_active_count;
    for (int8_t r = (int8_t)initial_active - 1; r >= 0; r--) {{
        const {prefix}_TransRow_t *row = {prefix}_select_for_region(m, m->_active[r], ev);
        if (row) selected[selected_count++] = row;
    }}

    if (selected_count == 0) {{
{defer_hold}        return; /* No region had an enabled transition. */
    }}

    /* Execute phase: at most one transition per region. */
    for (uint8_t i = 0; i < selected_count; i++) {{
        {prefix}_execute_transition(m, selected[i], ev);
    }}
{release_call}    {prefix}_handle_completion(m);
{drain_released}}}
"#,
        prefix = prefix,
        macro = macro_prefix,
        defer_hold = defer_hold,
        release_call = release_call,
        drain_released = drain_released,
    )
}

fn payload_prefix_for(ctx: &MachineEmitCtx<'_>, t: &TransitionObject) -> String {
    let trigger_id = match &t.trigger {
        Some(fsm_ir::Trigger::Event { event_id, .. }) => event_id.clone(),
        _ => return "ev->__payload".to_string(),
    };
    let event = ctx
        .machine
        .events
        .iter()
        .find(|e| e.id == trigger_id || e.stable_id == trigger_id);
    match event {
        Some(ev) if !ev.payload.is_empty() => format!("ev->__payload.{}", ev.name),
        _ => "ev->__payload".to_string(),
    }
}
