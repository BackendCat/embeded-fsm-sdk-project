//! Switch-based dispatch — Doc 11 §8, Doc 00 §7.8 (B-10).
//!
//! Pattern:
//!  - Emit `Motor_parent_table[]: static const M_StateId_t[]`.
//!  - Emit per-state helper `Motor_try_transitions_in_state` returning
//!    `bool` (true if a transition fired).
//!  - Emit the outer `Motor_dispatch` that walks leaf-to-root using the
//!    parent table.

use fsm_ir::{StateNode, TransitionKind, TransitionObject};

use crate::expr::emit_guard;
use crate::state_index::{StateRecordKind, ROOT_SENTINEL};

use super::entry_exit::{entry_path, exit_path};
use super::MachineEmitCtx;

/// Emit the parent table + per-state helpers + outer dispatch loop.
pub fn emit_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let mut s = String::new();
    s.push_str(&emit_parent_table(ctx));
    s.push_str("\n");
    s.push_str(&emit_per_state_helpers(ctx));
    s.push_str("\n");
    s.push_str(&emit_outer_dispatch(ctx));
    s
}

fn emit_parent_table(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "/* B-10 hierarchical dispatch: parent-pointer table indexed by StateId. */\n",
    ));
    s.push_str(&format!(
        "static const {prefix}_StateId_t {prefix}_parent_table[{macro}_STATE__COUNT] = {{\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    for (i, rec) in ctx.index.records.iter().enumerate() {
        let parent_rec = ctx.index.get(rec.parent);
        s.push_str(&format!(
            "    [{}] = {macro}_STATE_{name}, /* {} */\n",
            i,
            rec.dsl_name,
            macro = macro_prefix,
            name = parent_rec.c_name,
        ));
    }
    s.push_str("};\n");
    s
}

fn emit_per_state_helpers(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "/* Per-state transition try function. Returns true if a transition fired. */\n",
    ));
    s.push_str(&format!(
        "static bool {prefix}_try_transitions_in_state({prefix}_t *m, {prefix}_StateId_t s, const {prefix}_Event_t *ev) {{\n",
        prefix = prefix,
    ));
    s.push_str("    switch (s) {\n");

    // Walk every state in the machine; emit a case for any state that has
    // transitions.
    fn walk_state(state: &StateNode, ctx: &MachineEmitCtx<'_>, out: &mut String) {
        let (id, transitions, recurse) = match state {
            StateNode::Simple(s) => (&s.id, &s.transitions, None),
            StateNode::Composite(c) => (&c.id, &c.transitions, Some(c.regions.as_slice())),
            StateNode::Parallel(p) => (&p.id, &p.transitions, Some(p.regions.as_slice())),
            _ => return,
        };
        if !transitions.is_empty() {
            let idx = ctx.index.must_lookup(id);
            let rec = ctx.index.get(idx);
            out.push_str(&format!(
                "    case {macro}_STATE_{name}: /* {dsl} */\n",
                macro = ctx.macro_prefix(),
                name = rec.c_name,
                dsl = rec.dsl_name,
            ));
            // Sort by priority (ascending = lower priority number wins per
            // Doc 08 §4.2), then document order. The IR already preserves
            // document order via Vec, so sort_by_key with priority alone
            // suffices.
            let mut sorted: Vec<&TransitionObject> = transitions.iter().collect();
            sorted.sort_by(|a, b| a.priority.cmp(&b.priority));
            // Group cases by trigger id for cleaner switch output.
            out.push_str("        switch (ev->id) {\n");
            for t in &sorted {
                emit_one_case(t, ctx, out);
            }
            out.push_str("        default: break;\n");
            out.push_str("        }\n");
            out.push_str("        break;\n");
        }
        if let Some(regions) = recurse {
            for r in regions {
                for s in &r.states {
                    walk_state(s, ctx, out);
                }
            }
        }
    }
    for state in &ctx.machine.root.states {
        walk_state(state, ctx, &mut s);
    }
    s.push_str("    default: break;\n");
    s.push_str("    }\n");
    s.push_str("    return false;\n");
    s.push_str("}\n");
    let _ = macro_prefix;
    s
}

fn emit_one_case(t: &TransitionObject, ctx: &MachineEmitCtx<'_>, out: &mut String) {
    let trigger_id = match &t.trigger {
        Some(fsm_ir::Trigger::Event { event_id, .. }) => event_id.clone(),
        // Completion / timer triggers come in as the reserved internal
        // event. We use the synthetic completion event ID here as a stand-in.
        Some(fsm_ir::Trigger::Completion { .. }) | None => {
            format!("{}_EVENT__COMPLETION", ctx.macro_prefix())
        }
        Some(fsm_ir::Trigger::After { .. }) | Some(fsm_ir::Trigger::Every { .. }) => {
            // Timers are dispatched via completion event with a per-timer
            // payload in this simplified codegen; the analyzer is the
            // authoritative resolver. We map to completion as the safe
            // catch-all.
            format!("{}_EVENT__COMPLETION", ctx.macro_prefix())
        }
    };
    let event_c = if trigger_id.starts_with(&ctx.macro_prefix()) {
        trigger_id.clone()
    } else {
        ctx.event_c_enum(&trigger_id)
    };

    out.push_str(&format!("        case {}: {{\n", event_c));
    // Guard.
    if let Some(g) = &t.guard {
        let cond = emit_guard(g, "m->context", "ev->__payload");
        out.push_str(&format!("            if (!{}) break;\n", cond));
    }
    // Exit sequence. Final states have no user-supplied exit action (matches
    // `impl_header.rs` which skips them when emitting prototypes).
    for exit_idx in exit_path(t, ctx.index, ctx.parents) {
        let rec = ctx.index.get(exit_idx);
        if rec.kind.is_active_at_rest() && rec.kind != StateRecordKind::Final {
            out.push_str(&format!(
                "            {prefix}_exit_{name}(m);\n",
                prefix = ctx.type_prefix(),
                name = rec.c_name,
            ));
        }
    }
    // Inline action statements. Routed through stmt emitter (machine_prefix
    // governs `raise`/`send` lowering).
    let stmt_ctx = crate::stmt::StmtContext {
        machine_prefix: ctx.type_prefix(),
        ctx_prefix: "m->context",
        payload_prefix: "ev->__payload",
    };
    out.push_str(&crate::stmt::emit_stmts(&t.actions, &stmt_ctx, 12));
    // Update state to target (if not internal).
    if !matches!(t.kind, TransitionKind::Internal) {
        let target_idx = ctx.index.must_lookup(&t.target);
        let target_rec = ctx.index.get(target_idx);
        out.push_str(&format!(
            "            m->_state = {macro}_STATE_{name};\n",
            macro = ctx.macro_prefix(),
            name = target_rec.c_name,
        ));
    }
    // Entry sequence. Final states have no user-supplied entry action.
    for entry_idx in entry_path(t, ctx.index, ctx.parents) {
        let rec = ctx.index.get(entry_idx);
        if rec.kind.is_active_at_rest() && rec.kind != StateRecordKind::Final {
            out.push_str(&format!(
                "            {prefix}_entry_{name}(m);\n",
                prefix = ctx.type_prefix(),
                name = rec.c_name,
            ));
        }
    }
    out.push_str("            return true;\n");
    out.push_str("        }\n");
    let _ = ROOT_SENTINEL;
}

fn emit_outer_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    format!(
        r#"void {prefix}_dispatch({prefix}_t *m, const {prefix}_Event_t *ev) {{
    /* B-10: walk from the active leaf up through ancestors via parent_table.
     * First state that fires a transition wins (innermost beats outermost). */
    if (ev->id < {macro}_EVENT__COUNT && ({prefix}_defer_mask[m->_state] & (1u << ev->id))) {{
        /* Deferred — store and return without processing. v1.0 simplifies
         * the defer queue to a single slot per state; multi-slot defer is
         * tracked under follow-up work. */
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
    /* No ancestor handled the event — discard per Doc 08 §3.1. */
}}
"#,
        prefix = prefix,
        macro = macro_prefix,
    )
}
