//! Table-driven dispatch — Doc 11 §9, Doc 00 §7.8 (B-11).
//!
//! Two-phase collect-then-execute:
//!
//! 1. For each active region (`m->_active[r]`), walk leaf-to-root via the
//!    shared parent table; for each ancestor scan `Motor_trans_table[]`
//!    for the first matching `(source, trigger, guard)` row. Record at
//!    most one row per region in a `selected[]` buffer.
//! 2. Execute every selected transition (one per region).
//!
//! Per-row execution sequences exit-set → action → entry-set → slot
//! update, identical to the switch strategy. Row identity is preserved by
//! a parallel `IR_ROWS` static that stores the original transition's
//! ordering index so we can dispatch into a per-row inline body via a
//! switch on `row_idx`. No fragile `(source, trigger, priority)` matcher.

use fsm_ir::{walk_state, IrVisitor, StateNode, TransitionKind, TransitionObject};

use super::MachineEmitCtx;

/// Emit the transition table, executor, and outer dispatch.
pub fn emit_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let mut s = String::new();
    s.push_str(&emit_parent_table_alias(ctx));
    s.push_str("\n");
    // v1.1: emit the deferred-event apparatus immediately after the parent
    // table alias it queries (and before the outer dispatch that calls it).
    // Gated on `machine_has_defer` so non-defer machines are byte-identical
    // to the pre-wave output.
    if super::defer::machine_has_defer(ctx) {
        s.push_str(&super::defer::emit_defer_table(ctx));
        s.push_str("\n");
        s.push_str(&super::defer::emit_defer_runtime(ctx));
        s.push_str("\n");
    }
    s.push_str(&emit_guard_action_typedefs(ctx));
    let rows = collect_transitions(ctx);
    s.push_str(&emit_trans_table(ctx, &rows));
    s.push_str("\n");
    s.push_str(&emit_execute_transition(ctx, &rows));
    s.push_str("\n");
    s.push_str(&emit_row_guard_enabled(ctx, &rows));
    s.push_str("\n");
    s.push_str(&emit_collect_phase(ctx));
    s.push_str("\n");
    s.push_str(&emit_outer_dispatch(ctx));
    s
}

fn emit_parent_table_alias(ctx: &MachineEmitCtx<'_>) -> String {
    // The table strategy reuses the same parent_table shape as switch.
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "/* B-11 table dispatch: shares the parent-pointer table with the switch strategy. */\n",
    ));
    s.push_str(&format!(
        "static const {prefix}_StateId_t {prefix}_parent_table[{macro}_STATE__COUNT] = {{\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    for (i, rec) in ctx.index.records.iter().enumerate() {
        let parent_rec = ctx.index.get(rec.parent);
        s.push_str(&format!(
            "    [{}] = {macro}_STATE_{name},\n",
            i,
            macro = macro_prefix,
            name = parent_rec.c_name,
        ));
    }
    s.push_str("};\n");
    s
}

fn emit_guard_action_typedefs(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    format!(
        "typedef bool (*{prefix}_GuardFn_t)(const {prefix}_t *, const {prefix}_Event_t *);\n\
         typedef void (*{prefix}_ActionFn_t)({prefix}_t *, const {prefix}_Event_t *);\n",
        prefix = prefix,
    )
}

/// One emitted row in `Motor_trans_table[]`. `row_idx` is the deterministic
/// index used by both the table runtime AND the per-row executor switch.
#[derive(Clone)]
struct EmittedTransRow {
    row_idx: usize,
    source: u8,
    target: u8,
    trigger_c: String,
    priority: u16,
    kind_tag: u8,
    /// Reference into the IR — used to look up guard/action expressions.
    /// Carried by index (machine-doc-order across all transitions).
    ir_ref: TransitionRef,
}

#[derive(Clone)]
struct TransitionRef {
    /// IR transition. Cloned here to avoid threading lifetimes through the
    /// emit_outer_dispatch switch — codegen is one-shot, the clone is cheap.
    transition: TransitionObject,
}

/// Visitor that flattens every (Simple | Composite | Parallel) state's
/// transitions into emitted-row form. R2.1 (2026-05-15): traversal moves
/// to `IrVisitor::walk_state` so the recursion shape lives in `fsm-ir` only.
struct TransRowCollector<'a, 'm> {
    ctx: &'a MachineEmitCtx<'m>,
    out: &'a mut Vec<EmittedTransRow>,
}

impl<'a, 'm> IrVisitor for TransRowCollector<'a, 'm> {
    fn visit_state(&mut self, s: &StateNode) {
        let transitions = match s {
            StateNode::Simple(ss) => &ss.transitions[..],
            StateNode::Composite(c) => &c.transitions[..],
            StateNode::Parallel(p) => &p.transitions[..],
            // v1.1-W2d: emit the submachine ref-state's own transitions
            // (`on EVT` / `done -> Target`, Doc 09 §4.11) as table rows so
            // the table strategy fires them identically to the switch
            // strategy — same transition-wins + `done` semantics.
            StateNode::Submachine(sm) => &sm.transitions[..],
            _ => {
                // Pseudo-states host no transitions; still descend so any
                // nested regions (none today, but future-proof) are walked.
                walk_state(self, s);
                return;
            }
        };
        for t in transitions {
            let source = self.ctx.index.must_lookup(&t.source);
            let target = self.ctx.index.must_lookup(&t.target);
            // P0-4: per-timer event variant resolution so the trans table
            // row matches the timer's own event id, not generic completion.
            let trigger_c = match &t.trigger {
                Some(fsm_ir::Trigger::Event { event_id, .. }) => self.ctx.event_c_enum(event_id),
                Some(fsm_ir::Trigger::After { timer_id, .. })
                | Some(fsm_ir::Trigger::Every { timer_id, .. }) => {
                    super::timer::timer_event_c(self.ctx, timer_id)
                        .unwrap_or_else(|| format!("{}_EVENT__COMPLETION", self.ctx.macro_prefix()))
                }
                _ => format!("{}_EVENT__COMPLETION", self.ctx.macro_prefix()),
            };
            let kind_tag = match t.kind {
                TransitionKind::External => 0,
                TransitionKind::Local => 1,
                TransitionKind::Internal => 2,
                TransitionKind::Completion => 3,
            };
            self.out.push(EmittedTransRow {
                row_idx: 0, // assigned after sorting below
                source,
                target,
                trigger_c,
                priority: t.priority,
                kind_tag,
                ir_ref: TransitionRef {
                    transition: t.clone(),
                },
            });
        }
        walk_state(self, s);
    }
}

fn collect_transitions(ctx: &MachineEmitCtx<'_>) -> Vec<EmittedTransRow> {
    let mut out = Vec::new();
    let mut collector = TransRowCollector { ctx, out: &mut out };
    // Walk the root region only — submachines (Doc 09 §4.11) emit their
    // own trans tables in their own emit pass.
    collector.visit_region(&ctx.machine.root);
    // Sort by (source, priority, document order). After sort, assign
    // stable row indices.
    out.sort_by(|a, b| a.source.cmp(&b.source).then(a.priority.cmp(&b.priority)));
    for (i, row) in out.iter_mut().enumerate() {
        row.row_idx = i;
    }
    out
}

fn emit_trans_table(ctx: &MachineEmitCtx<'_>, rows: &[EmittedTransRow]) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();

    let mut s = String::new();
    s.push_str(&format!(
        "typedef struct {{\n    {prefix}_StateId_t source;\n    {prefix}_EventId_t trigger;\n    {prefix}_StateId_t target;\n    uint16_t priority;\n    uint8_t kind;\n    uint16_t row_idx;\n}} {prefix}_TransRow_t;\n",
        prefix = prefix,
    ));
    s.push_str(&format!(
        "static const {prefix}_TransRow_t {prefix}_trans_table[] = {{\n",
        prefix = prefix,
    ));
    if rows.is_empty() {
        // TD-BUG-1: a valid zero-transition machine would otherwise emit
        // `static const T arr[] = {};` — an empty initializer + zero-size
        // array, both rejected by `gcc -std=c99 -Wpedantic -Werror` ("ISO
        // C forbids empty initializer braces", "zero or negative size
        // array"), and the `sizeof/sizeof`-derived TABLE_SIZE would make
        // the `select_for_region` loop bound `unsigned < 0`
        // (`-Werror=type-limits`). Emit one all-zero sentinel row whose
        // `source` is the out-of-range `_COUNT` state id. The array is then
        // a well-formed non-empty C99 array (TABLE_SIZE == 1, no
        // type-limits), and because no active leaf is ever `>= _COUNT` the
        // `row->source != s` test always continues — the sentinel can never
        // be selected, so dispatch behaviour is identical to "no table".
        s.push_str(&format!(
            "    /* TD-BUG-1 sentinel: unmatchable (source == _COUNT). */\n    {{ {macro}_STATE__COUNT, 0, {macro}_STATE__COUNT, 0, 0, 0 }},\n",
            macro = macro_prefix,
        ));
    }
    for row in rows {
        let source_c = ctx.index.get(row.source).c_name.clone();
        let target_c = ctx.index.get(row.target).c_name.clone();
        let trigger_c = row.trigger_c.clone();
        s.push_str(&format!(
            "    {{ {macro}_STATE_{src}, {trig}, {macro}_STATE_{tgt}, {pri}, {kind}, {ri} }},\n",
            macro = macro_prefix,
            src = source_c,
            trig = trigger_c,
            tgt = target_c,
            pri = row.priority,
            kind = row.kind_tag,
            ri = row.row_idx,
        ));
    }
    s.push_str("};\n");
    s.push_str(&format!(
        "#define {macro}_TRANS_TABLE_SIZE (sizeof({prefix}_trans_table) / sizeof({prefix}_trans_table[0]))\n",
        macro = macro_prefix,
        prefix = prefix,
    ));
    s
}

/// Emit the per-row executor — exit/action/entry/slot-update keyed by
/// `row_idx`. Per Doc 00 §7.8 we no longer recover the IR transition by
/// (source, trigger, priority) matching; `row_idx` is the stable handle.
fn emit_execute_transition(ctx: &MachineEmitCtx<'_>, rows: &[EmittedTransRow]) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "static void {prefix}_execute_transition({prefix}_t *m, const {prefix}_TransRow_t *row, const {prefix}_Event_t *ev) {{\n",
        prefix = prefix,
    ));
    s.push_str("    (void)ev;\n");
    // TD-BUG-1: with zero transitions the switch below has only `default:
    // break;`, so `m` is never read (a transition body is the only thing
    // that touches `m`). `row` IS still used (the switch discriminant
    // `row->row_idx`), so it is deliberately NOT cast. When rows exist a
    // transition body references `m`, so the cast must be conditional.
    if rows.is_empty() {
        s.push_str("    (void)m;\n");
    }
    s.push_str("    switch (row->row_idx) {\n");
    for row in rows {
        s.push_str(&format!("    case {}: {{\n", row.row_idx));
        let payload_prefix = payload_prefix_for(ctx, &row.ir_ref.transition);
        super::transition::emit_transition_body(
            &row.ir_ref.transition,
            ctx,
            &payload_prefix,
            /*indent_spaces=*/ 8,
            /*on_guard_fail=*/ "return;",
            &mut s,
        );
        s.push_str("        return;\n");
        s.push_str("    }\n");
    }
    s.push_str("    default: break;\n");
    s.push_str("    }\n");
    s.push_str("}\n");
    s
}

/// W7-FU-1: emit `<M>_row_guard_enabled(row_idx, m, ev)` — returns whether
/// the transition behind `row_idx` is *enabled* (its guard evaluates true,
/// or it is unguarded / `[else]`). The pre-fix `select_for_region` returned
/// the FIRST `(source,trigger)` row IGNORING the guard, then the executor
/// re-checked the guard and silently no-op'd if it was false — so a state
/// with `on E [g1] -> A` / `on E [g2] -> B` dropped E whenever g1 was false
/// instead of taking the g2 transition (the P0-1 silent-data-loss class).
///
/// The guard expression addresses the event-specific payload union member
/// (`ev->__payload.<EventName>`), which differs per row, so the guard MUST
/// be emitted per-row (keyed by the deterministic `row_idx`) rather than as
/// one generic expression in the scan loop. Doc 08 §4.3: a guard is
/// evaluated exactly once per candidate at selection time — this function IS
/// that single selection-time evaluation; the executor no longer re-checks
/// (its `on_guard_fail` path is now dead for table-selected rows, but kept
/// harmless and identical to the switch strategy's shared body).
fn emit_row_guard_enabled(ctx: &MachineEmitCtx<'_>, rows: &[EmittedTransRow]) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "/* W7-FU-1: guard-aware selection. true => row_idx's transition is\n\
         \x20* enabled (guard holds, or unguarded/[else]); first enabled row in\n\
         \x20* (source, priority, document-order) wins (Doc 08 §4.1/§4.2). */\n",
    ));
    s.push_str(&format!(
        "static bool {prefix}_row_guard_enabled(uint16_t row_idx, const {prefix}_t *m, const {prefix}_Event_t *ev) {{\n",
        prefix = prefix,
    ));
    // A machine whose every same-(source,event) group is single-row (or all
    // unguarded) still benefits from a uniform helper; cast unused params
    // when NO row carries a guard so a guard-free machine stays
    // `-Werror=unused-parameter` clean (TD-BUG-1 discipline).
    let any_guard = rows.iter().any(|r| r.ir_ref.transition.guard.is_some());
    if !any_guard {
        s.push_str("    (void)row_idx; (void)m; (void)ev;\n");
        s.push_str("    return true; /* no guarded transition in this machine */\n");
        s.push_str("}\n");
        return s;
    }
    // Build the case arms first so we can emit precise `(void)` casts for
    // only the genuinely-unused params (an extern guard like `[is_ready()]`
    // references neither `m` nor `ev`; a `ctx.x` guard uses `m` but not
    // `ev`; only a `payload.x` guard uses `ev`). Blanket-casting a param
    // that a guard then dereferences is misleading and some toolchains
    // flag `(void)x;` immediately followed by a use — keep it exact
    // (TD-BUG-1 discipline, zero-legacy).
    let mut arms = String::new();
    for row in rows {
        let t = &row.ir_ref.transition;
        match &t.guard {
            None => {
                // Unguarded: always enabled. Emitting an explicit `return
                // true` arm (vs folding into default) keeps the mapping
                // row_idx -> semantics 1:1 and self-documenting.
                arms.push_str(&format!(
                    "    case {}: return true; /* unguarded */\n",
                    row.row_idx
                ));
            }
            Some(g) => {
                let payload_prefix = payload_prefix_for(ctx, t);
                let cond = crate::expr::emit_guard(g, "m->context", &payload_prefix);
                // Doc 08 §4.3: guards are side-effect-free, so a plain
                // evaluation here (selection time) is the single mandated
                // evaluation. The `likely`/`rare` hint is a pure
                // instruction-layout concern (semantically inert, sim
                // ignores it) and selection correctness must not depend on
                // it, so the SELECTION test is the bare condition; the hot/
                // cold layout hint stays where execution happens (the shared
                // `emit_transition_body`). This keeps sim==codegen.
                arms.push_str(&format!(
                    "    case {ri}: return {cond};\n",
                    ri = row.row_idx,
                    cond = cond,
                ));
            }
        }
    }
    // `row_idx` is always used (the switch discriminant). `m`/`ev` are used
    // iff some arm's emitted condition dereferences them.
    if !arms.contains("m->") {
        s.push_str("    (void)m;\n");
    }
    if !arms.contains("ev->") {
        s.push_str("    (void)ev;\n");
    }
    s.push_str("    switch (row_idx) {\n");
    s.push_str(&arms);
    s.push_str("    default: return true;\n");
    s.push_str("    }\n");
    s.push_str("}\n");
    s
}

/// Emit the per-region collect helper. Walks leaf-to-root via parent
/// table; for each ancestor scans the table for the first matching
/// (source, trigger) row WHOSE GUARD IS ENABLED (W7-FU-1).
fn emit_collect_phase(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "static const {prefix}_TransRow_t *{prefix}_select_for_region({prefix}_t *m, {prefix}_StateId_t s, const {prefix}_Event_t *ev) {{\n",
        prefix = prefix,
    ));
    // W7-FU-1: `m`/`ev` are now genuinely used — passed to
    // `<M>_row_guard_enabled` for selection-time guard evaluation. The
    // pre-fix `(void)m;` (selection ignored the guard) is removed; a
    // guard-free machine's unused-param concern is handled inside
    // `_row_guard_enabled` itself (it casts them there).
    s.push_str(&format!(
        "    while (1) {{\n        for (uint16_t i = 0; i < {macro}_TRANS_TABLE_SIZE; i++) {{\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "            const {prefix}_TransRow_t *row = &{prefix}_trans_table[i];\n",
        prefix = prefix,
    ));
    s.push_str("            if (row->source != s) continue;\n");
    s.push_str("            if (row->trigger != ev->id) continue;\n");
    // W7-FU-1: honour the guard AT SELECTION TIME. The table is sorted by
    // (source, priority, row_idx) = (source, priority, document-order), so
    // scanning in array order and returning the FIRST row whose guard is
    // enabled implements Doc 08 §4.1's `min(candidates, key=(priority,
    // document_order))` for this (source,event) group. A disabled guard
    // `continue`s to the next candidate row (e.g. the `[else]`/unguarded
    // fallback, or a lower-priority alternative) instead of the pre-fix
    // behaviour of returning row-1 and letting the executor silently drop
    // the event. This is exactly the simulator's `select_transitions`
    // (guard filtered into `candidates` before the priority/doc-order
    // pick) — sim is the oracle, both strategies now match it.
    s.push_str(&format!(
        "            if (!{prefix}_row_guard_enabled(row->row_idx, m, ev)) continue;\n",
        prefix = prefix,
    ));
    s.push_str("            return row;\n");
    s.push_str("        }\n");
    s.push_str(&format!(
        "        if (s == {macro}_STATE_ROOT) return NULL;\n",
        macro = macro_prefix,
    ));
    s.push_str(&format!(
        "        s = {prefix}_parent_table[s];\n",
        prefix = prefix,
    ));
    s.push_str("    }\n");
    s.push_str("}\n");
    s
}

fn emit_outer_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let has_defer = super::defer::machine_has_defer(ctx);
    let sub_refs = super::submachine::collect_sub_refs(ctx);

    // v1.1-W2d: table-strategy submachine delegation + completion sweep.
    // Emitted AFTER the collect loop (so a selected parent row wins —
    // transition-wins) and BEFORE the `selected_count == 0` early return
    // (so a delegated event still advances the sub when no parent row
    // matched). Mirrors W2c's `select_transitions` → `try_delegate` order.
    // Event-routing block (delegate the event into the sub for any
    // ref-state region with no selected parent row — transition-wins). The
    // completion sweep is NOT here: it must run AFTER the execute phase so
    // the selected `done` row is applied before any re-sweep (otherwise the
    // recursive synthetic-completion dispatch loops forever — the table
    // strategy collects-then-executes). This matches W2c's `rtc_step`
    // (selection+execution) → `sync_submachines` (completion) order.
    let delegation = if sub_refs.is_empty() {
        String::new()
    } else {
        let mut d = String::new();
        super::submachine::emit_delegation_block_table(ctx, &sub_refs, "    ", true, &mut d);
        d
    };
    let sub_pending_check = if sub_refs.is_empty() {
        String::new()
    } else {
        let mut p = String::new();
        super::submachine::emit_sub_pending_check(ctx, &sub_refs, "    ", &mut p);
        p
    };
    let completion_sweep = if sub_refs.is_empty() {
        String::new()
    } else {
        let mut c = String::new();
        super::submachine::emit_completion_sweep(ctx, &sub_refs, "    ", &mut c);
        c
    };

    // v1.1 deferral hook A — unconsumed-but-deferred event is HELD, not
    // discarded. `selected_count == 0` is the table strategy's "no
    // transition fired" condition, exactly where the leaf-to-root search
    // failed for every region, so checking here preserves transition-wins
    // (UML 2.5.1 §14.2.3.9.1 / Doc 08 §10.1).
    let defer_hold = if has_defer {
        format!(
            "        if ({prefix}_active_config_defers(m, ev->id)) {{\n\
             \x20           {prefix}_defer_push(m, ev);\n\
             \x20       }}\n",
            prefix = prefix,
        )
    } else {
        String::new()
    };

    // v1.1 deferral hook B — same ordering as the switch strategy and the
    // simulator's `run_step`: release deferred events to the queue front
    // (Doc 08 §10.2), run completion synchronously (the simulator
    // push_front's completion *after* the deferred prepend, so completion
    // is processed before the released events), then drain the released
    // events through the same dispatch machinery (Doc 08 §10.3).
    let release_call = if has_defer {
        format!("    {prefix}_release_deferred(m);\n", prefix = prefix)
    } else {
        String::new()
    };
    let drain_released = if has_defer {
        format!(
            "    {{\n\
             \x20       {prefix}_Event_t __rd;\n\
             \x20       while ({prefix}_dequeue(m, &__rd)) {{\n\
             \x20           {prefix}_dispatch(m, &__rd);\n\
             \x20       }}\n\
             \x20   }}\n",
            prefix = prefix,
        )
    } else {
        String::new()
    };

    // Submachine-bearing machines need to record, per region, whether a
    // parent row was selected (so delegation only runs for ref-state
    // regions with no row — transition-wins). Non-submachine machines emit
    // the byte-identical pre-W2d body (no `__region_selected[]`, no
    // `__delegated_any`, no delegation block).
    let has_subs = !sub_refs.is_empty();
    let region_sel_decl = if has_subs {
        format!(
            "    bool __region_selected[{macro}_MAX_PARALLEL_REGIONS] = {{ false }};\n\
             \x20   /* v1.1-W2d: __delegated_any — a delegated-and-consumed event\n\
             \x20    * must not also be discarded/deferred (W2c treats it as\n\
             \x20    * consumed). __sub_pending — a sub reached its Final, so the\n\
             \x20    * parent must process the synthetic completion even when no\n\
             \x20    * parent row was selected (mirrors W2c's unconditional\n\
             \x20    * post-step `sync_submachines`). */\n\
             \x20   bool __delegated_any = false;\n\
             \x20   bool __sub_pending = false;\n",
            macro = macro_prefix,
        )
    } else {
        String::new()
    };
    // The active-config collect loop — `select_transitions`'s per-region
    // innermost-first walk for NON-`TimerFire` events. Emitted verbatim
    // (byte-identical to pre-FW110-FU-E) as the `default:` arm of the
    // timer-fire dispatch switch (and, for a timerless machine, as the
    // ENTIRE collect phase — `emit_timer_fire_selection` returns `None`).
    let region_collect = if has_subs {
        format!(
            "    for (int8_t r = (int8_t)initial_active - 1; r >= 0; r--) {{\n\
             \x20       const {prefix}_TransRow_t *row = {prefix}_select_for_region(m, m->_active[r], ev);\n\
             \x20       if (row) {{ selected[selected_count++] = row; __region_selected[r] = true; }}\n\
             \x20   }}\n",
            prefix = prefix,
        )
    } else {
        format!(
            "    for (int8_t r = (int8_t)initial_active - 1; r >= 0; r--) {{\n\
             \x20       const {prefix}_TransRow_t *row = {prefix}_select_for_region(m, m->_active[r], ev);\n\
             \x20       if (row) selected[selected_count++] = row;\n\
             \x20   }}\n",
            prefix = prefix,
        )
    };

    // FW110-FU-E: a timer-fire event selects its transition from the
    // timer's STATIC (owner-state, transition) binding —
    // `fsm_simulator::select_transitions`'s `EventKind::TimerFire`
    // early-return — by running the table collect keyed at the owner state
    // (`select_for_region` from the owner finds the timer's row on its
    // first iteration; ancestor scan never matches the per-timer-unique
    // event id, so this is the precise analogue of the simulator's single
    // `node(source_state)` lookup). When the owner is active this is
    // byte-identical to the region collect (same row, same
    // `execute_transition`); when a same-instant transition already exited
    // the owner the row is still selected and executed with its per-kind
    // semantics (Internal ⇒ actions only) — the unforked oracle's record.
    // A timer-fire selection is at most one transition (no parallel
    // fan-out), exactly like the simulator's TimerFire arm; it is NOT
    // delegated to a submachine (the simulator's TimerFire arm returns
    // before `try_delegate_to_submachine`), so `__region_selected[]` is
    // intentionally not set on the timer branch. Non-timer events fall to
    // the `default:` region collect unchanged. GATED on `timers_can_co_arm`
    // (same gate as FW110-FU-D's `emit_advance_clock`): a machine with no
    // timers OR where no state owns ≥2 timers emits `region_collect`
    // verbatim (no switch wrapper) ⇒ byte-identical C — the owner is
    // provably always active when its timer fires there (no-regression
    // guardrail).
    let collect_loop = super::timer::emit_timer_fire_selection(
        ctx,
        &|_ctx, owner_macro| {
            format!(
                "        {{ const {prefix}_TransRow_t *row = {prefix}_select_for_region(m, {owner}, ev);\n\
                 \x20         if (row) selected[selected_count++] = row; }}\n",
                prefix = prefix,
                owner = owner_macro,
            )
        },
        &region_collect,
        "    ",
    )
    .unwrap_or(region_collect);
    // The no-row branch: a delegated-and-consumed event must NOT fall into
    // the defer/discard path (W2c treats it as consumed). With submachines,
    // skip the early return when the sub consumed the event; the completion
    // it may have triggered already ran inside the delegation block.
    let no_row_branch = if has_subs {
        // Discard/defer ONLY when nothing was consumed AND no sub completed.
        // A delegated-and-consumed event (W2c: consumed, not deferred) or a
        // pending sub-completion (W2c: `sync_submachines` enqueues
        // `Completion(ref_id)` post-step regardless) must fall through to
        // the (no-op) execute loop + the post-execute completion sweep.
        format!(
            "    if (selected_count == 0 && !__delegated_any && !__sub_pending) {{\n\
             {defer_hold}        return; /* No row, nothing delegated, no sub completed. */\n\
             \x20   }}\n",
            defer_hold = defer_hold,
        )
    } else {
        format!(
            "    if (selected_count == 0) {{\n\
             {defer_hold}        return; /* No region had an enabled transition. */\n\
             \x20   }}\n",
            defer_hold = defer_hold,
        )
    };

    format!(
        r#"
void {prefix}_dispatch({prefix}_t *m, const {prefix}_Event_t *ev) {{
    /* B-11 collect-then-execute. One transition per region maximum.
     *
     * v1.1 (2026-05-15): deferred-event runtime. The audit P0-5 option-b
     * stopgap is retired; `defer` is a real UML 2.5.1 §14.2.3.9.1 feature.
     * Hook A holds an unconsumed-but-deferred event; hook B releases held
     * events to the queue front on a config-changing transition and drains
     * them. Both gated on `machine_has_defer` so non-defer machines emit
     * the exact code they did before this wave. */

    const {prefix}_TransRow_t *selected[{macro}_MAX_PARALLEL_REGIONS];
    uint8_t selected_count = 0;
{region_sel_decl}    /* Iterate innermost-first (Doc 08 §4.1). */
    uint8_t initial_active = m->_active_count;
{collect_loop}{delegation}{sub_pending_check}{no_row_branch}
    /* Execute phase: at most one transition per region. */
    for (uint8_t i = 0; i < selected_count; i++) {{
        {prefix}_execute_transition(m, selected[i], ev);
    }}
{completion_sweep}{release_call}    {prefix}_handle_completion(m);
{drain_released}}}
"#,
        prefix = prefix,
        macro = macro_prefix,
        region_sel_decl = region_sel_decl,
        collect_loop = collect_loop,
        delegation = delegation,
        sub_pending_check = sub_pending_check,
        no_row_branch = no_row_branch,
        completion_sweep = completion_sweep,
        release_call = release_call,
        drain_released = drain_released,
    )
}

fn payload_prefix_for(ctx: &MachineEmitCtx<'_>, t: &TransitionObject) -> String {
    let trigger_id = match &t.trigger {
        Some(fsm_ir::Trigger::Event { event_id, .. }) => event_id.clone(),
        _ => return "ev->__payload".to_string(),
    };
    let event = ctx
        .machine
        .events
        .iter()
        .find(|e| e.id == trigger_id || e.stable_id == trigger_id);
    match event {
        Some(ev) if !ev.payload.is_empty() => format!("ev->__payload.{}", ev.name),
        _ => "ev->__payload".to_string(),
    }
}
