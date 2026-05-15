//! Submachine ref-state codegen — Doc 08 §12, Doc 09 §4.11 (v1.1-W2d).
//!
//! A `state X is Sub` lowers to a [`fsm_ir::StateNode::Submachine`] ref-state
//! (W2b). Pre-W2d codegen treated it as an inert leaf (the defensive arm):
//! generated C compiled `-Werror`-clean but never instantiated the sub, never
//! delegated events into it, and never fired the parent `done ->` on the
//! sub's completion — the parent was behaviourally stuck on the ref-state.
//!
//! W2d makes the generated C run the sub-instance, byte-matching the merged
//! W2c simulator (`fsm-simulator/src/interpreter.rs` +
//! `examples/submachine/submachine.trace`). The model is a **nested value
//! member** (heap-free per Doc 02 G2 — exactly how the parallel `_active[]`,
//! the timer slots, and the defer buffer are nested as struct members):
//!
//! - The parent struct gains `Sub_t _sub_<refstate>;` (the referenced
//!   template is its own codegen unit, emitted first by
//!   `emit_machine_recursive`).
//! - **Entry** to the ref-state → `Sub_init(&m->_sub_<refstate>)` (the sub
//!   starts at its implicit initial — W2b emits `entry_points` empty;
//!   `sync_submachines` in W2c does the same `init`-shaped entry).
//! - **Dispatch** while the ref-state is the active leaf: the parent's own
//!   leaf-to-root transition walk runs FIRST (a ref-state `on EVT` / `done`
//!   transition wins — transition-wins, UML 2.5.1 §14.2.3.9.1, mirroring
//!   W2c's `select_transitions` before `try_delegate_to_submachine`); only
//!   if no parent transition fired for that region is the event delegated
//!   into `Sub_dispatch` (resolved by NAME into the sub's own event id
//!   space — W2c re-resolves by name via `event_id_by_name`).
//! - **Completion**: after delegation, if the sub-instance's active leaf is
//!   its `Final`, synthesise the parent `EVENT__COMPLETION` so the
//!   ref-state's `done -> Target` (a `TransitionKind::Completion` edge now
//!   emitted as a normal per-state case — see the `Submachine` arm added to
//!   `dispatch_switch` / `dispatch_table`) fires through the **existing**
//!   completion machinery. We never re-implement `done`; the sub-Final is
//!   only the trigger, exactly like W2c's "enqueue `Completion(ref_id)` and
//!   let the existing R1 path fire". Idempotent: once `done` fires the
//!   ref-state is exited, so the synthetic completion is not re-raised.
//! - **Teardown**: a value member needs no free; a ref-state self-transition
//!   (`on RECONNECT -> Connecting`) re-enters the ref-state, and the entry
//!   path re-calls `Sub_init`, resetting the sub fresh — mirroring W2c's
//!   `run_exit` teardown → re-sync re-instantiation.
//!
//! Bounded nesting: a submachine's own `submachines` is empty (W2b
//! `include_submachines:false`), so `Sub_t` is a finite struct; FSM-E0502
//! rejects static template cycles at analysis, so no runtime cycle guard is
//! emitted (the struct nesting provably terminates at compile time).

use fsm_ir::{StateNode, TransitionKind};

use crate::state_index::{c_ident, StateRecordKind};

use super::MachineEmitCtx;

/// One submachine ref-state discovered in the parent's root region, with the
/// pre-resolved identifiers codegen needs (the parent's C state id, the
/// referenced template's C type/macro prefix, the member name, and the
/// template's `Final` state — its completion trigger).
pub struct SubRef<'a> {
    /// The `SubmachineRef` IR node.
    pub sref: &'a fsm_ir::SubmachineRef,
    /// Parent state-index id for this ref-state (e.g. `CONNECTING`).
    pub parent_state_c: String,
    /// Referenced template machine (resolved via `machine.submachines`).
    pub template: &'a fsm_ir::MachineObject,
    /// Mixed-case C type prefix of the template (e.g. `Connection`).
    pub sub_type: String,
    /// Upper-case macro prefix of the template (e.g. `CONNECTION`).
    pub sub_macro: String,
    /// Nested value-member name on the parent struct (e.g. `_sub_CONNECTING`).
    pub member: String,
}

impl SubRef<'_> {
    /// `Sub_t` — the nested value member's C type.
    pub fn member_type(&self) -> String {
        format!("{}_t", self.sub_type)
    }

    /// C state id of the template's `Final` state (`CONNECTION_STATE_DONE`),
    /// the completion trigger. `None` if the template has no reachable
    /// `final` (degrades to "never completes" rather than mis-firing — Doc
    /// 09 §1 partial-IR principle; the analyzer flags an unreachable/missing
    /// final separately).
    pub fn final_state_c(&self) -> Option<String> {
        // The template lowers a single root-region `final NAME` to
        // `StateNode::Final`; mirror `state_index::c_ident(name)` so the id
        // matches the emitted `<MACRO>_STATE_<NAME>` enumerator.
        find_final_name(&self.template.root.states).map(|n| c_ident(&n))
    }
}

/// Collect every submachine ref-state in the parent machine's root region,
/// resolving each `submachine_id` to its template in `machine.submachines`.
///
/// W2b carries every `submachine Name { … }` as a `MachineObject` in
/// `machine.submachines` with `id == "m-<Name>"`, and the ref-state's
/// `submachine_id` is exactly that id, so the lookup is a direct match. An
/// unresolved ref (FSM-E0103 at analysis) is skipped — generated C then
/// keeps the inert-leaf shape for that state rather than emitting a
/// reference to a non-existent `Sub_t` (partial-IR principle, Doc 09 §1).
///
/// Only the parent's *own* root region is walked: a submachine ref nested
/// inside a composite/parallel is out of scope for the v1.1 example
/// (`is Sub` appears at the top level in `examples/submachine/`); a deeper
/// ref would simply not be collected here and retain the defensive leaf
/// behaviour rather than mis-emit. (Generalising to nested refs is a
/// follow-up; W2b's example exercises the top-level case the trace pins.)
pub fn collect_sub_refs<'a>(ctx: &MachineEmitCtx<'a>) -> Vec<SubRef<'a>> {
    let mut out = Vec::new();
    for s in &ctx.machine.root.states {
        let StateNode::Submachine(sref) = s else {
            continue;
        };
        let Some(template) = ctx
            .machine
            .submachines
            .iter()
            .find(|m| m.id == sref.submachine_id)
        else {
            // Unresolved template — leave the inert-leaf behaviour.
            continue;
        };
        let Some(parent_idx) = ctx.index.lookup(&sref.id) else {
            continue;
        };
        let parent_state_c = ctx.index.get(parent_idx).c_name.clone();
        let sub_type = template.name.clone();
        let sub_macro = c_ident(&template.name);
        let member = format!("_sub_{}", parent_state_c);
        out.push(SubRef {
            sref,
            parent_state_c,
            template,
            sub_type,
            sub_macro,
            member,
        });
    }
    out
}

/// True when `state_id` is a submachine ref-state whose template resolves —
/// i.e. it owns a nested sub-instance member. Used by the entry-sequence
/// emitters to decide whether to emit `Sub_init` instead of a user
/// `_entry_X` extern call (a ref-state has no user entry/exit action — the
/// `is Sub { … }` grammar carries only transitions; the sub-instance
/// lifecycle is codegen's job, exactly as W2c runs no ref-state entry
/// action and instead builds the sub-`RuntimeState`).
pub fn ref_state_member<'a>(ctx: &MachineEmitCtx<'a>, state_id: &str) -> Option<SubRef<'a>> {
    collect_sub_refs(ctx)
        .into_iter()
        .find(|sr| sr.sref.id == state_id)
}

/// Emit the `Sub_init(&m->_sub_X)` call that instantiates the sub-instance
/// fresh when the ref-state is entered (Doc 08 §12.2). Re-callable: a
/// ref-state self-transition re-enters and this resets the sub (mirrors
/// W2c's teardown→re-instantiate on `run_exit`).
pub fn emit_sub_init(sr: &SubRef<'_>, pad: &str, out: &mut String) {
    out.push_str(&format!(
        "{pad}/* Submachine entry (Doc 08 §12.2): instantiate `{name}`'s sub-instance\n\
         {pad} * at its implicit initial. Value member — heap-free (Doc 02 G2);\n\
         {pad} * a re-entry (ref-state self-transition) resets it fresh. */\n",
        pad = pad,
        name = sr.sub_type,
    ));
    out.push_str(&format!(
        "{pad}{sub}_init(&m->{member});\n",
        pad = pad,
        sub = sr.sub_type,
        member = sr.member,
    ));
}

/// Emit the delegation + completion block, run AFTER the parent's
/// transition walk so a parent-level ref-state transition wins
/// (transition-wins, mirroring W2c's `select_transitions` →
/// `try_delegate_to_submachine` order).
///
/// For each region whose active leaf is a submachine ref and which did NOT
/// fire a parent transition this RTC step (`!fired_in_region[r]`), the
/// event is re-resolved BY NAME into the sub's own event id space (W2c does
/// `event_id_by_name` against the sub's table) and routed into
/// `Sub_dispatch`. Then `Sub_completion_sweep` checks whether the sub
/// reached its `Final` and, if so, synthesises the parent
/// `EVENT__COMPLETION` so the ref-state's `done -> Target` fires through the
/// EXISTING completion machinery (not duplicated).
///
/// `fired_in_region_arr` / `region_idx_var` thread the switch strategy's
/// per-region fired flags; the table strategy passes its own equivalents.
/// `track_delegated`: emit the `__delegated_any = true;` write. Only the
/// defer hook reads `__delegated_any` (to avoid holding a delegated event),
/// so a machine with no `defer` must NOT emit the write — an unread
/// `bool __delegated_any` would trip `-Werror=unused-but-set-variable`
/// (the TD-BUG-1 family). When false the `fired_in_region[__sr]` write
/// alone already prevents re-processing the delegated event.
pub fn emit_delegation_block(
    ctx: &MachineEmitCtx<'_>,
    sub_refs: &[SubRef<'_>],
    pad: &str,
    track_delegated: bool,
    out: &mut String,
) {
    if sub_refs.is_empty() {
        return;
    }
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();

    out.push_str(&format!(
        "{pad}/* Submachine delegation (Doc 08 §12.1) — transition-wins: only the\n\
         {pad} * regions whose leaf is a submachine ref AND that did not fire a\n\
         {pad} * parent transition this step delegate the event into the sub. The\n\
         {pad} * parent event is re-resolved BY NAME into the sub's own event id\n\
         {pad} * space (the sub template has an independent id namespace —\n\
         {pad} * mirrors the merged W2c `event_id_by_name`). */\n",
        pad = pad,
    ));
    out.push_str(&format!(
        "{pad}for (int8_t __sr = (int8_t)initial_active - 1; __sr >= 0; __sr--) {{\n",
        pad = pad,
    ));
    out.push_str(&format!(
        "{pad}    if ((uint8_t)__sr >= m->_active_count) continue;\n",
        pad = pad,
    ));
    out.push_str(&format!(
        "{pad}    if (fired_in_region[__sr]) continue; /* parent transition won */\n",
        pad = pad,
    ));
    out.push_str(&format!(
        "{pad}    switch (m->_active[__sr]) {{\n",
        pad = pad,
    ));
    for sr in sub_refs {
        out.push_str(&format!(
            "{pad}    case {macro}_STATE_{st}: {{\n",
            pad = pad,
            macro = macro_prefix,
            st = sr.parent_state_c,
        ));
        out.push_str(&format!(
            "{pad}        {sub}_Event_t __sev;\n{pad}        bool __routed = false;\n",
            pad = pad,
            sub = sr.sub_type,
        ));
        out.push_str(&format!("{pad}        switch (ev->id) {{\n", pad = pad));
        // Map by NAME: a parent event whose name also names a sub event
        // delegates; others are not routed (the sub doesn't declare them —
        // W2c's `event_id_by_name` returns None and delegation is skipped).
        for pev in &ctx.machine.events {
            let Some(sev) = sr.template.events.iter().find(|e| e.name == pev.name) else {
                continue;
            };
            out.push_str(&format!(
                "{pad}        case {pmacro}_EVENT_{pn}: __sev.id = {smacro}_EVENT_{sn}; __routed = true; break;\n",
                pad = pad,
                pmacro = macro_prefix,
                pn = c_ident(&pev.name),
                smacro = sr.sub_macro,
                sn = c_ident(&sev.name),
            ));
        }
        out.push_str(&format!("{pad}        default: break;\n", pad = pad));
        out.push_str(&format!("{pad}        }}\n", pad = pad));
        let track = if track_delegated {
            format!("{pad}            __delegated_any = true;\n", pad = pad)
        } else {
            String::new()
        };
        out.push_str(&format!(
            "{pad}        if (__routed) {{\n\
             {pad}            {sub}_dispatch(&m->{member}, &__sev);\n\
             {pad}            fired_in_region[__sr] = true; /* consumed by the sub */\n\
             {track}{pad}        }}\n",
            pad = pad,
            sub = sr.sub_type,
            member = sr.member,
            track = track,
        ));
        out.push_str(&format!("{pad}        break;\n{pad}    }}\n", pad = pad));
    }
    out.push_str(&format!("{pad}    default: break;\n", pad = pad));
    out.push_str(&format!("{pad}    }}\n", pad = pad));
    out.push_str(&format!("{pad}}}\n", pad = pad));
    // The completion sweep is emitted separately via `emit_completion_sweep`
    // (the switch dispatch calls it immediately after this block — correct,
    // since the switch strategy executes transitions inline during the
    // parent walk, so by the time the sweep runs a `done` would already
    // have exited the ref-state).
    let _ = (prefix, macro_prefix);
}

/// Table-strategy variant of [`emit_delegation_block`]. The table dispatch
/// has no per-region `fired_in_region[]`; instead it records, per region
/// slot, whether `select_for_region` returned a row (`__region_selected[]`).
/// Transition-wins is preserved: delegation runs only for a ref-state
/// region with NO selected row — exactly the table analogue of W2c's
/// "parent selection empty ⇒ try delegate".
///
/// Emitted before the `selected_count == 0` early return so a delegated
/// event progresses the sub even when no parent row matched (the Device
/// example: `CONNECT`/`ACK`/`ESTABLISHED` match no `Connecting` row). The
/// completion sweep then synthesises the parent `EVENT__COMPLETION` so
/// `done -> Target` fires through the existing collect-then-execute path
/// (recursively re-entering `..._dispatch`, same as the switch strategy and
/// W2c's queued `Completion`).
pub fn emit_delegation_block_table(
    ctx: &MachineEmitCtx<'_>,
    sub_refs: &[SubRef<'_>],
    pad: &str,
    track_delegated: bool,
    out: &mut String,
) {
    if sub_refs.is_empty() {
        return;
    }
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();

    out.push_str(&format!(
        "{pad}/* v1.1-W2d submachine delegation (Doc 08 §12.1) — transition-wins:\n\
         {pad} * a ref-state region with NO selected parent row delegates the\n\
         {pad} * event into its sub (by-name id resolution — mirrors W2c\n\
         {pad} * `event_id_by_name`). Runs before the no-row early return so the\n\
         {pad} * sub still advances when no parent row matched. */\n",
        pad = pad,
    ));
    out.push_str(&format!(
        "{pad}for (int8_t __sr = (int8_t)initial_active - 1; __sr >= 0; __sr--) {{\n",
        pad = pad,
    ));
    out.push_str(&format!(
        "{pad}    if ((uint8_t)__sr >= m->_active_count) continue;\n",
        pad = pad,
    ));
    out.push_str(&format!(
        "{pad}    if (__region_selected[__sr]) continue; /* parent row won */\n",
        pad = pad,
    ));
    out.push_str(&format!(
        "{pad}    switch (m->_active[__sr]) {{\n",
        pad = pad
    ));
    for sr in sub_refs {
        out.push_str(&format!(
            "{pad}    case {macro}_STATE_{st}: {{\n",
            pad = pad,
            macro = macro_prefix,
            st = sr.parent_state_c,
        ));
        out.push_str(&format!(
            "{pad}        {sub}_Event_t __sev;\n{pad}        bool __routed = false;\n",
            pad = pad,
            sub = sr.sub_type,
        ));
        out.push_str(&format!("{pad}        switch (ev->id) {{\n", pad = pad));
        for pev in &ctx.machine.events {
            let Some(sev) = sr.template.events.iter().find(|e| e.name == pev.name) else {
                continue;
            };
            out.push_str(&format!(
                "{pad}        case {pmacro}_EVENT_{pn}: __sev.id = {smacro}_EVENT_{sn}; __routed = true; break;\n",
                pad = pad,
                pmacro = macro_prefix,
                pn = c_ident(&pev.name),
                smacro = sr.sub_macro,
                sn = c_ident(&sev.name),
            ));
        }
        out.push_str(&format!("{pad}        default: break;\n", pad = pad));
        out.push_str(&format!("{pad}        }}\n", pad = pad));
        // The table strategy ALWAYS reads `__delegated_any` (the no-row
        // early-return branch: `if (__delegated_any) return;`), so the
        // write is always emitted regardless of `defer` — no
        // unused-but-set risk. `track_delegated` is accepted for signature
        // symmetry with the switch variant.
        let _ = track_delegated;
        out.push_str(&format!(
            "{pad}        if (__routed) {{\n\
             {pad}            {sub}_dispatch(&m->{member}, &__sev);\n\
             {pad}            __delegated_any = true;\n\
             {pad}        }}\n",
            pad = pad,
            sub = sr.sub_type,
            member = sr.member,
        ));
        out.push_str(&format!("{pad}        break;\n{pad}    }}\n", pad = pad));
    }
    out.push_str(&format!("{pad}    default: break;\n", pad = pad));
    out.push_str(&format!("{pad}    }}\n", pad = pad));
    out.push_str(&format!("{pad}}}\n", pad = pad));
    // NOTE: the table strategy's completion sweep is emitted SEPARATELY,
    // AFTER the execute phase (see `emit_completion_sweep` +
    // `dispatch_table::emit_outer_dispatch`). Running it here (before the
    // selected `done` row executes) would re-enter `_dispatch` while
    // `Connecting` is still active and the sub still at `Final`, recursing
    // forever (the table strategy collects-then-executes; the row is not
    // applied until the execute phase). W2c orders `rtc_step` (selection +
    // execution) BEFORE `sync_submachines` (the completion sweep); the
    // table strategy matches that ordering by sweeping post-execute. (The
    // switch strategy executes transitions inline during the parent walk,
    // so its sweep is correctly co-located with delegation — see
    // `emit_delegation_block`.)
    let _ = (prefix, macro_prefix);
}

/// Emit the submachine completion sweep — a sub-instance whose active leaf
/// is its `Final` makes the parent receive a synthetic `EVENT__COMPLETION`,
/// firing the ref-state's `done -> Target` through the EXISTING completion
/// machinery (NOT duplicated — mirrors W2c enqueuing `Completion(ref_id)`
/// and letting the established R1 path fire it). Idempotent: the `done`
/// transition exits the ref-state, so `m->_active[__cs] == <ref>` is false
/// on any re-sweep — the synthetic completion is not re-raised (the
/// `_completion_depth` watchdog additionally bounds Doc 08 §9.4).
///
/// Strategy-agnostic. The **switch** strategy runs this right after the
/// parent walk (transitions already applied inline). The **table** strategy
/// runs this AFTER the execute phase (so the selected `done` row is applied
/// before any re-sweep — recursion-safe, matching W2c's `rtc_step` →
/// `sync_submachines` order).
pub fn emit_completion_sweep(
    ctx: &MachineEmitCtx<'_>,
    sub_refs: &[SubRef<'_>],
    pad: &str,
    out: &mut String,
) {
    if sub_refs.is_empty() {
        return;
    }
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    out.push_str(&format!(
        "{pad}/* Submachine completion sweep (Doc 08 §12.3): a sub-instance whose\n\
         {pad} * active leaf is its `Final` makes the parent receive a synthetic\n\
         {pad} * completion, firing the ref-state `done -> Target` through the SAME\n\
         {pad} * completion path every other completion uses (NOT duplicated —\n\
         {pad} * mirrors W2c enqueuing `Completion(ref_state)`; idempotent — the\n\
         {pad} * `done` exits the ref-state so this does not re-fire). */\n",
        pad = pad,
    ));
    for sr in sub_refs {
        let Some(final_c) = sr.final_state_c() else {
            continue;
        };
        out.push_str(&format!(
            "{pad}for (uint8_t __cs = 0; __cs < m->_active_count; __cs++) {{\n\
             {pad}    if (m->_active[__cs] == {macro}_STATE_{st} &&\n\
             {pad}        {sub}_current_state(&m->{member}) == {smacro}_STATE_{fin}) {{\n\
             {pad}        {prefix}_Event_t __comp;\n\
             {pad}        __comp.id = {macro}_EVENT__COMPLETION;\n\
             {pad}        {prefix}_dispatch(m, &__comp);\n\
             {pad}    }}\n\
             {pad}}}\n",
            pad = pad,
            macro = macro_prefix,
            st = sr.parent_state_c,
            sub = sr.sub_type,
            member = sr.member,
            smacro = sr.sub_macro,
            fin = final_c,
            prefix = prefix,
        ));
    }
}

/// Pre-execute check (table strategy): set `__sub_pending` true if any
/// active ref-state's sub reached its `Final`. The table dispatch uses this
/// so a sub-completion is processed (sweep + `done`) even when
/// `selected_count == 0` — without it the no-row early return would discard
/// the synthetic completion and the parent would never advance. Mirrors
/// W2c, where `sync_submachines` enqueues `Completion(ref_id)`
/// unconditionally after the step regardless of whether the step fired a
/// parent transition.
pub fn emit_sub_pending_check(
    ctx: &MachineEmitCtx<'_>,
    sub_refs: &[SubRef<'_>],
    pad: &str,
    out: &mut String,
) {
    if sub_refs.is_empty() {
        return;
    }
    let macro_prefix = ctx.macro_prefix();
    for sr in sub_refs {
        let Some(final_c) = sr.final_state_c() else {
            continue;
        };
        out.push_str(&format!(
            "{pad}for (uint8_t __pc = 0; __pc < m->_active_count; __pc++) {{\n\
             {pad}    if (m->_active[__pc] == {macro}_STATE_{st} &&\n\
             {pad}        {sub}_current_state(&m->{member}) == {smacro}_STATE_{fin}) {{\n\
             {pad}        __sub_pending = true;\n\
             {pad}    }}\n\
             {pad}}}\n",
            pad = pad,
            macro = macro_prefix,
            st = sr.parent_state_c,
            sub = sr.sub_type,
            member = sr.member,
            smacro = sr.sub_macro,
            fin = final_c,
        ));
    }
}

/// Whether a `StateNode::Submachine` ref-state's own transitions
/// (`SubmachineRef.transitions` — parent-level `on EVT` / `done ->`) should
/// be emitted as a normal per-state dispatch case. Always true: these are
/// the parent's transitions hung on the ref-state (Doc 09 §4.11) and must
/// dispatch like any other state's, which is what makes transition-wins
/// observable and what fires `done -> Target` on the synthetic completion.
pub fn submachine_has_dispatch_transitions(sref: &fsm_ir::SubmachineRef) -> bool {
    !sref.transitions.is_empty()
}

/// The `done -> Target` (completion-kind) transitions a submachine ref-state
/// carries. Surfaced for documentation/symmetry; the dispatch emitters emit
/// every transition uniformly so this is informational only.
pub fn ref_done_transitions(sref: &fsm_ir::SubmachineRef) -> Vec<&fsm_ir::TransitionObject> {
    sref.transitions
        .iter()
        .filter(|t| t.kind == TransitionKind::Completion)
        .collect()
}

/// Recursively search a region's states for a `final NAME`, returning its
/// DSL name. The v1.1 submachine template (`examples/submachine/`) has a
/// single top-level `final`; a nested final is found too for robustness.
fn find_final_name(states: &[StateNode]) -> Option<String> {
    for s in states {
        match s {
            StateNode::Final(f) => return Some(f.name.clone()),
            StateNode::Composite(c) => {
                for r in &c.regions {
                    if let Some(n) = find_final_name(&r.states) {
                        return Some(n);
                    }
                }
            }
            StateNode::Parallel(p) => {
                for r in &p.regions {
                    if let Some(n) = find_final_name(&r.states) {
                        return Some(n);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Whether `rec` is a submachine ref-state record (used to suppress the
/// user `_entry_X`/`_exit_X` extern contract — codegen owns the
/// sub-instance lifecycle, the user implements nothing for a ref-state).
pub fn is_submachine_record(kind: StateRecordKind) -> bool {
    kind == StateRecordKind::Submachine
}
