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
//!
//! AUDIT_2026_06_06 §6 P2.2 — split from a single 1054-LOC `transition.rs`
//! into:
//!   - `mod.rs`       — the public `emit_transition_body` and
//!                      `emit_initial_expansion_for_target`
//!   - `find.rs`      — IR-tree lookups (initial target, composite, parallel)
//!   - `parallel.rs`  — sibling-region exit + FW109 record-set helpers
//!   - `composite.rs` — composite active-descendant exit-set walk
//!   - `enter.rs`     — initial-chain entry emission + initial-chain resolver

use fsm_ir::{BranchHint, TransitionKind, TransitionObject};

use crate::expr::emit_guard;
use crate::state_index::StateRecordKind;

use super::entry_exit::{entry_path, exit_path};
use super::MachineEmitCtx;

mod composite;
mod enter;
mod find;
mod parallel;

use self::composite::composite_descendant_leaves;
use self::enter::{emit_enter_chain, resolve_initial_chain};
use self::find::{find_composite, find_parallel};
use self::parallel::{
    collect_parallel_region_leaves_all, collect_parallel_region_leaves_excluding,
};

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
        // FW109 (Doc 32 §1 W1, the record-model determination; the FW1-FU-2
        // active-descendant class generalised from Composite to Parallel):
        // the shipped `fsm_simulator::execute_one_transition`'s `full_exits`
        // adds **every active descendant** of every exited state. When the
        // exited state is this Parallel, that is the runtime-active leaf of
        // EVERY region (the source region's too — it is an active descendant
        // of the Parallel — AND any **Final** leaf). The prior codegen
        // recorded only the Parallel itself in the trace exit-set (via the
        // static chain's `exited_ir`), so vending-machine's `RESET`
        // (`Operational -> Done`) recorded `ext=Operational` while the
        // oracle records `ext=Operational,PaymentFinal,SelectionFinal` — a
        // genuine trace-record-fidelity codegen bug (NOT a projection
        // artifact; the simulator's exit-set genuinely contains those
        // active Final leaves). Append, under `#ifdef FSM_TRACE` ONLY (the
        // production C is byte-unchanged — this is a trace tap), the
        // runtime-resolved active leaf of every region slot of the exited
        // Parallel. This is a record of the C's OWN `_active[]` (what it
        // observed), not a re-derivation of the simulator (the keystone,
        // Doc 32 §2). The Parallel itself is NOT re-appended here (the
        // static `exited_ir` already records it).
        let all_region_leaves = collect_parallel_region_leaves_all(ctx, parallel_idx);
        if !all_region_leaves.is_empty() {
            out.push_str(&format!(
                "{pad}#ifdef FSM_TRACE\n{pad}/* Parallel active-descendant exit-set (Doc 08 §6.1/§6.3, FW109) */\n",
                pad = pad,
            ));
            for leaf_idx in all_region_leaves {
                let leaf = ctx.index.get(leaf_idx);
                let slot = ctx.layout.slot(leaf_idx);
                out.push_str(&format!(
                    "{pad}if (m->_active[{slot}] == {macro}_STATE_{lname}) fsm_trace_csv_append(m->_trace_ext, sizeof(m->_trace_ext), {lit});\n",
                    pad = pad,
                    slot = slot,
                    macro = ctx.macro_prefix(),
                    lname = leaf.c_name,
                    lit = super::trace_hook::c_string_literal(&leaf.ir_id),
                ));
            }
            out.push_str(&format!("{pad}#endif /* FSM_TRACE */\n", pad = pad));
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
                if leaf.kind == StateRecordKind::Final {
                    // FW110: a **Final** active-descendant leaf has NO
                    // `_exit_<Final>` handler — the impl-header /
                    // entry-exit declaration passes deliberately omit
                    // entry/exit handlers for Final states
                    // (`impl_header.rs` `rec.kind == StateRecordKind::Final
                    // ⇒ continue`; `source.rs` guards `_exit_X` with
                    // `!= Final`). The shipped `fsm_simulator::run_exit`
                    // likewise runs NO exit action for a Final (it has no
                    // `exit` block) yet `full_exits`/`exit_set` DO record
                    // the active Final leaf in `exited_states`. So the
                    // codegen must mirror BOTH facts: emit the trace `ext`
                    // record (behavioural parity — the simulator records
                    // it) but emit NO `<M>_exit_<Final>(m)` call (the
                    // bug this fixes: the prior emitter called the
                    // never-declared `<M>_exit_<Final>`, breaking the
                    // -Werror build of any FSM whose composite has a
                    // completion/exit edge with a live Final descendant —
                    // e.g. a `done ->` cascade through Final states).
                    // Symmetric to the submachine-ref branch above
                    // (trace-record-but-no-`_exit_X`).
                    if !emitted_header {
                        out.push_str(&format!(
                            "{pad}/* Composite active-descendant exit-set (Doc 08 §6.1, FW1-FU-2; FW110 Final-leaf: trace-only, no _exit_<Final>) */\n",
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

    // FW110-FU-A: is the transition target a `choice` / `junction`
    // pseudostate? Like the History case above, the prior codegen wrote
    // `_active[slot] = STATE_<CHOICE>` (a pseudo-state, never a resting
    // config) and never resolved the guard chain — the machine rested on
    // the choice forever (the FW110 differential's `[DIFF] sim=[]
    // gen=[s-Router-Decide]`). When set, the static slot-write (step 3) and
    // the static `entry_path` loop + `emit_initial_expansion_for_target`
    // (step 4) are SUPPRESSED exactly as they are for a history target;
    // `super::pseudostate::emit_choice_resolution` instead emits the runtime
    // guard chain mirroring the shipped `fsm_simulator::resolve_target` +
    // its post-resolve `entry_path`/`expand_initial`, terminating each
    // branch in the resolved concrete state's full entry sequence (incl. the
    // `#ifdef FSM_TRACE` `m->_trace_ent` appends), so the emitted
    // `StepRecord` (`cfgA`/`ent`) byte-matches the simulator. (Internal
    // transitions never have a pseudostate target — guarded by the
    // `!Internal` checks below, same as history.)
    let choice_target_idx: Option<u8> =
        if super::pseudostate::is_choice_or_junction(ctx, target_idx) {
            Some(target_idx)
        } else {
            None
        };

    // 3. Update `_active[]` slot. Internal transitions leave the
    // configuration unchanged. A history target does NOT write the slot
    // here — `<M>_history_restore_*` writes the *resolved leaf* slot
    // (writing `STATE_HAuto`, a pseudo-state, would corrupt the config and
    // the cfgA projection, exactly the prior bug).
    // FW110-FU-A: a choice/junction target also does NOT write the slot
    // here — `emit_choice_resolution` writes the *resolved concrete leaf*
    // slot (writing `STATE_<CHOICE>`, a pseudo-state, would corrupt the
    // config + the cfgA projection — the exact prior bug, the History
    // analogue).
    if !matches!(t.kind, TransitionKind::Internal)
        && history_owner_idx.is_none()
        && choice_target_idx.is_none()
    {
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
    //
    // FW110-FU-A: SUPPRESSED for a choice/junction target. The static
    // `entry_path(t)` would walk the CHOICE pseudostate's static ancestor
    // chain — but the resolved concrete leaf (and the correct LCA→leaf
    // ancestor entry sequence) is only known at runtime via the guard
    // chain. `emit_choice_resolution` (called below) emits the entire
    // entry sequence for the resolved branch, including ancestors, so the
    // static loop here must NOT also run (it would double-enter ancestors
    // / enter the wrong path). Symmetric to how the History target relies
    // on `<M>_history_restore_*` for the runtime-resolved leaf — except a
    // choice has NO statically-entered owning composite at all, so the
    // WHOLE static entry is skipped (history keeps the owning composite's
    // static entry; a choice's resolved target may be anywhere).
    let static_entry: Vec<u8> = if choice_target_idx.is_some() {
        Vec::new()
    } else {
        entry_path(t, ctx.index, ctx.parents)
    };
    for entry_idx in static_entry {
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
    // (A choice/junction target's kind is Choice/Junction — never
    // Parallel/Composite — so `choice_target_idx.is_none()` is redundant
    // with the `matches!` below, but stated for intent: the
    // resolved-branch initial expansion is emitted by
    // `emit_choice_resolution`, not here.)
    if !matches!(t.kind, TransitionKind::Internal) && choice_target_idx.is_none() {
        let target_rec = ctx.index.get(target_idx);
        if matches!(
            target_rec.kind,
            StateRecordKind::Parallel | StateRecordKind::Composite
        ) {
            emit_initial_expansion_for_target(ctx, target_idx, &pad, out);
        }
    }

    // FW110-FU-A: the runtime choice/junction guard-chain resolution. The
    // owning context's exit + the transition action block already ran
    // above (steps 1-2, byte-identical to the History/normal path); now
    // emit the `if (<g0>) {<enter branch-0 target>} else if (<g1>) {…}
    // else {<[else] / trap>}` chain mirroring the shipped
    // `fsm_simulator::resolve_target` (interpreter.rs:1572-1612) + its
    // caller's post-resolve `entry_path(lca,tgt)` / `expand_initial`
    // (interpreter.rs:1360-1391). The terminal concrete-state entry
    // sequence is the SAME a direct transition to that state emits, so
    // the resulting `StepRecord` (`cfgA`/`ent`) is byte-identical to the
    // simulator's. `effective_lca(t)` is the transition's LCA — the same
    // value the simulator passes to its post-resolve `entry_path`.
    if let Some(choice_idx) = choice_target_idx {
        let lca = super::entry_exit::effective_lca(t, ctx.index, ctx.parents);
        super::pseudostate::emit_choice_resolution(ctx, choice_idx, lca, payload_prefix, &pad, out);
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
    //
    // FW109 (Doc 32 §1 W1, the record-model determination): the trace
    // entered/exited SET must mirror the shipped
    // `fsm_simulator::execute_one_transition`'s `entered_all`/`exited_all`
    // *exactly* — and the simulator records **Final** states in those sets
    // (`enter_state_path` pushes every `entry_path` state incl. Final into
    // `entered_all`; `is_leaflike` puts a Final leaf into `active_states`;
    // `full_exits`/`exit_set` symmetrically include the active Final leaf —
    // see interpreter.rs). The prior trace filter `&& != Final` wrongly
    // dropped Final from the *recorded* set (a genuine record-fidelity
    // codegen bug of the FW1-FU-2 active-descendant class, not a projection
    // artifact): vending-machine's `Dispensing -> SelectionFinal` /
    // `ChangeAvailable -> PaymentFinal` and submachine's `Established ->
    // s-Connection-Done` are real `entered_states`/`exited_states` the
    // oracle records and the C must too. The function-emission gate keeps
    // `&& != Final` (a Final state has no user `_entry_X`/`_exit_X`); ONLY
    // this trace-record set is corrected to `is_active_at_rest()` (which
    // already *includes* Final — the simulator's exact record set).
    //
    // FW110-FU-A: for a choice/junction target the STATIC `entry_path(t)`
    // resolves the CHOICE pseudostate's ancestor chain (and the choice
    // itself is filtered by `is_active_at_rest` anyway) — but the actually-
    // entered concrete state(s) are runtime-resolved by the guard chain.
    // `emit_choice_resolution` emits the `#ifdef FSM_TRACE`
    // `m->_trace_ent` append for every runtime-entered `is_active_at_rest`
    // state itself (the LCA→resolved-leaf path), exactly as
    // `<M>_history_restore_*` does for a history target's runtime leaf. So
    // the static `entered_ir` here MUST be empty for a choice target — the
    // resolver owns the entire `ent` set (a non-empty static set would
    // double-count or record the wrong path).
    let entered_ir: Vec<String> = if choice_target_idx.is_some() {
        Vec::new()
    } else {
        entry_path(t, ctx.index, ctx.parents)
            .into_iter()
            .filter(|i| ctx.index.get(*i).kind.is_active_at_rest())
            .map(|i| ctx.index.get(i).ir_id.clone())
            .collect()
    };
    let exited_ir: Vec<String> = exits
        .iter()
        .copied()
        .filter(|i| ctx.index.get(*i).kind.is_active_at_rest())
        .map(|i| ctx.index.get(i).ir_id.clone())
        .collect();
    out.push_str(&super::trace_hook::emit_trace_record_transition(
        t,
        &entered_ir,
        &exited_ir,
        &pad,
    ));
}

/// Emit the initial-chain expansion for a transition target that is a
/// composite or parallel state. For a parallel target, every region's
/// initial leaf is assigned to its slot and entry actions run; for a
/// composite, the single region's initial chain is followed.
pub(crate) fn emit_initial_expansion_for_target(
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
