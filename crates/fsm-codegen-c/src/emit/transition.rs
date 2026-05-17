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

use fsm_ir::{BranchHint, StateNode, TransitionKind, TransitionObject};

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
    //
    // v1.1-W4: a `likely`/`rare` transition prefix wraps this guard
    // condition in the portable `<PREFIX>_LIKELY`/`_UNLIKELY` macro
    // (`__builtin_expect` on GNU/clang, plain `(x)` fallback — see
    // header.rs::emit_branch_hint_macros). `likely` ⇒ the guard is
    // expected to HOLD (the transition is the hot path) ⇒ `_LIKELY(cond)`;
    // `rare` ⇒ expected to FAIL ⇒ `_UNLIKELY(cond)`. Unhinted ⇒ the bare
    // `cond` exactly as before this wave (byte-identical generated C).
    // This is a pure instruction-layout hint: it never changes whether the
    // guard passes, only the compiler's hot/cold block placement, so the
    // runtime behaviour and the simulator stay in lock-step (sim ignores
    // the hint entirely).
    if let Some(g) = &t.guard {
        let cond = emit_guard(g, "m->context", payload_prefix);
        let hinted = match t.hint {
            Some(BranchHint::Likely) => format!("{}_LIKELY({})", ctx.macro_prefix(), cond),
            Some(BranchHint::Rare) => format!("{}_UNLIKELY({})", ctx.macro_prefix(), cond),
            None => cond,
        };
        out.push_str(&format!(
            "{pad}if (!{cond}) {fail}\n",
            pad = pad,
            cond = hinted,
            fail = on_guard_fail,
        ));
    }

    // 1. Exit sequence — innermost first.
    let source_idx = ctx.index.must_lookup(&t.source);
    let target_idx = ctx.index.must_lookup(&t.target);
    let exits = exit_path(t, ctx.index, ctx.parents);

    // FW1-FU-2 (Doc 08 §6.4, Doc 32 §1 W1): snapshot history BEFORE running
    // any exit actions. The shipped `fsm_simulator` calls
    // `record_history_before_exit(rt, &exits)` first thing in
    // `execute_one_transition` — for every exited composite that declares a
    // `history` pseudo-state it records the (shallow: direct-child /
    // deep: full descendant) live config. The prior codegen emitted the
    // `_history_record_X` helper but NEVER called it ("the dispatch path
    // does not yet target history-pseudo restore" — history.rs), so a later
    // `RESUME -> HAuto` had nothing to restore. We now wire the snapshot in
    // at exactly the simulator's point (before exit actions), so the next
    // history-targeted entry resolves to the remembered leaf.
    for exit_idx in &exits {
        let rec = ctx.index.get(*exit_idx);
        if rec.history_pseudo.is_some() {
            out.push_str(&format!(
                "{pad}{prefix}_history_record_{name}(m); /* Doc 08 §6.4 — snapshot before exit (FW1-FU-2) */\n",
                pad = pad,
                prefix = prefix,
                name = rec.c_name,
            ));
        }
    }

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
    // P0-4: when a state owns timers, disarm them on exit so they cannot
    // fire after the owning state is no longer active (Doc 08 §13.2).
    //
    // FW1-FU-2 (Doc 32 §1 W1): the *full exit-set* of a composite includes
    // its currently-active descendant substate(s) — exiting `Auto` (with
    // active leaf `Red`) must also exit `Red`. The shipped
    // `fsm_simulator::execute_one_transition` does this by walking
    // `active_states` for descendants of every exited state
    // (interpreter.rs `full_exits` loop) and emitting them innermost-first
    // (Doc 08 §6.1/§6.3). The prior codegen only exited the static
    // source→LCA chain, so when the *source itself* is a composite the
    // active leaf was never exited (the simulator recorded
    // `ext=Auto,Red`; the C recorded only `Auto`). We mirror the simulator:
    // for each exited composite whose active leaf is NOT already on the
    // static exit chain, emit a runtime switch on its `_active[]` slot that
    // exits the live descendant leaf FIRST (innermost-first), then the
    // composite. (The parallel case is already handled above by the
    // sibling-region leaf emitter; this is the composite analogue. A
    // composite's descendants all share the composite's slot in the v1.0
    // single-region layout, so the live leaf is `m->_active[slot]`.)
    let all_timers = super::timer::collect_timers(ctx);
    let statically_exited: std::collections::HashSet<u8> = exits.iter().copied().collect();
    for exit_idx in &exits {
        let rec = ctx.index.get(*exit_idx);
        // Composite exit: first exit whichever descendant leaf is live now
        // (the simulator's active-descendant exit), then the composite
        // itself. Skip leaves already on the static chain (they are exited
        // by the loop body below) to avoid a double `_exit_X`.
        if rec.kind == StateRecordKind::Composite {
            let descendant_leaves = composite_descendant_leaves(ctx, *exit_idx);
            let slot = ctx.layout.slot(*exit_idx);
            let mut emitted_header = false;
            for leaf_idx in descendant_leaves {
                if statically_exited.contains(&leaf_idx) {
                    continue;
                }
                let leaf = ctx.index.get(leaf_idx);
                if super::submachine::ref_state_member(ctx, &leaf.ir_id).is_some() {
                    // Sub-instance teardown is implicit (W2d) — no `_exit_X`,
                    // but the leaf IS still part of the exit-set the trace
                    // must record.
                    if !emitted_header {
                        out.push_str(&format!(
                            "{pad}/* Composite active-descendant exit-set (Doc 08 §6.1, FW1-FU-2) */\n",
                            pad = pad,
                        ));
                        emitted_header = true;
                    }
                    out.push_str(&format!(
                        "{pad}#ifdef FSM_TRACE\n{pad}if (m->_active[{slot}] == {macro}_STATE_{lname}) fsm_trace_csv_append(m->_trace_ext, sizeof(m->_trace_ext), {lit});\n{pad}#endif\n",
                        pad = pad,
                        slot = slot,
                        macro = ctx.macro_prefix(),
                        lname = leaf.c_name,
                        lit = super::trace_hook::c_string_literal(&leaf.ir_id),
                    ));
                    continue;
                }
                if !emitted_header {
                    out.push_str(&format!(
                        "{pad}/* Composite active-descendant exit-set (Doc 08 §6.1, FW1-FU-2) */\n",
                        pad = pad,
                    ));
                    emitted_header = true;
                }
                out.push_str(&format!(
                    "{pad}if (m->_active[{slot}] == {macro}_STATE_{lname}) {{\n",
                    pad = pad,
                    slot = slot,
                    macro = ctx.macro_prefix(),
                    lname = leaf.c_name,
                ));
                out.push_str(&format!(
                    "{pad}    {prefix}_exit_{lname}(m);\n",
                    pad = pad,
                    prefix = prefix,
                    lname = leaf.c_name,
                ));
                // Disarm any timer the live leaf owns (P0-4 parity — the
                // static loop below disarms timers for statically-exited
                // states; the runtime-resolved leaf needs the same).
                for timer in &all_timers {
                    if timer.owner_state == leaf_idx {
                        out.push_str(&format!(
                            "{pad}    m->_timer_{tname}_remaining_ms = 0u; /* P0-4: cancel owned timer on exit */\n",
                            pad = pad,
                            tname = timer.field_name,
                        ));
                    }
                }
                out.push_str(&format!(
                    "{pad}    #ifdef FSM_TRACE\n{pad}    fsm_trace_csv_append(m->_trace_ext, sizeof(m->_trace_ext), {lit});\n{pad}    #endif\n",
                    pad = pad,
                    lit = super::trace_hook::c_string_literal(&leaf.ir_id),
                ));
                out.push_str(&format!("{pad}}}\n", pad = pad));
            }
        }
        if rec.kind.is_active_at_rest() && rec.kind != StateRecordKind::Final {
            // v1.1-W2d: a submachine ref-state has no user `_exit_X`. Its
            // sub-instance is a value member — teardown is implicit (no
            // free), and a fresh re-init happens on the next entry (the
            // ref-state self-transition case), mirroring W2c's `run_exit`
            // teardown→re-instantiate. So emit nothing on exit here.
            if super::submachine::ref_state_member(ctx, &rec.ir_id).is_none() {
                out.push_str(&format!(
                    "{pad}{prefix}_exit_{name}(m);\n",
                    pad = pad,
                    prefix = prefix,
                    name = rec.c_name,
                ));
            }
        }
        for timer in &all_timers {
            if timer.owner_state == *exit_idx {
                out.push_str(&format!(
                    "{pad}m->_timer_{tname}_remaining_ms = 0u; /* P0-4: cancel owned timer on exit */\n",
                    pad = pad,
                    tname = timer.field_name,
                ));
            }
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

    // FW1-FU-2: is the transition target a history pseudo-state? The
    // shipped simulator's `resolve_target` resolves a History target to the
    // recorded child (or its default) and runs the full entry sequence to
    // that leaf. The prior codegen wrote `_active[slot] = STATE_HAuto` (a
    // pseudo-state, which never appears at rest) and never entered the
    // restored leaf. We instead enter the *owning composite* via the static
    // `entry_path` below (History is filtered out of it — see step 4 / the
    // `entered_ir` filter), then call `<M>_history_restore_<composite>`
    // which writes the real leaf slot + records the restored leaf in the
    // trace `ent` set (history.rs). Find the composite that owns this
    // history pseudo.
    let history_owner_idx: Option<u8> =
        if ctx.index.get(target_idx).kind == StateRecordKind::History {
            ctx.index
                .records
                .iter()
                .position(|r| r.history_pseudo == Some(target_idx))
                .map(|p| p as u8)
        } else {
            None
        };

    // 3. Update `_active[]` slot. Internal transitions leave the
    // configuration unchanged. A history target does NOT write the slot
    // here — `<M>_history_restore_*` writes the *resolved leaf* slot
    // (writing `STATE_HAuto`, a pseudo-state, would corrupt the config and
    // the cfgA projection, exactly the prior bug).
    if !matches!(t.kind, TransitionKind::Internal) && history_owner_idx.is_none() {
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
    // P0-4: arm any timers owned by states being entered (Doc 08 §13.1).
    for entry_idx in entry_path(t, ctx.index, ctx.parents) {
        let rec = ctx.index.get(entry_idx);
        if rec.kind.is_active_at_rest() && rec.kind != StateRecordKind::Final {
            // v1.1-W2d: entering a submachine ref-state instantiates its
            // sub-instance fresh (Doc 08 §12.2) instead of calling a user
            // `_entry_X`. For a ref-state self-transition
            // (`Connecting --RECONNECT--> Connecting`) this is the
            // re-init-on-re-entry that mirrors W2c's teardown→re-sync.
            if let Some(sr) = super::submachine::ref_state_member(ctx, &rec.ir_id) {
                super::submachine::emit_sub_init(&sr, &pad, out);
            } else {
                out.push_str(&format!(
                    "{pad}{prefix}_entry_{name}(m);\n",
                    pad = pad,
                    prefix = prefix,
                    name = rec.c_name,
                ));
            }
        }
        for timer in &all_timers {
            if timer.owner_state == entry_idx {
                out.push_str(&format!(
                    "{pad}m->_timer_{tname}_remaining_ms = {dur}u; /* P0-4: arm timer on entry */\n",
                    pad = pad,
                    tname = timer.field_name,
                    dur = timer.duration_ms,
                ));
            }
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

    // FW1-FU-2: history-target restore. The owning composite was just
    // entered by the static `entry_path` loop (its `_entry_X` ran and it is
    // in `entered_ir`); now resolve + enter the *remembered child leaf*.
    // `<M>_history_restore_<composite>` writes the resolved leaf into its
    // `_active[]` slot and, under `#ifdef FSM_TRACE`, appends the entered
    // child ids to `m->_trace_ent` — mirroring the simulator's
    // `resolve_target(History) -> entry_path/expand_initial -> leaf`. The
    // composite itself is NOT re-appended here (it is already in
    // `entered_ir`); only the runtime-resolved inner leaf is, which is why
    // it must be emitted inline (it is not statically known).
    if let Some(owner_idx) = history_owner_idx {
        out.push_str(&format!(
            "{pad}{prefix}_history_restore_{name}(m); /* FW1-FU-2: resolve shallow_history → remembered leaf */\n",
            pad = pad,
            prefix = prefix,
            name = ctx.index.get(owner_idx).c_name,
        ));
    }

    // W1 R7 host-trace differential (Doc 32 §1 W1, compile-time-gated):
    // record WHICH transition this generated C just fired. `stable_id` /
    // `source` / `target` are compile-time literals of the transition the
    // C's OWN dispatch selected; the entered/exited IR ids are this code's
    // OWN `entry_path`/`exit_path` decisions (the very thing the
    // differential cross-checks against the simulator). This is a trace
    // tap, not a re-derivation — `#ifndef FSM_TRACE` strips it entirely so
    // the production C is byte-identical (the keystone, Doc 32 §2).
    let entered_ir: Vec<String> = entry_path(t, ctx.index, ctx.parents)
        .into_iter()
        .filter(|i| {
            let k = ctx.index.get(*i).kind;
            k.is_active_at_rest() && k != StateRecordKind::Final
        })
        .map(|i| ctx.index.get(i).ir_id.clone())
        .collect();
    let exited_ir: Vec<String> = exits
        .iter()
        .copied()
        .filter(|i| {
            let k = ctx.index.get(*i).kind;
            k.is_active_at_rest() && k != StateRecordKind::Final
        })
        .map(|i| ctx.index.get(i).ir_id.clone())
        .collect();
    out.push_str(&super::trace_hook::emit_trace_record_transition(
        t,
        &entered_ir,
        &exited_ir,
        &pad,
    ));
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
fn composite_descendant_leaves(ctx: &MachineEmitCtx<'_>, composite_idx: u8) -> Vec<u8> {
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
    let all_timers = super::timer::collect_timers(ctx);
    let arm_timers = |state_idx: u8, out: &mut String| {
        for timer in &all_timers {
            if timer.owner_state == state_idx {
                out.push_str(&format!(
                    "{pad}m->_timer_{tname}_remaining_ms = {dur}u; /* P0-4: arm timer on initial-chain entry */\n",
                    pad = pad,
                    tname = timer.field_name,
                    dur = timer.duration_ms,
                ));
            }
        }
    };
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
                // v1.1-W2d: a ref-state reached via an initial-chain
                // expansion (a `state X is Sub` as a composite's initial)
                // instantiates its sub-instance instead of a user
                // `_entry_X`.
                if let Some(sr) = super::submachine::ref_state_member(ctx, &rec.ir_id) {
                    super::submachine::emit_sub_init(&sr, pad, out);
                } else {
                    out.push_str(&format!(
                        "{pad}{prefix}_entry_{name}(m);\n",
                        pad = pad,
                        prefix = prefix,
                        name = rec.c_name,
                    ));
                }
                arm_timers(state_idx, out);
            }
        }
        StateRecordKind::Composite => {
            out.push_str(&format!(
                "{pad}{prefix}_entry_{name}(m);\n",
                pad = pad,
                prefix = prefix,
                name = rec.c_name,
            ));
            arm_timers(state_idx, out);
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
            arm_timers(state_idx, out);
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
