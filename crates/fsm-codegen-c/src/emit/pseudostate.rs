//! Runtime `choice` / `junction` pseudostate resolution lowering
//! (FW110-FU-A; Doc 09 §4.7).
//!
//! ## The defect this closes
//!
//! `choice` / `junction` pseudostates were INDEXED as state records + slots
//! (`state_index.rs`, `region_layout.rs`) but **no `emit/` site lowered the
//! runtime guard-chain resolution**: a transition whose target was a choice
//! emitted `m->_active[0] = <CHOICE pseudostate>` and rested there forever
//! (the FW110 differential's `[DIFF] sim=[] gen=[s-Router-Decide]`). This
//! module emits the missing guard chain.
//!
//! ## The unforked reference — `fsm_simulator::resolve_target`
//!
//! The shipped, UNFORKED reference is
//! `fsm_simulator::interpreter::resolve_target` (interpreter.rs:1572-1612)
//! and its single caller `execute_one_transition` (interpreter.rs:1356-1391).
//! `resolve_target` for a `Choice { branches } | Junction { branches }`:
//!
//!   1. iterate branches **top-to-bottom**; the FIRST whose guard evaluates
//!      `true` wins (`Ok(true) ⇒ pick; break`); an `[else]` branch
//!      (`GuardExpr::Else`) is remembered and wins iff no guarded branch
//!      matched (`pick.or(else_branch)`);
//!   2. if NO branch matched and there is NO `[else]` ⇒ a runtime error
//!      (`StepError::Internal("choice {target} has no matching branch")`);
//!   3. run the winning branch's `actions` (`execute_statements`) BEFORE
//!      recursing;
//!   4. **recurse** on the branch's own `target` (`resolve_target(...,
//!      &branch.target, ...)`) — so a branch may itself target another
//!      Choice / Junction / `Initial` (transitive chaining);
//!   5. an `Initial { target }` target recurses to its `target`.
//!
//! Its caller then, for the resolved concrete state(s) (interpreter.rs
//! 1360-1391): walks `entry_path(lca, tgt)` running `run_entry` for each and
//! pushing into `entered_all`, then `expand_initial(tgt)` (composite /
//! parallel initial chain) likewise, then pushes the leaf-like resolved
//! state(s) into `active_states`, then arms entry timers.
//!
//! ## What this emitter produces (byte-mirrors the above)
//!
//! For a transition whose target is a Choice / Junction, the caller
//! (`emit_transition_body`) suppresses the static `_active[]` write + static
//! `entry_path` loop (exactly as the FW1-FU-2 History precedent suppresses
//! them for a history target) and calls [`emit_choice_resolution`], which
//! emits:
//!
//! ```c
//! if (<guard0 C-expr>) {
//!     <branch-0 actions>
//!     <enter resolved target of branch 0: ancestor _entry_X's,
//!      m->_active[slot] = STATE_R, initial-expansion, trace ent appends>
//! } else if (<guard1 C-expr>) {
//!     ...
//! } else {                       /* the [else] branch, if any */
//!     <branch-else actions + enter>
//! }
//! /* no [else] and no guarded match ⇒ the simulator errors; the C traps */
//! else { fsm_hal_assert(false, "FSM-E0100: choice <id> no matching branch"); }
//! ```
//!
//! recursing transitively when a branch's own target is another Choice /
//! Junction / Initial. The terminal concrete-state entry sequence is the
//! SAME one a direct transition to that state emits (ancestor `_entry_X`
//! chain from the effective LCA down, the leaf `m->_active[slot]` write,
//! `emit_initial_expansion_for_target` for composite/parallel, and a
//! `m->_trace_ent` append for every `is_active_at_rest` state entered) — so
//! the generated C's resulting `StepRecord` (`cfgA`/`ent`) is byte-identical
//! to the shipped simulator's. `dst=` stays the transition's literal target
//! (the choice id) on BOTH sides (the C already emits
//! `c_string_literal(&t.target)`; the simulator records `t.target`), so it
//! matches with no special handling.
//!
//! ## FSM_TRACE discipline (the keystone)
//!
//! Every emitted line is PRODUCTION C **except** the `m->_trace_ent` appends,
//! which are `#ifdef FSM_TRACE`-gated exactly like every other trace tap
//! (`trace_hook::emit_trace_record_transition`). `#ifndef FSM_TRACE` the
//! emitted resolution is pure state/entry logic — no trace surface — so the
//! production C of a choice-bearing FSM gains only the (correct, previously
//! missing) guard chain, and a non-choice FSM's generated C is byte-unchanged
//! (this module is only ever reached when `target_idx` is Choice/Junction).

use std::collections::HashSet;

use fsm_ir::{ChoiceBranch, GuardExpr, StateNode};

use crate::expr::emit_guard;
use crate::state_index::StateRecordKind;

use super::MachineEmitCtx;

/// Is `idx` a Choice or Junction pseudostate record? (The two share the
/// branch shape — Doc 09 §4.7 — and `resolve_target` treats them
/// identically: `NodeKind::Choice { branches } | NodeKind::Junction {
/// branches }`. The codegen mirrors that.)
pub fn is_choice_or_junction(ctx: &MachineEmitCtx<'_>, idx: u8) -> bool {
    matches!(
        ctx.index.get(idx).kind,
        StateRecordKind::Choice | StateRecordKind::Junction
    )
}

/// Look up the branch list of a Choice / Junction by IR id. Mirrors the
/// simulator reading `node.kind` → `NodeKind::Choice { branches } |
/// Junction { branches }`.
fn branches_of<'a>(m: &'a fsm_ir::MachineObject, ir_id: &str) -> Option<&'a [ChoiceBranch]> {
    fn walk<'a>(states: &'a [StateNode], id: &str) -> Option<&'a [ChoiceBranch]> {
        for s in states {
            match s {
                StateNode::Choice(c) if c.id == id => return Some(&c.branches),
                StateNode::Junction(j) if j.id == id => return Some(&j.branches),
                StateNode::Composite(c) => {
                    for r in &c.regions {
                        if let Some(b) = walk(&r.states, id) {
                            return Some(b);
                        }
                    }
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
                        if let Some(b) = walk(&r.states, id) {
                            return Some(b);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&m.root.states, ir_id)
}

/// Emit the guard-chain resolution for a transition whose target is the
/// Choice / Junction at `choice_idx`. `lca` is the transition's effective
/// LCA index (the simulator passes `effective_lca(t)` to its post-resolve
/// `entry_path`); the terminal concrete-state entry sequence is built from
/// it so it byte-matches a direct transition to the resolved state.
///
/// `payload_prefix` / `pad` / the timer set / the entered-trace sink are
/// threaded through unchanged from `emit_transition_body`.
pub fn emit_choice_resolution(
    ctx: &MachineEmitCtx<'_>,
    choice_idx: u8,
    lca: u8,
    payload_prefix: &str,
    pad: &str,
    out: &mut String,
) {
    let mut visited: HashSet<u8> = HashSet::new();
    emit_resolve(ctx, choice_idx, lca, payload_prefix, pad, &mut visited, out);
}

/// Recursively emit the resolution of pseudostate `pseudo_idx`. Mirrors
/// `resolve_target`'s `Choice|Junction` arm structurally: an `if / else if /
/// else` chain over the branches in document order, `[else]` as the trailing
/// `else`, a no-match trap when there is no `[else]`. A branch whose own
/// target is another Choice/Junction/Initial recurses (transitive chaining).
///
/// `visited` guards against a malformed cyclic pseudostate graph (the
/// analyzer rejects choice cycles, but a defensive bound keeps codegen from
/// emitting non-terminating C — the `resolve_initial_chain` `bounce` guard
/// precedent). On a detected cycle we emit the same runtime trap as a
/// no-match (a cyclic choice can never resolve — behaviourally a no-match).
fn emit_resolve(
    ctx: &MachineEmitCtx<'_>,
    pseudo_idx: u8,
    lca: u8,
    payload_prefix: &str,
    pad: &str,
    visited: &mut HashSet<u8>,
    out: &mut String,
) {
    let rec = ctx.index.get(pseudo_idx);
    let pseudo_ir = rec.ir_id.clone();

    if !visited.insert(pseudo_idx) {
        emit_no_match_trap(ctx, &pseudo_ir, pad, out);
        return;
    }

    let Some(branches) = branches_of(ctx.machine, &pseudo_ir) else {
        // Indexed as a choice but no branch list found — malformed IR the
        // analyzer should have rejected. Trap rather than emit nothing
        // (a silent fall-through would rest on the pseudostate, the very
        // bug this wave fixes).
        emit_no_match_trap(ctx, &pseudo_ir, pad, out);
        visited.remove(&pseudo_idx);
        return;
    };

    // Partition into guarded branches (document order) and the `[else]`
    // (if any) — `resolve_target` does exactly this: it iterates branches
    // and collects `else_branch` separately, then uses
    // `pick.or(else_branch)`. NOTE on faithful `[else]` selection: the
    // simulator's loop does `if matches!(b.guard, Else) { else_branch =
    // Some(b.clone()); continue; }` for EVERY `Else` branch — so when the
    // IR is malformed and contains MULTIPLE `[else]` branches, the **LAST**
    // one wins (each iteration overwrites `else_branch`). A well-formed FSM
    // has exactly one `[else]` (the analyzer rejects >1), so first==last
    // and this is byte-identical to the well-formed path; mirroring the
    // last-wins keeps the codegen == the shipped simulator even on the
    // malformed IR a buggy upstream lowering can produce (the no-game
    // discipline: match the oracle's ACTUAL behaviour, not an idealised
    // one).
    let guarded: Vec<&ChoiceBranch> = branches
        .iter()
        .filter(|b| !matches!(b.guard, GuardExpr::Else))
        .collect();
    let else_branch: Option<&ChoiceBranch> = branches
        .iter()
        .filter(|b| matches!(b.guard, GuardExpr::Else))
        .last();

    for (i, b) in guarded.iter().enumerate() {
        let cond = emit_guard(&b.guard, "m->context", payload_prefix);
        let kw = if i == 0 { "if" } else { "} else if" };
        out.push_str(&format!(
            "{pad}{kw} ({cond}) {{\n",
            pad = pad,
            kw = kw,
            cond = cond,
        ));
        emit_branch_body(ctx, b, lca, payload_prefix, pad, visited, out);
    }

    if let Some(eb) = else_branch {
        if guarded.is_empty() {
            // No guarded branches at all — the `[else]` is unconditional.
            // (`resolve_target` returns `else_branch` directly; emit the
            // body with no surrounding `if`.)
            emit_branch_body(ctx, eb, lca, payload_prefix, pad, visited, out);
        } else {
            out.push_str(&format!("{pad}}} else {{\n", pad = pad));
            emit_branch_body(ctx, eb, lca, payload_prefix, pad, visited, out);
            out.push_str(&format!("{pad}}}\n", pad = pad));
        }
    } else if !guarded.is_empty() {
        // No `[else]`: close the chain with a trap mirroring
        // `resolve_target`'s `.ok_or_else(|| StepError::Internal("choice
        // {target} has no matching branch"))` — the FSM-E0100-class
        // behaviour. The analyzer warns on a choice without `[else]`; at
        // runtime an unmatched choice is a hard fault, not a silent rest.
        out.push_str(&format!("{pad}}} else {{\n", pad = pad));
        emit_no_match_trap(ctx, &pseudo_ir, &format!("{pad}    "), out);
        out.push_str(&format!("{pad}}}\n", pad = pad));
    }
    // (guarded.is_empty() && else_branch.is_some() already emitted its body
    // unwrapped above; nothing to close.)

    visited.remove(&pseudo_idx);
}

/// Emit one branch's body: its `actions` (the simulator runs
/// `execute_statements(rt, &branch.actions, ...)` BEFORE recursing), then
/// either recurse (target is another pseudostate) or emit the resolved
/// concrete-state entry sequence (target is a real state).
fn emit_branch_body(
    ctx: &MachineEmitCtx<'_>,
    b: &ChoiceBranch,
    lca: u8,
    payload_prefix: &str,
    pad: &str,
    visited: &mut HashSet<u8>,
    out: &mut String,
) {
    let inner = format!("{pad}    ");

    // Branch actions FIRST (resolve_target: `execute_statements(rt,
    // &branch.actions, ...)` then `resolve_target(rt, &branch.target, ...)`).
    let stmt_ctx = crate::stmt::StmtContext {
        machine_prefix: ctx.type_prefix(),
        ctx_prefix: "m->context",
        payload_prefix,
    };
    out.push_str(&crate::stmt::emit_stmts(&b.actions, &stmt_ctx, inner.len()));

    let Some(tgt_idx) = ctx.index.lookup(&b.target) else {
        // Branch target is not an indexed state id. `resolve_target` does
        // NOT trap here — it mirrors its OWN entry guard `let Some(node) =
        // rt.machine.node(target) else { return Ok(vec![target.to_string()])
        // }`: an unknown target id is **silently returned** and the caller's
        // `entry_path` / `is_leaflike` then drop it (the state never becomes
        // active → an empty config, NO runtime error). This is DISTINCT from
        // the "no branch matched and no `[else]`" case (the FSM-E0100 trap
        // below), which is `resolve_target`'s separate `.ok_or_else(||
        // StepError::Internal(...))`. So to byte-mirror the shipped
        // simulator we must emit **nothing** here (no slot write, no entry,
        // no trace) — exactly the simulator's silent no-op. (In a
        // well-formed FSM every branch target IS an indexed state and this
        // arm is unreachable; it only triggers under malformed IR a buggy
        // upstream lowering can produce — e.g. the FW110-FU-A-discovered
        // analyzer defect that leaves choice-branch targets unresolved as
        // raw DSL names. Mirroring the simulator's silent behaviour keeps
        // the differential a clean catalogued RECORD-MODEL RED rather than a
        // process abort, and never games the catalogue: the codegen ==
        // the oracle by construction even on the malformed IR.)
        return;
    };

    let tgt_kind = ctx.index.get(tgt_idx).kind;
    if matches!(
        tgt_kind,
        StateRecordKind::Choice | StateRecordKind::Junction
    ) {
        // Transitive chaining: the branch targets another pseudostate.
        // `resolve_target` recurses here.
        emit_resolve(ctx, tgt_idx, lca, payload_prefix, &inner, visited, out);
    } else if tgt_kind == StateRecordKind::Initial {
        // `resolve_target`'s `NodeKind::Initial { target } =>
        // resolve_target(rt, &target.clone(), ...)`: an Initial pseudo
        // forwards to its `target`. Resolve the initial chain to its first
        // concrete/pseudo state and continue.
        emit_initial_redirect(ctx, tgt_idx, lca, payload_prefix, &inner, visited, out);
    } else {
        // A concrete (Simple / Composite / Parallel / Final / Submachine)
        // resolved target. Emit the SAME entry sequence a direct transition
        // to it would (the caller of `resolve_target` runs
        // `entry_path(lca,tgt)` + `expand_initial(tgt)`).
        emit_resolved_entry(ctx, tgt_idx, lca, &inner, out);
    }
}

/// `resolve_target`'s `Initial { target } => resolve_target(&target)`:
/// follow the initial pseudo's `target` (which may itself be a state or
/// another pseudo) and continue resolution.
fn emit_initial_redirect(
    ctx: &MachineEmitCtx<'_>,
    initial_idx: u8,
    lca: u8,
    payload_prefix: &str,
    pad: &str,
    visited: &mut HashSet<u8>,
    out: &mut String,
) {
    // An Initial with no resolvable `target`, or a `target` that is not an
    // indexed state id, is `resolve_target`'s SILENT `node(target) else {
    // return Ok(vec![target.to_string()]) }` fallback (the caller then
    // drops the unknown id) — NOT a trap. Emit nothing, mirroring the
    // simulator (same rationale as the unresolvable choice-branch-target
    // arm above; keep codegen == oracle even on malformed IR).
    let init_ir = ctx.index.get(initial_idx).ir_id.clone();
    let Some(target_id) = find_initial_target(ctx.machine, &init_ir) else {
        return;
    };
    let Some(tgt_idx) = ctx.index.lookup(&target_id) else {
        return;
    };
    let tgt_kind = ctx.index.get(tgt_idx).kind;
    if matches!(
        tgt_kind,
        StateRecordKind::Choice | StateRecordKind::Junction
    ) {
        emit_resolve(ctx, tgt_idx, lca, payload_prefix, pad, visited, out);
    } else if tgt_kind == StateRecordKind::Initial {
        emit_initial_redirect(ctx, tgt_idx, lca, payload_prefix, pad, visited, out);
    } else {
        emit_resolved_entry(ctx, tgt_idx, lca, pad, out);
    }
}

/// Emit the concrete-state entry sequence for a choice-resolved target
/// `tgt_idx`, byte-mirroring what `execute_one_transition` does for a
/// resolved target (interpreter.rs:1360-1391): `entry_path(lca, tgt)` ⇒
/// `run_entry` each + push to `entered_all`; then `expand_initial(tgt)` ⇒
/// `run_entry` each + push; then the leaf is active. The codegen analogue:
///
///   • for each ancestor on the path `(lca, tgt]` that
///     `is_active_at_rest`: `<M>_entry_X(m)` + arm its entry timers +
///     `#ifdef FSM_TRACE` append to `m->_trace_ent`;
///   • write the leaf `m->_active[slot] = STATE_tgt`;
///   • for the leaf itself (Simple/Final/Submachine): its `_entry_X` (unless
///     Final / submachine-ref) + timer-arm + trace `ent`;
///   • if the leaf is Composite/Parallel: reuse the existing
///     `transition::emit_initial_expansion_for_target` (the SAME helper a
///     direct composite/parallel-targeted transition uses — identical
///     initial-chain expansion) + the composite/parallel's own `_entry_X` +
///     trace `ent`.
///
/// The path is computed with the same `parents`/`ROOT_SENTINEL` walk the
/// simulator's `entry_path` uses, so the ancestor set is identical.
fn emit_resolved_entry(
    ctx: &MachineEmitCtx<'_>,
    tgt_idx: u8,
    lca: u8,
    pad: &str,
    out: &mut String,
) {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let all_timers = super::timer::collect_timers(ctx);

    // entry_path(lca, tgt): walk tgt upward to (but not including) lca,
    // reverse → root-first. Identical to `fsm_simulator::entry_path` and to
    // `emit::entry_exit::entry_path`'s body (the same parents-walk; we
    // recompute it here because the choice's resolved target is NOT the
    // transition's static `t.target`).
    let mut chain: Vec<u8> = Vec::new();
    let mut cur = tgt_idx;
    while cur != lca && cur != crate::ROOT_SENTINEL {
        chain.push(cur);
        cur = ctx.parents.parents[cur as usize];
    }
    chain.reverse();

    let arm_timers = |state_idx: u8, out: &mut String| {
        for timer in &all_timers {
            if timer.owner_state == state_idx {
                out.push_str(&format!(
                    "{pad}m->_timer_{tname}_remaining_ms = {dur}u; /* P0-4: arm timer on choice-resolved entry */\n",
                    pad = pad,
                    tname = timer.field_name,
                    dur = timer.duration_ms,
                ));
            }
        }
    };
    let trace_ent = |ir_id: &str, out: &mut String| {
        out.push_str(&format!(
            "{pad}#ifdef FSM_TRACE\n{pad}fsm_trace_csv_append(m->_trace_ent, sizeof(m->_trace_ent), {lit});\n{pad}#endif /* FSM_TRACE */\n",
            pad = pad,
            lit = super::trace_hook::c_string_literal(ir_id),
        ));
    };

    // Ancestors on the path (lca, tgt] EXCLUDING the leaf itself — the leaf
    // is handled below (slot write + entry). The simulator's
    // `entry_path`+`run_entry` enters every state on the path incl. the
    // target; we split leaf vs ancestors so the leaf's `_active[slot]` write
    // (the resting config) is emitted, mirroring the static-target path.
    for &anc_idx in chain.iter().filter(|&&i| i != tgt_idx) {
        let anc = ctx.index.get(anc_idx);
        if anc.kind.is_active_at_rest() && anc.kind != StateRecordKind::Final {
            if let Some(sr) = super::submachine::ref_state_member(ctx, &anc.ir_id) {
                super::submachine::emit_sub_init(&sr, pad, out);
            } else {
                out.push_str(&format!(
                    "{pad}{prefix}_entry_{name}(m);\n",
                    pad = pad,
                    prefix = prefix,
                    name = anc.c_name,
                ));
            }
            arm_timers(anc_idx, out);
        }
        // Trace-record set mirrors the simulator's `entered_all` EXACTLY:
        // every `is_active_at_rest` state on the path (Final included — the
        // simulator's `enter_state_path` pushes Final too; FW109).
        if anc.kind.is_active_at_rest() {
            trace_ent(&anc.ir_id, out);
        }
    }

    // The resolved leaf itself.
    let tgt = ctx.index.get(tgt_idx);
    let slot = ctx.layout.slot(tgt_idx);
    match tgt.kind {
        StateRecordKind::Simple | StateRecordKind::Final | StateRecordKind::Submachine => {
            out.push_str(&format!(
                "{pad}m->_active[{slot}] = {macro}_STATE_{name};\n",
                pad = pad,
                slot = slot,
                macro = macro_prefix,
                name = tgt.c_name,
            ));
            if tgt.kind != StateRecordKind::Final {
                if let Some(sr) = super::submachine::ref_state_member(ctx, &tgt.ir_id) {
                    super::submachine::emit_sub_init(&sr, pad, out);
                } else {
                    out.push_str(&format!(
                        "{pad}{prefix}_entry_{name}(m);\n",
                        pad = pad,
                        prefix = prefix,
                        name = tgt.c_name,
                    ));
                }
                arm_timers(tgt_idx, out);
            }
            trace_ent(&tgt.ir_id, out);
        }
        StateRecordKind::Composite | StateRecordKind::Parallel => {
            // The composite/parallel container is entered (its `_entry_X` +
            // trace ent), THEN its initial chain expands — exactly what the
            // simulator does (`entry_path` enters the composite; then
            // `expand_initial` enters the initial leaf). The slot write for
            // a composite/parallel is performed by the initial-expansion
            // (the leaf takes the slot); the container itself has no resting
            // `_active[]` value of its own in the v1.0 single-region layout
            // (mirrors the static-target path, which also does NOT write the
            // composite's slot — `emit_initial_expansion_for_target` writes
            // the leaf slot).
            out.push_str(&format!(
                "{pad}{prefix}_entry_{name}(m);\n",
                pad = pad,
                prefix = prefix,
                name = tgt.c_name,
            ));
            arm_timers(tgt_idx, out);
            trace_ent(&tgt.ir_id, out);
            super::transition::emit_initial_expansion_for_target(ctx, tgt_idx, pad, out);
        }
        _ => {
            // Initial/Choice/Junction are handled by the recursion in
            // `emit_branch_body`; Root/EntryPoint/ExitPoint/Fork/Join as a
            // choice branch target are out of the v1.0 corpus. Trap rather
            // than emit a silent rest (never game the catalogue by hiding a
            // gap).
            emit_no_match_trap(ctx, &tgt.ir_id, pad, out);
        }
    }
}

/// The no-matching-branch runtime trap — the codegen analogue of
/// `resolve_target`'s `StepError::Internal("choice {target} has no matching
/// branch")` (the FSM-E0100-class behaviour). Uses the same `fsm_hal_assert`
/// abort hook the runtime already relies on for invariant violations (it is
/// declared in `fsm_hal.h` and used elsewhere in the generated runtime), so
/// this needs no new HAL surface. Production AND FSM_TRACE identical (it is
/// behaviour, not a trace tap).
fn emit_no_match_trap(ctx: &MachineEmitCtx<'_>, pseudo_ir: &str, pad: &str, out: &mut String) {
    let _ = ctx;
    out.push_str(&format!(
        "{pad}fsm_hal_assert(false, \"FSM-E0100: choice/junction '{id}' has no matching branch\");\n",
        pad = pad,
        id = pseudo_ir.replace('\\', "\\\\").replace('"', "\\\""),
    ));
}

/// Resolve an `Initial` pseudo's declared target id (the `target` field of
/// the `InitialPseudo` node). Mirrors
/// `transition::find_initial_target` (same walk); duplicated module-locally
/// to keep this emitter self-contained (it is a tiny pure IR lookup).
fn find_initial_target(machine: &fsm_ir::MachineObject, ir_id: &str) -> Option<String> {
    fn walk(states: &[StateNode], id: &str) -> Option<String> {
        for s in states {
            match s {
                StateNode::Initial(i) if i.id == id => return Some(i.target.clone()),
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
