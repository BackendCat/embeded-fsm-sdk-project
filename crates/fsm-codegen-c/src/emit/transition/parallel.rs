//! Parallel-state-specific helpers for the transition emitter — Doc 08
//! §6.3 (sibling-region exit) and FW109 (the active-descendant exit-set
//! trace-record class generalised from Composite to Parallel).

use fsm_ir::StateNode;

use crate::emit::MachineEmitCtx;
use crate::state_index::StateRecordKind;

/// Collect every leaf state in every region of `parallel_idx` EXCEPT the
/// region containing `source_idx`. Used for sibling-region exit per Doc
/// 08 §6.3.
pub(super) fn collect_parallel_region_leaves_excluding(
    ctx: &MachineEmitCtx<'_>,
    parallel_idx: u8,
    source_idx: u8,
) -> Vec<u8> {
    let parallel_rec = ctx.index.get(parallel_idx);
    let source_slot = ctx.layout.slot(source_idx);

    // Walk every state whose slot differs from source_slot AND is inside
    // this parallel.
    let mut out = Vec::new();
    let in_parallel = states_inside_parallel(ctx.machine, &parallel_rec.ir_id);
    for ir_id in in_parallel {
        let Some(idx) = ctx.index.lookup(&ir_id) else {
            continue;
        };
        if ctx.layout.slot(idx) == source_slot {
            continue;
        }
        if ctx.layout.slot(idx) == 0 {
            continue;
        }
        let rec = ctx.index.get(idx);
        if matches!(
            rec.kind,
            StateRecordKind::Simple | StateRecordKind::Final | StateRecordKind::Submachine
        ) {
            out.push(idx);
        }
    }
    out
}

/// Collect every leaf state (simple / final / submachine-ref) in **every**
/// region of `parallel_idx` — the source region's leaves included, Final
/// leaves included.
///
/// FW109: the *trace-record* candidate set for the Parallel active-
/// descendant exit-set. The shipped `fsm_simulator`'s `full_exits` records
/// every active descendant of an exited Parallel — that is the runtime-
/// active leaf of EVERY region (including the source region's leaf, which
/// is still an active descendant of the Parallel, AND any Final leaf). This
/// is the static candidate set; the emitted code's runtime
/// `m->_active[slot] == STATE_X` guard selects exactly the live ones,
/// mirroring the simulator's `active_states` membership test. Pure data —
/// no semantics; the recorded set is read off the C's own `_active[]`.
pub(super) fn collect_parallel_region_leaves_all(
    ctx: &MachineEmitCtx<'_>,
    parallel_idx: u8,
) -> Vec<u8> {
    let parallel_rec = ctx.index.get(parallel_idx);
    let mut out = Vec::new();
    for ir_id in states_inside_parallel(ctx.machine, &parallel_rec.ir_id) {
        let Some(idx) = ctx.index.lookup(&ir_id) else {
            continue;
        };
        let rec = ctx.index.get(idx);
        if matches!(
            rec.kind,
            StateRecordKind::Simple | StateRecordKind::Final | StateRecordKind::Submachine
        ) {
            out.push(idx);
        }
    }
    out
}

fn states_inside_parallel(m: &fsm_ir::MachineObject, parallel_ir_id: &str) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(states: &[StateNode], out: &mut Vec<String>) {
        for s in states {
            match s {
                StateNode::Simple(s) => out.push(s.id.clone()),
                StateNode::Final(f) => out.push(f.id.clone()),
                StateNode::Submachine(s) => out.push(s.id.clone()),
                StateNode::Composite(c) => {
                    out.push(c.id.clone());
                    for r in &c.regions {
                        walk(&r.states, out);
                    }
                }
                StateNode::Parallel(p) => {
                    out.push(p.id.clone());
                    for r in &p.regions {
                        walk(&r.states, out);
                    }
                }
                _ => {}
            }
        }
    }
    fn find_and_walk(states: &[StateNode], parallel_id: &str, out: &mut Vec<String>) -> bool {
        for s in states {
            match s {
                StateNode::Parallel(p) if p.id == parallel_id => {
                    for r in &p.regions {
                        walk(&r.states, out);
                    }
                    return true;
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
                        if find_and_walk(&r.states, parallel_id, out) {
                            return true;
                        }
                    }
                }
                StateNode::Composite(c) => {
                    for r in &c.regions {
                        if find_and_walk(&r.states, parallel_id, out) {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }
    find_and_walk(&m.root.states, parallel_ir_id, &mut out);
    out
}
