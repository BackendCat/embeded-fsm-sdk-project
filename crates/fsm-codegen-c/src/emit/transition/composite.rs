//! Composite-state-specific helpers — the active-descendant exit-set walk
//! that mirrors the shipped simulator's `full_exits` for a composite
//! source (Doc 08 §6.1, FW1-FU-2).

use fsm_ir::StateNode;

use crate::emit::MachineEmitCtx;

use super::find::find_composite;

/// Collect every leaf state (simple / final / submachine-ref) nested
/// anywhere inside the composite `composite_idx`, in declaration order.
///
/// FW1-FU-2: the set of states whose `_exit_X` may need to run when the
/// composite is left, depending on which one is the live leaf at runtime.
/// Mirrors the simulator's "active descendant of an exited state" walk
/// (interpreter.rs `full_exits`), but expressed as the *static* candidate
/// set the runtime switch selects from (the codegen analogue of the
/// simulator's `active_states` membership test). Nested composites are
/// recursed into so a deep leaf is reached; the composite/parallel
/// container states themselves are NOT leaves (the simulator records them
/// via the static exit chain / parallel emitter, not here).
pub(super) fn composite_descendant_leaves(ctx: &MachineEmitCtx<'_>, composite_idx: u8) -> Vec<u8> {
    let rec = ctx.index.get(composite_idx);
    let Some(c) = find_composite(ctx.machine, &rec.ir_id) else {
        return Vec::new();
    };
    fn walk(states: &[StateNode], leaves: &mut Vec<String>) {
        for s in states {
            match s {
                StateNode::Simple(x) => leaves.push(x.id.clone()),
                StateNode::Final(f) => leaves.push(f.id.clone()),
                StateNode::Submachine(sm) => leaves.push(sm.id.clone()),
                StateNode::Composite(cc) => {
                    for r in &cc.regions {
                        walk(&r.states, leaves);
                    }
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
                        walk(&r.states, leaves);
                    }
                }
                _ => {}
            }
        }
    }
    let mut leaf_ids = Vec::new();
    for r in &c.regions {
        walk(&r.states, &mut leaf_ids);
    }
    leaf_ids
        .into_iter()
        .filter_map(|id| ctx.index.lookup(&id))
        .collect()
}
