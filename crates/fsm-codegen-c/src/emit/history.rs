//! History pseudo-state codegen — Doc 11 §14 (shallow) and §21 (deep).
//!
//! Both kinds of history store one `StateId_t` slot per history pseudo-state
//! in the machine struct. On exit from the composite the slot is updated
//! (`_history_record_X`, wired from `transition.rs` at the simulator's
//! `record_history_before_exit` point — Doc 08 §6.4). On a transition that
//! *targets* the history pseudo-state, `_history_restore_X` resolves the
//! remembered child, runs its entry sequence, writes the live `_active[]`
//! slot, and (under `#ifdef FSM_TRACE`) appends the entered ids to the
//! step's `ent` set so the host-trace differential matches the shipped
//! `fsm_simulator` (FW1-FU-2, Doc 32 §1 W1). B-14 makes `default_target`
//! mandatory, so the analyzer guarantees a valid fallback.
//!
//! ## FW1-FU-2 — what the prior codegen got wrong
//!
//! The prior `_history_restore_X` only assigned `m->_active[0] = restore`
//! and was **never called** ("the dispatch path does not yet target
//! history-pseudo restore"). The shipped simulator's `resolve_target`
//! resolves a History target to the recorded child (or its default) and
//! then runs the *full entry sequence* down to the leaf (entering the
//! composite + the restored leaf, expanding any deeper initial). The
//! generated C reported the raw `HAuto` pseudo-state in `_active[]` and
//! never entered the restored leaf — `cfgA=HAuto`/`ent=Auto` instead of
//! `cfgA=Red`/`ent=Auto,Red`. This module now mirrors the simulator:
//! `transition.rs` enters the owning composite via the static `entry_path`
//! (History is filtered out of `_active[]`/`ent` there); this helper enters
//! the runtime-resolved restored child, writes the real leaf into the slot,
//! and records it in the trace.
//!
//! ## Scope (honest — updated FW110-FU-B)
//!
//! - **Shallow history into a flat (single-leaf) region** — fully correct
//!   (the recorded direct-child IS the leaf; the restore enters it and
//!   writes the slot). This is the `traffic-light` fixture (BYTE_EQUAL).
//! - **Shallow history into a region whose direct child is itself a
//!   composite** — the restore enters that direct child and then expands
//!   *its* initial chain (a fresh deeper config, the UML shallow-history
//!   semantics: only the direct child is remembered, deeper state is
//!   re-initialised). Handled.
//! - **Deep history into a (possibly multi-level) nested *single-region*
//!   composite** — **now fully correct (FW110-FU-B)**. The recorded slot
//!   holds the innermost active *leaf* (for a non-parallel nested composite
//!   the deep leaf already lives in `_active[0]`, so `_history_record_X` is
//!   unchanged and correct); `restore_deep_leaf_chains` re-enters EVERY
//!   intermediate composite from the owning composite's region down to that
//!   remembered leaf and then the leaf, writing its slot. This mirrors the
//!   shipped `fsm_simulator` `record_history_before_exit`
//!   (`HistoryKind::Deep` → `find_active_in_subtree`) +
//!   `resolve_target`(History) → `entry_path`/`expand_initial` EXACTLY (the
//!   `stress-deep-history` fixture is BYTE_EQUAL to the unforked oracle).
//!   UML deep history: the full nested configuration is *remembered and
//!   restored*, not re-initialised (the shallow contrast above).
//! - **History across parallel regions** remains the pre-existing v1.0
//!   limitation: `_history_record_X` snapshots `_active[0]` only, so for a
//!   history-bearing composite whose subtree contains a PARALLEL state the
//!   other regions' leaves are not captured/restored. This is a genuinely
//!   separable residual (a multi-slot snapshot + per-region restore-ordering
//!   problem), NOT exercised by `stress-deep-history` (a single-region
//!   nested composite) nor any shipped example. Recorded honestly, not
//!   silently widened (the anti-scope-creep discipline); a dedicated
//!   follow-up wave is recommended if/when a parallel-region-history fixture
//!   is added (the W1→W1-FU precedent — see the FW110-FU-B completion
//!   report's explicit scope decision).

use fsm_ir::StateNode;

use super::MachineEmitCtx;
use crate::state_index::StateRecordKind;

/// Emit `<M>_history_record_X(m)` + `<M>_history_restore_X(m)` helpers for
/// every history-bearing composite state. `record` is called from the
/// exit sequence of the composite to snapshot the current child / leaf;
/// `restore` is called from a transition whose target is the history
/// pseudo-state, to re-enter the remembered child.
///
/// Helpers stay `static inline` (called from this TU's dispatch body; the
/// inline keyword keeps `-Werror=unused-function` quiet on the unlikely
/// machine that declares a history pseudo it never targets).
pub fn emit_history_helpers(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    for rec in &ctx.index.records {
        if rec.history_pseudo.is_none() {
            continue;
        }
        let hp_idx = rec.history_pseudo.unwrap();
        let hp_rec = ctx.index.get(hp_idx);

        // ── record ──────────────────────────────────────────────────────
        s.push_str(&format!(
            "static inline void {prefix}_history_record_{name}({prefix}_t *m) {{\n",
            prefix = prefix,
            name = rec.c_name,
        ));
        s.push_str(&format!(
            "    /* Record current leaf for history {hp} (Doc 08 §6.4) */\n",
            hp = hp_rec.dsl_name,
        ));
        // History recording targets the composite's primary slot. For a
        // non-parallel composite the relevant active leaf always lives in
        // `_active[0]` — and this is correct for BOTH shallow and deep:
        //   • SHALLOW: the simulator records `direct_child_of(ex, leaf)`;
        //     for a flat region the direct child IS the leaf (== `_active[0]`)
        //     and `restore_children_chains` re-expands a nested direct child's
        //     initial (the UML shallow re-init), so the slot need only pin
        //     the direct child / flat leaf.
        //   • DEEP (FW110-FU-B): the simulator records
        //     `find_active_in_subtree` = the innermost active *leaf*; for a
        //     single-region nested composite that leaf is exactly
        //     `_active[0]`, and `restore_deep_leaf_chains` re-enters the full
        //     path down to it (UML deep restore). So the single-`StateId_t`
        //     slot is sufficient and `_active[0]` is the right thing to
        //     snapshot here.
        // History ACROSS PARALLEL REGIONS is the documented limitation
        // (module header §scope): a parallel subtree's non-primary regions
        // are not in `_active[0]`, so they are not captured. Not exercised
        // by `stress-deep-history` (single-region nested); not silently
        // widened.
        s.push_str(&format!(
            "    m->_history_{name} = m->_active[0];\n",
            name = rec.c_name,
        ));
        s.push_str("}\n\n");

        // ── restore ─────────────────────────────────────────────────────
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
        let default_idx = find_history_default_idx(ctx, &rec.ir_id);
        if let Some(def_idx) = default_idx {
            let def_rec = ctx.index.get(def_idx);
            // No history yet (`memset`-0 == STATE_ROOT) → the analyzer-
            // validated default child. (UML: the first entry through
            // history uses the default.)
            s.push_str(&format!(
                "    if (restore == {macro}_STATE_ROOT) restore = {macro}_STATE_{def};\n",
                macro = macro_prefix,
                def = def_rec.c_name,
            ));
        } else {
            s.push_str(
                "    /* default_target missing — analyzer (FSM-E0111) should have rejected */\n",
            );
        }
        // SHALLOW: for each possible remembered *direct child* of the
        // composite's region, run that child's entry sequence (the
        // simulator's `resolve_target` → `entry_path`/`expand_initial`),
        // write the leaf-most state into the slot, and (gated) record the
        // entered ids in the step's `ent` set.
        //
        // DEEP (FW110-FU-B): the simulator's `record_history_before_exit`
        // `HistoryKind::Deep` arm snapshots the FULL active descendant set
        // (`find_active_in_subtree` — for a single-region nested composite
        // that is exactly the innermost active leaf, since `active_states`
        // holds only leaf-like states — `is_leaflike`), and `resolve_target`
        // returns that remembered leaf path. So the recorded slot holds the
        // *deep leaf* (not a direct child). The match key is therefore the
        // set of all possible *deep leaves*; the entry chain re-enters every
        // intermediate composite from the owning composite's region down to
        // that leaf and then the leaf (the simulator's `entry_path` from the
        // composite to the remembered leaf — see `restore_deep_leaf_chains`).
        // The single-`StateId_t` `_history_{name}` slot is sufficient here:
        // for a non-parallel nested composite the deep leaf already lives in
        // `_active[0]` at record time, so `record` is unchanged. History
        // ACROSS PARALLEL REGIONS remains a documented limitation (module
        // header §scope); the fixture exercises a single-region nested
        // composite only — the anti-scope-creep discipline.
        //
        // Exactly one branch fires at runtime (the recorded leaf, or the
        // default normalised above — for deep, the default_target is in the
        // deep-leaf set when it is itself a leaf, exactly like the simulator
        // returning `default_target` on first entry).
        let chains = match history_kind_of(ctx, &rec.ir_id) {
            Some(fsm_ir::HistoryKind::Deep) => restore_deep_leaf_chains(ctx, rec),
            // Shallow (or, defensively, an absent kind — treat as shallow,
            // the pre-FW110 behaviour) keeps the byte-identical direct-child
            // restore (this is the `traffic-light` shallow-history path; it
            // MUST stay byte-identical — proven in the completion report).
            _ => restore_children_chains(ctx, rec),
        };
        for (child_idx, chain) in chains {
            let child_rec = ctx.index.get(child_idx);
            s.push_str(&format!(
                "    if (restore == {macro}_STATE_{cname}) {{\n",
                macro = macro_prefix,
                cname = child_rec.c_name,
            ));
            // Entry sequence for the restored child chain: composites/
            // parallels first, leaf last (root-first like `entry_path`).
            let mut leaf_slot: Option<(u8, &str)> = None;
            for &cs in &chain {
                let csr = ctx.index.get(cs);
                if csr.kind.is_active_at_rest() && csr.kind != StateRecordKind::Final {
                    if let Some(sr) = super::submachine::ref_state_member(ctx, &csr.ir_id) {
                        super::submachine::emit_sub_init(&sr, "        ", &mut s);
                    } else {
                        s.push_str(&format!(
                            "        {prefix}_entry_{cn}(m);\n",
                            prefix = prefix,
                            cn = csr.c_name,
                        ));
                    }
                }
                // Arm any timer the entered state owns (P0-4 parity with
                // the normal entry path — a restored leaf with an `after`
                // must re-arm just like a normally-entered one).
                for timer in &super::timer::collect_timers(ctx) {
                    if timer.owner_state == cs {
                        s.push_str(&format!(
                            "        m->_timer_{tname}_remaining_ms = {dur}u; /* P0-4: arm timer on history restore */\n",
                            tname = timer.field_name,
                            dur = timer.duration_ms,
                        ));
                    }
                }
                if matches!(
                    csr.kind,
                    StateRecordKind::Simple | StateRecordKind::Final | StateRecordKind::Submachine
                ) {
                    leaf_slot = Some((ctx.layout.slot(cs), &csr.c_name));
                }
                s.push_str(&format!(
                    "        #ifdef FSM_TRACE\n        fsm_trace_csv_append(m->_trace_ent, sizeof(m->_trace_ent), {lit});\n        #endif\n",
                    lit = super::trace_hook::c_string_literal(&csr.ir_id),
                ));
            }
            if let Some((slot, leaf_c)) = leaf_slot {
                s.push_str(&format!(
                    "        m->_active[{slot}] = {macro}_STATE_{leaf};\n",
                    slot = slot,
                    macro = macro_prefix,
                    leaf = leaf_c,
                ));
            }
            s.push_str("    }\n");
        }
        s.push_str("}\n\n");
    }
    s
}

/// The composite's region direct-child states, each paired with the entry
/// chain to run when *that* child is the restored one. For a simple/final/
/// submachine child the chain is `[child]`; for a composite/parallel child
/// the chain is `[child, …initial expansion…]` (UML shallow history: only
/// the direct child is remembered; deeper config is re-initialised).
fn restore_children_chains(
    ctx: &MachineEmitCtx<'_>,
    composite_rec: &crate::state_index::StateRecord,
) -> Vec<(u8, Vec<u8>)> {
    let Some(c) = find_composite(ctx.machine, &composite_rec.ir_id) else {
        return Vec::new();
    };
    let mut out: Vec<(u8, Vec<u8>)> = Vec::new();
    // A composite has exactly one region in v1.0 (parallel is a separate
    // node kind); iterate the first region's direct-child states.
    if let Some(region) = c.regions.first() {
        for st in &region.states {
            let id = match st {
                StateNode::Simple(x) => &x.id,
                StateNode::Composite(cc) => &cc.id,
                StateNode::Parallel(p) => &p.id,
                StateNode::Final(f) => &f.id,
                StateNode::Submachine(sm) => &sm.id,
                // Pseudo-states (initial / the history pseudo itself /
                // choice / …) are never a recorded shallow-history child.
                _ => continue,
            };
            let Some(child_idx) = ctx.index.lookup(id) else {
                continue;
            };
            let mut chain = vec![child_idx];
            // A composite/parallel direct child re-initialises deeper.
            expand_initial_chain(ctx, child_idx, &mut chain);
            out.push((child_idx, chain));
        }
    }
    out
}

/// The `history_kind` of the `history` pseudo declared on the composite
/// whose IR id is `composite_ir_id`, if any. Reads the SAME `c.history`
/// `HistoryObject` `find_history_default_idx` reads — no new parse, no
/// re-derivation; the analyzer already validated it (Doc 09 §4.8).
fn history_kind_of(ctx: &MachineEmitCtx<'_>, composite_ir_id: &str) -> Option<fsm_ir::HistoryKind> {
    fn walk_states<'a>(
        states: &'a [StateNode],
        composite_id: &str,
    ) -> Option<&'a fsm_ir::HistoryObject> {
        for s in states {
            match s {
                StateNode::Composite(c) => {
                    if c.id == composite_id {
                        return c.history.as_ref();
                    }
                    for r in &c.regions {
                        if let Some(hit) = walk_states(&r.states, composite_id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Parallel(p) => {
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
    walk_states(&ctx.machine.root.states, composite_ir_id).map(|h| h.history_kind)
}

/// FW110-FU-B — the DEEP-history restore chains. For every possible
/// **deep leaf** anywhere in the owning composite's subtree, the entry
/// chain to run when *that* leaf is the remembered one: every intermediate
/// composite/parallel ancestor strictly between the owning composite
/// (EXCLUSIVE — the static `entry_path` in `transition.rs` already enters
/// it) and the leaf, in root-first order, then the leaf itself.
///
/// This mirrors the shipped simulator EXACTLY: `record_history_before_exit`'s
/// `HistoryKind::Deep` arm stores `find_active_in_subtree` (the active
/// *leaf-like* descendants — `is_leaflike` is Simple/Final/SubmachineRef);
/// `resolve_target` returns that remembered leaf path; the transition driver
/// then runs `entry_path(lca, leaf)` (root-first state-node ancestors down
/// to the leaf) + `expand_initial(leaf)` (a leaf expands to nothing) and
/// pushes every entered state into `entered_all`, with the leaf landing in
/// `active_states` (`is_leaflike`). The chain produced here is precisely
/// that `entry_path` restricted to the descendants of the owning composite
/// (its own entry having already been emitted statically). UML deep history:
/// the FULL nested configuration is remembered and restored (NOT
/// re-initialised — the shallow contrast).
///
/// No initial re-expansion is appended: a deep leaf is leaf-like, so the
/// simulator's `expand_initial(leaf)` yields nothing; the remembered state
/// IS the resting leaf. (A composite/parallel can never be a `is_leaflike`
/// member of `active_states`, hence never a recorded deep-history slot — so
/// every match key here is a true leaf and the chain ends at it.)
fn restore_deep_leaf_chains(
    ctx: &MachineEmitCtx<'_>,
    composite_rec: &crate::state_index::StateRecord,
) -> Vec<(u8, Vec<u8>)> {
    let Some(c) = find_composite(ctx.machine, &composite_rec.ir_id) else {
        return Vec::new();
    };
    // Depth-first walk of the composite's region subtree. `path` is the
    // stack of intermediate composite/parallel container states (NOT the
    // owning composite, NOT pseudo-states); at each leaf we emit
    // `(leaf_idx, [..path.., leaf_idx])`.
    fn walk(
        ctx: &MachineEmitCtx<'_>,
        states: &[StateNode],
        path: &mut Vec<u8>,
        out: &mut Vec<(u8, Vec<u8>)>,
    ) {
        for st in states {
            match st {
                StateNode::Simple(x) => {
                    if let Some(idx) = ctx.index.lookup(&x.id) {
                        let mut chain = path.clone();
                        chain.push(idx);
                        out.push((idx, chain));
                    }
                }
                StateNode::Final(f) => {
                    if let Some(idx) = ctx.index.lookup(&f.id) {
                        let mut chain = path.clone();
                        chain.push(idx);
                        out.push((idx, chain));
                    }
                }
                StateNode::Submachine(sm) => {
                    if let Some(idx) = ctx.index.lookup(&sm.id) {
                        let mut chain = path.clone();
                        chain.push(idx);
                        out.push((idx, chain));
                    }
                }
                StateNode::Composite(cc) => {
                    if let Some(idx) = ctx.index.lookup(&cc.id) {
                        path.push(idx);
                        for r in &cc.regions {
                            walk(ctx, &r.states, path, out);
                        }
                        path.pop();
                    }
                }
                StateNode::Parallel(p) => {
                    if let Some(idx) = ctx.index.lookup(&p.id) {
                        path.push(idx);
                        for r in &p.regions {
                            walk(ctx, &r.states, path, out);
                        }
                        path.pop();
                    }
                }
                // Pseudo-states (initial / the history pseudo itself /
                // choice / junction / fork / join) are never an active
                // leaf-like state — never a recorded deep-history slot.
                _ => {}
            }
        }
    }
    let mut out: Vec<(u8, Vec<u8>)> = Vec::new();
    let mut path: Vec<u8> = Vec::new();
    // A composite has exactly one region in v1.0 (parallel is a distinct
    // node kind); the recursion above still descends nested parallels for
    // robustness (their slot/entry handling is the documented limitation —
    // see the module header §scope; this fixture is single-region nested).
    if let Some(region) = c.regions.first() {
        walk(ctx, &region.states, &mut path, &mut out);
    }
    out
}

/// Append the initial-chain expansion of `state_idx` (NOT including
/// `state_idx`) — composites follow their region initial, parallels follow
/// every region initial. Mirrors the simulator's `expand_initial`.
fn expand_initial_chain(ctx: &MachineEmitCtx<'_>, state_idx: u8, out: &mut Vec<u8>) {
    let rec = ctx.index.get(state_idx);
    match rec.kind {
        StateRecordKind::Composite => {
            if let Some(c) = find_composite(ctx.machine, &rec.ir_id) {
                if let Some(region) = c.regions.first() {
                    if let Some(init_idx) = ctx.index.lookup(&region.initial) {
                        for leaf in resolve_initial_chain(ctx, init_idx) {
                            out.push(leaf);
                            expand_initial_chain(ctx, leaf, out);
                        }
                    }
                }
            }
        }
        StateRecordKind::Parallel => {
            if let Some(p) = find_parallel(ctx.machine, &rec.ir_id) {
                for region in &p.regions {
                    if let Some(init_idx) = ctx.index.lookup(&region.initial) {
                        for leaf in resolve_initial_chain(ctx, init_idx) {
                            out.push(leaf);
                            expand_initial_chain(ctx, leaf, out);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Follow an initial-pseudo chain to the first concrete state. Bounded
/// against a malformed self-referential initial.
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

/// The composite's history `default_target` resolved to a codegen index.
fn find_history_default_idx(ctx: &MachineEmitCtx<'_>, composite_ir_id: &str) -> Option<u8> {
    fn walk_states<'a>(
        states: &'a [StateNode],
        composite_id: &str,
    ) -> Option<&'a fsm_ir::HistoryObject> {
        for s in states {
            match s {
                StateNode::Composite(c) => {
                    if c.id == composite_id {
                        return c.history.as_ref();
                    }
                    for r in &c.regions {
                        if let Some(hit) = walk_states(&r.states, composite_id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Parallel(p) => {
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
    ctx.index.lookup(&history.default_target)
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
