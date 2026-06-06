//! Entry-chain emission — the recursive walker that turns an
//! initial-chain target (simple / final / submachine / composite /
//! parallel) into the corresponding sequence of `_active[]` writes,
//! `_entry_X` calls, timer arms, and `#ifdef FSM_TRACE` `_trace_ent`
//! appends. Mirrors the shipped simulator's `expand_initial` +
//! `run_entry` interleaving (Doc 08 §7).

use crate::emit::MachineEmitCtx;
use crate::state_index::StateRecordKind;

use super::find::{find_composite, find_initial_target, find_parallel};

pub(super) fn emit_enter_chain(
    ctx: &MachineEmitCtx<'_>,
    state_idx: u8,
    pad: &str,
    out: &mut String,
) {
    let rec = ctx.index.get(state_idx);
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let all_timers = crate::emit::timer::collect_timers(ctx);
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
    // FW110-FU-B: record the initial-chain-entered state in the trace `ent`
    // set, gated by `#ifdef FSM_TRACE`. The shipped `fsm_simulator` pushes
    // EVERY `expand_initial` state into `entered_all` (interpreter.rs
    // `execute_one_transition` step 6 — `run_entry(sid)` then
    // `entered_all.push(sid)` for each expanded `sid`), so a composite/
    // parallel transition target's initial-expanded inner states (e.g.
    // `WorkA --GO--> WorkB` entering `WorkB`'s initial leaf `Deep1`) are
    // recorded `ent` ids the C must report too. The static
    // `emit_trace_record_transition` only emits the `entry_path` ids (the
    // LCA→target chain); these expanded ids are the runtime analogue of the
    // simulator's `expand_initial` contribution and were a latent
    // record-fidelity gap (no prior BYTE_EQUAL fixture had a composite/
    // parallel *transition target* that initial-expands — `traffic-light`
    // only ever reaches its composite via init / shallow-history restore,
    // both of which trace separately). `#ifndef FSM_TRACE` strips this
    // entirely → the production C is byte-identical (the keystone).
    let trace_ent = |state_idx: u8, out: &mut String| {
        out.push_str(&format!(
            "{pad}#ifdef FSM_TRACE\n{pad}fsm_trace_csv_append(m->_trace_ent, sizeof(m->_trace_ent), {lit});\n{pad}#endif\n",
            pad = pad,
            lit = crate::emit::trace_hook::c_string_literal(&ctx.index.get(state_idx).ir_id),
        ));
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
            // Mirror `expand_initial`→`entered_all`: a leaf reached via the
            // initial chain IS pushed to the simulator's `entered_all`
            // (incl. Final — `is_leaflike(Final)` and `enter_state_path`
            // push it). Record it BEFORE the entry-action emission so the
            // raw append order is irrelevant (the step emitter sorts the
            // whole `_trace_ent` union — see trace_hook.rs).
            trace_ent(state_idx, out);
            if rec.kind != StateRecordKind::Final {
                // v1.1-W2d: a ref-state reached via an initial-chain
                // expansion (a `state X is Sub` as a composite's initial)
                // instantiates its sub-instance instead of a user
                // `_entry_X`.
                if let Some(sr) = crate::emit::submachine::ref_state_member(ctx, &rec.ir_id) {
                    crate::emit::submachine::emit_sub_init(&sr, pad, out);
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
            // The simulator's `expand_initial` pushes a composite reached
            // via the initial chain into `entered_all` too (it recurses
            // INTO it, having pushed it). Record it.
            trace_ent(state_idx, out);
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
            // As Composite: `expand_initial` pushes the parallel container
            // into `entered_all` before recursing its regions. Record it.
            trace_ent(state_idx, out);
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

pub(super) fn resolve_initial_chain(ctx: &MachineEmitCtx<'_>, mut cur: u8) -> Vec<u8> {
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
