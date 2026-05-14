//! Shared per-transition execution body — used by both the switch and
//! table dispatch strategies.
//!
//! Doc 08 §6 (exit), §7 (entry), §3.1 (run-to-completion). The execution
//! sequence is:
//!
//! 1. Exit set — innermost first up to (but not including) the effective
//!    LCA. When the exit traverses a parallel state, the runtime exits
//!    every sibling region's active leaf as well (Doc 08 §6.3).
//! 2. Transition action.
//! 3. Update the `_active[]` slot (and, for cross-region transitions,
//!    `_active_count`).
//! 4. Entry set — root first from (but not including) the LCA down to
//!    the target. If the target is a composite or parallel, the entry
//!    set is extended with the initial-chain expansion of the substates.

use fsm_ir::{StateNode, TransitionKind, TransitionObject};

use crate::expr::emit_guard;
use crate::state_index::StateRecordKind;

use super::entry_exit::{entry_path, exit_path};
use super::MachineEmitCtx;

/// Emit the inline body of a transition case (everything between the
/// guard check and `return true;` / `return;`).
///
/// `indent_spaces` is the column count for the emitted body. Both the
/// switch strategy (12) and table strategy (8) supply their own values.
/// `on_guard_fail` is the C statement to emit when the guard rejects
/// (typically `break;` for a switch case or `return;` for a function).
pub fn emit_transition_body(
    t: &TransitionObject,
    ctx: &MachineEmitCtx<'_>,
    payload_prefix: &str,
    indent_spaces: usize,
    on_guard_fail: &str,
    out: &mut String,
) {
    let pad = " ".repeat(indent_spaces);
    let prefix = ctx.type_prefix();

    // Guard check (if any). Emitted up front per Doc 08 §4.3: guards
    // are evaluated exactly once per candidate transition.
    if let Some(g) = &t.guard {
        let cond = emit_guard(g, "m->context", payload_prefix);
        out.push_str(&format!(
            "{pad}if (!{cond}) {fail}\n",
            pad = pad,
            cond = cond,
            fail = on_guard_fail,
        ));
    }

    // 1. Exit sequence — innermost first.
    let source_idx = ctx.index.must_lookup(&t.source);
    let target_idx = ctx.index.must_lookup(&t.target);
    let exits = exit_path(t, ctx.index, ctx.parents);
    // If the exit set crosses a parallel state, exit every active leaf
    // in EVERY region of that parallel — Doc 08 §6.3.
    let parallel_being_exited = exits
        .iter()
        .copied()
        .find(|idx| ctx.index.get(*idx).kind == StateRecordKind::Parallel);
    if let Some(parallel_idx) = parallel_being_exited {
        // Exit sibling region leaves first (in reverse declaration order
        // for symmetry with entry order). v1.0 codegen has no per-region
        // exit-action emitter for arbitrary nested substates, so we emit
        // the leaf-level exit functions only — sufficient for the v1.0
        // test fixtures (vending-machine, parallel-Motor).
        let sibling_leaves =
            collect_parallel_region_leaves_excluding(ctx, parallel_idx, source_idx);
        for leaf_state in sibling_leaves {
            let rec = ctx.index.get(leaf_state);
            if rec.kind.is_active_at_rest() && rec.kind != StateRecordKind::Final {
                out.push_str(&format!(
                    "{pad}/* Sibling region leaf exit (Doc 08 §6.3) */\n",
                    pad = pad,
                ));
                out.push_str(&format!(
                    "{pad}{prefix}_exit_{name}(m);\n",
                    pad = pad,
                    prefix = prefix,
                    name = rec.c_name,
                ));
            }
        }
    }
    // Emit the source-region exit chain.
    for exit_idx in &exits {
        let rec = ctx.index.get(*exit_idx);
        if rec.kind.is_active_at_rest() && rec.kind != StateRecordKind::Final {
            out.push_str(&format!(
                "{pad}{prefix}_exit_{name}(m);\n",
                pad = pad,
                prefix = prefix,
                name = rec.c_name,
            ));
        }
    }
    // Clear sibling-region slots when we just exited a parallel.
    if parallel_being_exited.is_some() {
        out.push_str(&format!(
            "{pad}/* Cross-out-of-parallel: collapse `_active[]` back to singleton */\n",
            pad = pad,
        ));
        out.push_str(&format!(
            "{pad}for (uint8_t __i = 1; __i < {macro}_MAX_PARALLEL_REGIONS; __i++) m->_active[__i] = 0;\n",
            pad = pad,
            macro = ctx.macro_prefix(),
        ));
        out.push_str(&format!("{pad}m->_active_count = 1;\n", pad = pad));
    }

    // 2. Inline action statements.
    let stmt_ctx = crate::stmt::StmtContext {
        machine_prefix: ctx.type_prefix(),
        ctx_prefix: "m->context",
        payload_prefix,
    };
    out.push_str(&crate::stmt::emit_stmts(
        &t.actions,
        &stmt_ctx,
        indent_spaces,
    ));

    // 3. Update `_active[]` slot. Internal transitions leave the
    // configuration unchanged.
    if !matches!(t.kind, TransitionKind::Internal) {
        let target_rec = ctx.index.get(target_idx);
        let target_slot = ctx.layout.slot(target_idx);
        out.push_str(&format!(
            "{pad}m->_active[{slot}] = {macro}_STATE_{name};\n",
            pad = pad,
            slot = target_slot,
            macro = ctx.macro_prefix(),
            name = target_rec.c_name,
        ));
    }

    // 4. Entry sequence — root first.
    for entry_idx in entry_path(t, ctx.index, ctx.parents) {
        let rec = ctx.index.get(entry_idx);
        if rec.kind.is_active_at_rest() && rec.kind != StateRecordKind::Final {
            out.push_str(&format!(
                "{pad}{prefix}_entry_{name}(m);\n",
                pad = pad,
                prefix = prefix,
                name = rec.c_name,
            ));
        }
    }

    // If the target is a parallel state (or composite that contains a
    // parallel), expand its initial chain to populate every region slot.
    if !matches!(t.kind, TransitionKind::Internal) {
        let target_rec = ctx.index.get(target_idx);
        if matches!(
            target_rec.kind,
            StateRecordKind::Parallel | StateRecordKind::Composite
        ) {
            emit_initial_expansion_for_target(ctx, target_idx, &pad, out);
        }
    }
}

/// Collect every leaf state in every region of `parallel_idx` EXCEPT the
/// region containing `source_idx`. Used for sibling-region exit per Doc
/// 08 §6.3.
fn collect_parallel_region_leaves_excluding(
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

/// Emit the initial-chain expansion for a transition target that is a
/// composite or parallel state. For a parallel target, every region's
/// initial leaf is assigned to its slot and entry actions run; for a
/// composite, the single region's initial chain is followed.
fn emit_initial_expansion_for_target(
    ctx: &MachineEmitCtx<'_>,
    target_idx: u8,
    pad: &str,
    out: &mut String,
) {
    let target_rec = ctx.index.get(target_idx);
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();

    match target_rec.kind {
        StateRecordKind::Composite => {
            if let Some(c) = find_composite(ctx.machine, &target_rec.ir_id) {
                if let Some(region) = c.regions.first() {
                    if let Some(init_idx) = ctx.index.lookup(&region.initial) {
                        let chain = resolve_initial_chain(ctx, init_idx);
                        for leaf in chain {
                            emit_enter_chain(ctx, leaf, pad, out);
                        }
                    }
                }
            }
        }
        StateRecordKind::Parallel => {
            if let Some(p) = find_parallel(ctx.machine, &target_rec.ir_id) {
                let region_count = p.regions.len() as u8;
                for region in &p.regions {
                    if let Some(init_idx) = ctx.index.lookup(&region.initial) {
                        let chain = resolve_initial_chain(ctx, init_idx);
                        for leaf in chain {
                            emit_enter_chain(ctx, leaf, pad, out);
                        }
                    }
                }
                // Region 0 of the parallel shares slot 0; the remaining
                // regions take slots 1..N-1. Active count = N.
                out.push_str(&format!(
                    "{pad}m->_active_count = {n};\n",
                    pad = pad,
                    n = region_count,
                ));
                let _ = (prefix, macro_prefix);
            }
        }
        _ => {}
    }
}

fn emit_enter_chain(ctx: &MachineEmitCtx<'_>, state_idx: u8, pad: &str, out: &mut String) {
    let rec = ctx.index.get(state_idx);
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    match rec.kind {
        StateRecordKind::Simple | StateRecordKind::Final | StateRecordKind::Submachine => {
            let slot = ctx.layout.slot(state_idx);
            out.push_str(&format!(
                "{pad}m->_active[{slot}] = {macro}_STATE_{name};\n",
                pad = pad,
                slot = slot,
                macro = macro_prefix,
                name = rec.c_name,
            ));
            if rec.kind != StateRecordKind::Final {
                out.push_str(&format!(
                    "{pad}{prefix}_entry_{name}(m);\n",
                    pad = pad,
                    prefix = prefix,
                    name = rec.c_name,
                ));
            }
        }
        StateRecordKind::Composite => {
            out.push_str(&format!(
                "{pad}{prefix}_entry_{name}(m);\n",
                pad = pad,
                prefix = prefix,
                name = rec.c_name,
            ));
            if let Some(c) = find_composite(ctx.machine, &rec.ir_id) {
                if let Some(region) = c.regions.first() {
                    if let Some(init_idx) = ctx.index.lookup(&region.initial) {
                        let chain = resolve_initial_chain(ctx, init_idx);
                        for child in chain {
                            emit_enter_chain(ctx, child, pad, out);
                        }
                    }
                }
            }
        }
        StateRecordKind::Parallel => {
            out.push_str(&format!(
                "{pad}{prefix}_entry_{name}(m);\n",
                pad = pad,
                prefix = prefix,
                name = rec.c_name,
            ));
            if let Some(p) = find_parallel(ctx.machine, &rec.ir_id) {
                for region in &p.regions {
                    if let Some(init_idx) = ctx.index.lookup(&region.initial) {
                        let chain = resolve_initial_chain(ctx, init_idx);
                        for child in chain {
                            emit_enter_chain(ctx, child, pad, out);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn resolve_initial_chain(ctx: &MachineEmitCtx<'_>, mut cur: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut bounce = 0;
    while bounce < 32 {
        bounce += 1;
        let rec = ctx.index.get(cur);
        match rec.kind {
            StateRecordKind::Initial => {
                if let Some(target_id) = find_initial_target(ctx.machine, &rec.ir_id) {
                    if let Some(next) = ctx.index.lookup(&target_id) {
                        cur = next;
                        continue;
                    }
                }
                break;
            }
            _ => {
                out.push(cur);
                break;
            }
        }
    }
    out
}

fn find_initial_target(machine: &fsm_ir::MachineObject, ir_id: &str) -> Option<String> {
    fn walk(states: &[StateNode], id: &str) -> Option<String> {
        for s in states {
            match s {
                StateNode::Initial(i) => {
                    if i.id == id {
                        return Some(i.target.clone());
                    }
                }
                StateNode::Composite(c) => {
                    for r in &c.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
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
    walk(&machine.root.states, ir_id)
}

fn find_composite<'a>(
    m: &'a fsm_ir::MachineObject,
    id: &str,
) -> Option<&'a fsm_ir::CompositeState> {
    fn walk<'a>(states: &'a [StateNode], id: &str) -> Option<&'a fsm_ir::CompositeState> {
        for s in states {
            match s {
                StateNode::Composite(c) => {
                    if c.id == id {
                        return Some(c);
                    }
                    for r in &c.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
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

fn find_parallel<'a>(m: &'a fsm_ir::MachineObject, id: &str) -> Option<&'a fsm_ir::ParallelState> {
    fn walk<'a>(states: &'a [StateNode], id: &str) -> Option<&'a fsm_ir::ParallelState> {
        for s in states {
            match s {
                StateNode::Parallel(p) => {
                    if p.id == id {
                        return Some(p);
                    }
                    for r in &p.regions {
                        if let Some(hit) = walk(&r.states, id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Composite(c) => {
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
