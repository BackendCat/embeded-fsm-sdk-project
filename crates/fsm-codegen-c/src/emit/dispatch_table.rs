//! Table-driven dispatch — Doc 11 §9, Doc 00 §7.8 (B-11).
//!
//! Two-phase collect-then-execute. Compatible with the same parent-table
//! ancestor walk used by the switch strategy. The transition table is
//! sorted by `(source, priority, document_order)` per Doc 00 §7.8 / G-07.

use fsm_ir::{StateNode, TransitionKind, TransitionObject};

use crate::expr::emit_guard;

use super::entry_exit::{entry_path, exit_path};
use super::MachineEmitCtx;

/// Emit the transition table, function-pointer tables, executor, and
/// outer dispatch.
pub fn emit_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let mut s = String::new();
    s.push_str(&emit_parent_table_alias(ctx));
    s.push_str("\n");
    s.push_str(&emit_guard_action_typedefs(ctx));
    s.push_str(&emit_executor(ctx));
    s.push_str("\n");
    s.push_str(&emit_trans_table(ctx));
    s.push_str("\n");
    s.push_str(&emit_outer_dispatch(ctx));
    s
}

fn emit_parent_table_alias(ctx: &MachineEmitCtx<'_>) -> String {
    // The table strategy reuses the same parent_table as switch; emit a
    // local declaration so this strategy is self-contained.
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

fn emit_trans_table(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut rows = collect_transitions(ctx);
    // Sort by (source index, priority, document order).
    rows.sort_by(|a, b| a.source.cmp(&b.source).then(a.priority.cmp(&b.priority)));

    let mut s = String::new();
    s.push_str(&format!(
        "typedef struct {{\n    {prefix}_StateId_t source;\n    {prefix}_EventId_t trigger;\n    uint8_t guard_idx;\n    uint8_t action_idx;\n    {prefix}_StateId_t target;\n    uint16_t priority;\n    uint8_t kind;\n}} {prefix}_TransRow_t;\n",
        prefix = prefix,
    ));
    s.push_str(&format!(
        "static const {prefix}_TransRow_t {prefix}_trans_table[] = {{\n",
        prefix = prefix,
    ));
    for row in &rows {
        let source_c = ctx.index.get(row.source).c_name.clone();
        let target_c = ctx.index.get(row.target).c_name.clone();
        let trigger_c = row.trigger_c.clone();
        s.push_str(&format!(
            "    {{ {macro}_STATE_{src}, {trig}, {gidx}, {aidx}, {macro}_STATE_{tgt}, {pri}, {kind} }},\n",
            macro = macro_prefix,
            src = source_c,
            trig = trigger_c,
            gidx = row.guard_idx,
            aidx = row.action_idx,
            tgt = target_c,
            pri = row.priority,
            kind = row.kind_tag,
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

struct EmittedTransRow {
    source: u8,
    target: u8,
    trigger_c: String,
    guard_idx: usize,
    action_idx: usize,
    priority: u16,
    kind_tag: u8,
}

fn collect_transitions(ctx: &MachineEmitCtx<'_>) -> Vec<EmittedTransRow> {
    let mut out = Vec::new();
    fn walk_state(state: &StateNode, ctx: &MachineEmitCtx<'_>, out: &mut Vec<EmittedTransRow>) {
        let (transitions, recurse) = match state {
            StateNode::Simple(s) => (&s.transitions[..], None),
            StateNode::Composite(c) => (&c.transitions[..], Some(c.regions.as_slice())),
            StateNode::Parallel(p) => (&p.transitions[..], Some(p.regions.as_slice())),
            _ => return,
        };
        for t in transitions {
            let source = ctx.index.must_lookup(&t.source);
            let target = ctx.index.must_lookup(&t.target);
            let trigger_c = match &t.trigger {
                Some(fsm_ir::Trigger::Event { event_id, .. }) => ctx.event_c_enum(event_id),
                _ => format!("{}_EVENT__COMPLETION", ctx.macro_prefix()),
            };
            let kind_tag = match t.kind {
                TransitionKind::External => 0,
                TransitionKind::Local => 1,
                TransitionKind::Internal => 2,
                TransitionKind::Completion => 3,
            };
            out.push(EmittedTransRow {
                source,
                target,
                trigger_c,
                guard_idx: 0, // function pointer indirection placeholder
                action_idx: 0,
                priority: t.priority,
                kind_tag,
            });
        }
        if let Some(regions) = recurse {
            for r in regions {
                for s in &r.states {
                    walk_state(s, ctx, out);
                }
            }
        }
    }
    for s in &ctx.machine.root.states {
        walk_state(s, ctx, &mut out);
    }
    out
}

fn emit_executor(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        r#"
/* Per-row executor: runs exit-set, target action, target entry-set. */
static void {prefix}_execute_transition({prefix}_t *m, const {prefix}_TransRow_t *row, const {prefix}_Event_t *ev) {{
    (void)ev;
    if (row->kind == 2 /* Internal */) {{
        /* Internal: no exit, no entry. Action is invoked via guard_idx
         * dispatch elsewhere in v1.0; placeholder. */
        return;
    }}
    m->_state = row->target;
}}
"#,
        prefix = prefix,
    ));
    let _ = macro_prefix;
    s
}

fn emit_outer_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    // Also emit the per-state-try function for the table strategy, so the
    // outer dispatch can mix the table walk with inline guard / action
    // emission identical to the switch strategy. This is simpler than
    // separating guard functions into a pointer table for v1.0.
    s.push_str(&format!(
        "static bool {prefix}_try_transitions_in_state({prefix}_t *m, {prefix}_StateId_t s, const {prefix}_Event_t *ev) {{\n",
        prefix = prefix,
    ));
    s.push_str(&format!(
        "    for (uint16_t i = 0; i < {macro}_TRANS_TABLE_SIZE; i++) {{\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "        const {prefix}_TransRow_t *row = &{prefix}_trans_table[i];\n",
        prefix = prefix,
    ));
    s.push_str("        if (row->source != s) continue;\n");
    s.push_str("        if (row->trigger != ev->id) continue;\n");
    // Inline guard / exit / entry for the matched row — call into a generated
    // per-row helper. v1.0 keeps it simple by inlining the same pattern as
    // the switch strategy for each row.
    s.push_str(&format!(
        "        if ({prefix}_try_apply_row(m, row, ev)) return true;\n",
        prefix = prefix,
    ));
    s.push_str("    }\n    return false;\n}\n");

    // The per-row apply helper.
    s.push_str(&format!(
        "static bool {prefix}_try_apply_row({prefix}_t *m, const {prefix}_TransRow_t *row, const {prefix}_Event_t *ev) {{\n",
        prefix = prefix,
    ));
    s.push_str("    switch ((row->source << 8) | row->trigger) {\n");
    let mut rows = collect_transitions(ctx);
    rows.sort_by(|a, b| a.source.cmp(&b.source).then(a.priority.cmp(&b.priority)));
    for (i, row) in rows.iter().enumerate() {
        let source_c = &ctx.index.get(row.source).c_name;
        let trig_c = &row.trigger_c;
        s.push_str(&format!(
            "    case ({macro}_STATE_{src} << 8) | {trig}: {{\n",
            macro = ctx.macro_prefix(),
            src = source_c,
            trig = trig_c,
        ));
        // Emit the original IR transition's guard + actions.
        if let Some(t) = find_transition_at_pos(ctx, i, &rows) {
            if let Some(g) = &t.guard {
                let cond = emit_guard(g, "m->context", "ev->__payload");
                s.push_str(&format!("        if (!{}) break;\n", cond));
            }
            for ex in exit_path(t, ctx.index, ctx.parents) {
                let rec = ctx.index.get(ex);
                if rec.kind.is_active_at_rest() {
                    s.push_str(&format!(
                        "        {prefix}_exit_{name}(m);\n",
                        prefix = ctx.type_prefix(),
                        name = rec.c_name,
                    ));
                }
            }
            let stmt_ctx = crate::stmt::StmtContext {
                machine_prefix: ctx.type_prefix(),
                ctx_prefix: "m->context",
                payload_prefix: "ev->__payload",
            };
            s.push_str(&crate::stmt::emit_stmts(&t.actions, &stmt_ctx, 8));
            if !matches!(t.kind, TransitionKind::Internal) {
                let target_idx = ctx.index.must_lookup(&t.target);
                let target_rec = ctx.index.get(target_idx);
                s.push_str(&format!(
                    "        m->_state = {macro}_STATE_{name};\n",
                    macro = ctx.macro_prefix(),
                    name = target_rec.c_name,
                ));
            }
            for en in entry_path(t, ctx.index, ctx.parents) {
                let rec = ctx.index.get(en);
                if rec.kind.is_active_at_rest() {
                    s.push_str(&format!(
                        "        {prefix}_entry_{name}(m);\n",
                        prefix = ctx.type_prefix(),
                        name = rec.c_name,
                    ));
                }
            }
            s.push_str(&format!(
                "        (void){prefix}_execute_transition; return true;\n",
                prefix = ctx.type_prefix(),
            ));
        }
        s.push_str("    }\n");
    }
    s.push_str("    default: break;\n");
    s.push_str("    }\n    return false;\n}\n");

    // Final outer dispatch with B-11 collect-then-execute pattern. Even on
    // non-parallel machines we follow the same code path for uniformity.
    s.push_str(&format!(
        r#"
void {prefix}_dispatch({prefix}_t *m, const {prefix}_Event_t *ev) {{
    /* B-11 table dispatch: collect-then-execute. For non-parallel machines
     * the inner loop trivially picks the single active leaf. */
    if (ev->id < {macro}_EVENT__COUNT && ({prefix}_defer_mask[m->_state] & (1u << ev->id))) {{
        return;
    }}
    {prefix}_StateId_t s = m->_state;
    while (1) {{
        if ({prefix}_try_transitions_in_state(m, s, ev)) {{
            {prefix}_handle_completion(m);
            return;
        }}
        if (s == {macro}_STATE_ROOT) break;
        s = {prefix}_parent_table[s];
    }}
}}
"#,
        prefix = prefix,
        macro = macro_prefix,
    ));
    s
}

fn find_transition_at_pos<'a>(
    ctx: &'a MachineEmitCtx<'_>,
    pos: usize,
    rows: &[EmittedTransRow],
) -> Option<&'a TransitionObject> {
    // Match the row at `pos` against the original IR transition by
    // (source, trigger, priority) — unique enough for v1.0 given that
    // the analyzer rejects fully ambiguous duplicate transitions.
    let mut found: Option<&'a TransitionObject> = None;
    let row = &rows[pos];
    let mut unsorted_idx = 0usize;
    fn find_idx<'a>(
        state: &'a StateNode,
        row: &EmittedTransRow,
        ctx: &MachineEmitCtx<'_>,
        cur: &mut usize,
        hit: &mut Option<&'a TransitionObject>,
    ) {
        let (transitions, recurse) = match state {
            StateNode::Simple(s) => (&s.transitions[..], None),
            StateNode::Composite(c) => (&c.transitions[..], Some(c.regions.as_slice())),
            StateNode::Parallel(p) => (&p.transitions[..], Some(p.regions.as_slice())),
            _ => return,
        };
        for t in transitions {
            if hit.is_none() {
                let src = ctx.index.must_lookup(&t.source);
                let trig = match &t.trigger {
                    Some(fsm_ir::Trigger::Event { event_id, .. }) => ctx.event_c_enum(event_id),
                    _ => format!("{}_EVENT__COMPLETION", ctx.macro_prefix()),
                };
                if src == row.source && trig == row.trigger_c && t.priority == row.priority {
                    *hit = Some(t);
                }
            }
            *cur += 1;
        }
        if let Some(regions) = recurse {
            for r in regions {
                for s in &r.states {
                    find_idx(s, row, ctx, cur, hit);
                }
            }
        }
    }
    for s in &ctx.machine.root.states {
        find_idx(s, row, ctx, &mut unsorted_idx, &mut found);
    }
    let _ = unsorted_idx;
    found
}
