//! Completion handling — Doc 00 §7.6 (B-08), Doc 11 §15.
//!
//! After every entry sequence the runtime calls `Motor_handle_completion`,
//! which walks the ancestors of the just-entered state. For composite
//! ancestors it checks the single region's active leaf; for parallel
//! ancestors it MUST check that EVERY region's active leaf is `final` before
//! enqueueing a completion event (B-08).
//!
//! v1.0 codegen emits a per-final-state if-ladder. Parallel completion is
//! handled by a helper that calls `Motor_region_at_final(region_idx)` for
//! each region.

use crate::state_index::StateRecordKind;

use super::MachineEmitCtx;

/// Emit the `Motor_handle_completion(m)` body. Inlined into Motor.c after
/// every dispatch entry sequence.
pub fn emit_handle_completion(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "static void {prefix}_handle_completion({prefix}_t *m) {{\n",
        prefix = prefix,
    ));
    s.push_str("    /* B-08: completion fires only when ALL regions of a parallel parent\n");
    s.push_str("     * have reached Final. Composite parents fire as soon as their single\n");
    s.push_str("     * region's active leaf is Final. */\n");
    // Reference the helper unconditionally so non-parallel machines still
    // link the symbol; otherwise -Wunused-function fails the build.
    s.push_str(&format!(
        "    (void){prefix}_all_regions_final;\n",
        prefix = prefix,
    ));
    s.push_str("    m->_completion_depth++;\n");
    s.push_str("    FSM_ASSERT(m->_completion_depth <= 64);\n");
    s.push_str(&format!(
        "    {prefix}_Event_t comp;\n    comp.id = {macro}_EVENT__COMPLETION;\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    // Iterate every active slot; for each, check whether the leaf is a
    // final state, and trigger the appropriate completion behaviour.
    s.push_str("    for (uint8_t r = 0; r < m->_active_count; r++) {\n");
    s.push_str("        switch (m->_active[r]) {\n");
    let mut any_final = false;
    for rec in &ctx.index.records {
        if rec.kind != StateRecordKind::Final {
            continue;
        }
        any_final = true;
        let parent_idx = rec.parent;
        let parent_rec = ctx.index.get(parent_idx);
        s.push_str(&format!(
            "        case {macro}_STATE_{name}:\n",
            macro = ctx.macro_prefix(),
            name = rec.c_name,
        ));
        match parent_rec.kind {
            StateRecordKind::Composite => {
                // Composite parent — fire completion for the parent.
                s.push_str(&format!(
                    "            /* Composite parent {} fires immediately */\n",
                    parent_rec.dsl_name
                ));
                s.push_str(&format!(
                    "            {prefix}_dispatch(m, &comp);\n",
                    prefix = prefix,
                ));
            }
            StateRecordKind::Parallel => {
                s.push_str(&format!(
                    "            if ({prefix}_all_regions_final(m, {macro}_STATE_{parent})) {{\n",
                    prefix = prefix,
                    macro = ctx.macro_prefix(),
                    parent = parent_rec.c_name,
                ));
                s.push_str(&format!(
                    "                {prefix}_dispatch(m, &comp);\n",
                    prefix = prefix,
                ));
                s.push_str("            }\n");
            }
            _ => {
                // Root region final — top-level completion.
                s.push_str(&format!(
                    "            {prefix}_dispatch(m, &comp);\n",
                    prefix = prefix,
                ));
            }
        }
        s.push_str("            break;\n");
    }
    if !any_final {
        s.push_str("        default: (void)comp; break;\n");
    } else {
        s.push_str("        default: break;\n");
    }
    s.push_str("        }\n");
    s.push_str("    }\n");
    s.push_str("    m->_completion_depth--;\n");
    s.push_str("}\n");
    s
}

/// Emit the parallel-region completion helper. Always emitted (even when
/// the machine has no parallel states) so the dispatcher can reference it
/// unconditionally; the body becomes a trivial `(void)` no-op.
pub fn emit_all_regions_final_helper(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "static bool {prefix}_all_regions_final(const {prefix}_t *m, {prefix}_StateId_t parallel_state) {{\n",
        prefix = prefix,
    ));
    s.push_str("    (void)m; (void)parallel_state;\n");
    // Collect parallel states; for each region the codegen emits a check
    // against the assigned `_active[]` slot. Non-parallel machines still
    // get the helper (so callers can link unconditionally) but with a
    // trivial `return false;` body.
    let any_parallel = ctx
        .index
        .records
        .iter()
        .any(|r| r.kind == StateRecordKind::Parallel);
    if !any_parallel {
        s.push_str("    return false; /* no parallel states in this machine */\n");
        s.push_str("}\n");
        let _ = macro_prefix;
        return s;
    }
    s.push_str("    switch (parallel_state) {\n");
    for (parallel_idx, rec) in ctx.index.records.iter().enumerate() {
        if rec.kind != StateRecordKind::Parallel {
            continue;
        }
        let parallel_id = &rec.ir_id;
        if let Some(parallel_node) = find_parallel(ctx.machine, parallel_id) {
            s.push_str(&format!(
                "    case {macro}_STATE_{name}: /* parallel `{dsl}` */\n",
                macro = ctx.macro_prefix(),
                name = rec.c_name,
                dsl = rec.dsl_name,
            ));
            // For each region, check whether the leaf state assigned to
            // that region's slot is a final state.
            for region in &parallel_node.regions {
                let finals: Vec<String> = region
                    .states
                    .iter()
                    .filter_map(|s| match s {
                        fsm_ir::StateNode::Final(f) => Some(f.id.clone()),
                        _ => None,
                    })
                    .collect();
                if finals.is_empty() {
                    s.push_str("        return false; /* region has no Final state */\n");
                    continue;
                }
                // Resolve the slot for this region — every state in the
                // region shares the same slot, so any non-pseudo state
                // suffices.
                let slot = region
                    .states
                    .iter()
                    .filter_map(|sn| match sn {
                        fsm_ir::StateNode::Simple(s) => Some(&s.id),
                        fsm_ir::StateNode::Final(f) => Some(&f.id),
                        fsm_ir::StateNode::Composite(c) => Some(&c.id),
                        fsm_ir::StateNode::Parallel(p) => Some(&p.id),
                        _ => None,
                    })
                    .find_map(|sid| ctx.index.lookup(sid))
                    .map(|idx| ctx.layout.slot(idx))
                    .unwrap_or(0);
                let conds: Vec<String> = finals
                    .iter()
                    .map(|fid| {
                        let final_idx = ctx.index.must_lookup(fid);
                        let final_rec = ctx.index.get(final_idx);
                        format!(
                            "m->_active[{slot}] == {macro}_STATE_{name}",
                            slot = slot,
                            macro = ctx.macro_prefix(),
                            name = final_rec.c_name,
                        )
                    })
                    .collect();
                s.push_str(&format!(
                    "        if (!({})) return false;\n",
                    conds.join(" || ")
                ));
            }
            s.push_str("        return true;\n");
        }
        let _ = parallel_idx;
    }
    s.push_str("    default: return false;\n");
    s.push_str("    }\n");
    s.push_str("}\n");
    s
}

fn find_parallel<'a>(m: &'a fsm_ir::MachineObject, id: &str) -> Option<&'a fsm_ir::ParallelState> {
    fn walk<'a>(states: &'a [fsm_ir::StateNode], id: &str) -> Option<&'a fsm_ir::ParallelState> {
        for s in states {
            match s {
                fsm_ir::StateNode::Parallel(p) => {
                    if p.id == id {
                        return Some(p);
                    }
                    for r in &p.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                fsm_ir::StateNode::Composite(c) => {
                    for r in &c.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&m.root.states, id)
}
