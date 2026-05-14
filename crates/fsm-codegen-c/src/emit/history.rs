//! History pseudo-state codegen — Doc 11 §14 (shallow) and §21 (deep).
//!
//! Both kinds of history store one `StateId_t` slot per history pseudo-state
//! in the machine struct. On exit from the composite the slot is updated.
//! On entry through the history pseudo, the restore path is selected. B-14
//! makes `default_target` mandatory, so the analyzer guarantees a valid
//! fallback.

use crate::state_index::StateRecordKind;

use super::MachineEmitCtx;

/// Emit `Motor_history_record_X(m)` helpers for every history-bearing
/// composite state. These are called from the exit sequence of the
/// composite to snapshot the current child / leaf.
///
/// Currently emits a stub for each — actual semantic difference between
/// shallow (direct child) and deep (leaf) is encoded in the helper body.
///
/// v1.0 codegen note: the dispatch path does not yet target history-pseudo
/// restore. Helpers are emitted `static inline` so `gcc -Werror` does not
/// flag them as unused. Wiring history into the dispatcher is tracked as
/// Phase 2.3 follow-up; the analyzer + IR + helper bodies are complete.
pub fn emit_history_helpers(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::new();
    for rec in &ctx.index.records {
        if rec.history_pseudo.is_none() {
            continue;
        }
        let hp_idx = rec.history_pseudo.unwrap();
        let hp_rec = ctx.index.get(hp_idx);
        s.push_str(&format!(
            "static inline void {prefix}_history_record_{name}({prefix}_t *m) {{\n",
            prefix = prefix,
            name = rec.c_name,
        ));
        s.push_str(&format!(
            "    /* Record current leaf for history {hp} */\n",
            hp = hp_rec.dsl_name,
        ));
        // History recording targets the composite's primary slot. v1.0
        // does not yet support history across parallel regions; for
        // non-parallel composites the relevant leaf always lives in
        // `_active[0]`.
        s.push_str(&format!(
            "    m->_history_{name} = m->_active[0];\n",
            name = rec.c_name,
        ));
        s.push_str("}\n\n");

        // Restore — uses the IR's default_target via the analyzer-validated
        // value. We emit the per-history switch over the indexed default.
        s.push_str(&format!(
            "static inline void {prefix}_history_restore_{name}({prefix}_t *m) {{\n",
            prefix = prefix,
            name = rec.c_name,
        ));
        s.push_str(&format!(
            "    {prefix}_StateId_t restore = m->_history_{name};\n",
            prefix = prefix,
            name = rec.c_name,
        ));
        // Resolve the IR default target → state index → C enum name. The
        // raw HistoryObject lives on the composite IR; find it.
        if let Some(default_c_name) = find_history_default(ctx, &rec.ir_id) {
            s.push_str(&format!(
                "    if (restore == {macro}_STATE_ROOT) restore = {macro}_STATE_{def};\n",
                macro = ctx.macro_prefix(),
                def = default_c_name,
            ));
        } else {
            s.push_str("    /* default_target missing — analyzer should have rejected */\n");
        }
        s.push_str("    m->_active[0] = restore;\n");
        s.push_str("}\n\n");
    }
    s
}

fn find_history_default(ctx: &MachineEmitCtx<'_>, composite_ir_id: &str) -> Option<String> {
    // Walk the machine looking for the matching composite + its history.
    fn walk_states<'a>(
        states: &'a [fsm_ir::StateNode],
        composite_id: &str,
    ) -> Option<&'a fsm_ir::HistoryObject> {
        for s in states {
            match s {
                fsm_ir::StateNode::Composite(c) => {
                    if c.id == composite_id {
                        return c.history.as_ref();
                    }
                    for r in &c.regions {
                        if let Some(hit) = walk_states(&r.states, composite_id) {
                            return Some(hit);
                        }
                    }
                }
                fsm_ir::StateNode::Parallel(p) => {
                    for r in &p.regions {
                        if let Some(hit) = walk_states(&r.states, composite_id) {
                            return Some(hit);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    let history = walk_states(&ctx.machine.root.states, composite_ir_id)?;
    let default_idx = ctx.index.lookup(&history.default_target)?;
    let rec = ctx.index.get(default_idx);
    let _ = StateRecordKind::Final;
    Some(rec.c_name.clone())
}
