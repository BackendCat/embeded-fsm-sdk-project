//! Switch-based dispatch — Doc 11 §8, Doc 00 §7.8 (B-10 + B-11).
//!
//! Pattern:
//!  - Emit `Motor_parent_table[]: static const M_StateId_t[]`.
//!  - Emit per-state helper `Motor_try_transitions_in_state` returning
//!    `bool` (true if a transition fired).
//!  - Emit the outer `Motor_dispatch` that, per active region, walks
//!    leaf-to-root using the parent table (collect-then-execute over
//!    `_active[]`).

use fsm_ir::{walk_state, IrVisitor, StateNode, TransitionObject};

use super::MachineEmitCtx;

/// Emit the parent table + per-state helpers + outer dispatch loop.
pub fn emit_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let mut s = String::new();
    s.push_str(&emit_parent_table(ctx));
    s.push_str("\n");
    s.push_str(&emit_per_state_helpers(ctx));
    s.push_str("\n");
    s.push_str(&emit_outer_dispatch(ctx));
    s
}

fn emit_parent_table(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "/* B-10 hierarchical dispatch: parent-pointer table indexed by StateId. */\n",
    ));
    s.push_str(&format!(
        "static const {prefix}_StateId_t {prefix}_parent_table[{macro}_STATE__COUNT] = {{\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    for (i, rec) in ctx.index.records.iter().enumerate() {
        let parent_rec = ctx.index.get(rec.parent);
        s.push_str(&format!(
            "    [{}] = {macro}_STATE_{name}, /* {} */\n",
            i,
            rec.dsl_name,
            macro = macro_prefix,
            name = parent_rec.c_name,
        ));
    }
    s.push_str("};\n");
    s
}

fn emit_per_state_helpers(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "/* Per-state transition try function. Returns true if a transition fired. */\n",
    ));
    s.push_str(&format!(
        "static bool {prefix}_try_transitions_in_state({prefix}_t *m, {prefix}_StateId_t s, const {prefix}_Event_t *ev) {{\n",
        prefix = prefix,
    ));
    s.push_str("    switch (s) {\n");

    // R2.1 (2026-05-15): traversal delegated to `IrVisitor::walk_state` so
    // adding a new pseudo-state variant only requires editing
    // `fsm-ir/src/visitor.rs`. Per-variant emit logic stays here. We start
    // at the root region (not the machine) because the switch dispatch
    // emits cases for *this* machine only; submachines (Doc 09 §4.11)
    // emit their own switch.
    let mut emitter = SwitchEmitter { ctx, out: &mut s };
    emitter.visit_region(&ctx.machine.root);
    s.push_str("    default: break;\n");
    s.push_str("    }\n");
    s.push_str("    return false;\n");
    s.push_str("}\n");
    s
}

/// Per-state-case emitter for the switch-strategy dispatch. Implemented as
/// an `IrVisitor` so traversal lives in `fsm-ir` (R2.1). The traversal-only
/// concern (visit every state recursively) is the trait default; this struct
/// supplies the per-state emit body.
struct SwitchEmitter<'a, 'm> {
    ctx: &'a MachineEmitCtx<'m>,
    out: &'a mut String,
}

impl<'a, 'm> IrVisitor for SwitchEmitter<'a, 'm> {
    fn visit_state(&mut self, s: &StateNode) {
        let (id, transitions) = match s {
            StateNode::Simple(ss) => (&ss.id, &ss.transitions),
            StateNode::Composite(c) => (&c.id, &c.transitions),
            StateNode::Parallel(p) => (&p.id, &p.transitions),
            _ => {
                // Pseudo-states have no per-state switch case. Still descend
                // so any future regions inside (currently none) are visited.
                walk_state(self, s);
                return;
            }
        };
        if !transitions.is_empty() {
            let idx = self.ctx.index.must_lookup(id);
            let rec = self.ctx.index.get(idx);
            self.out.push_str(&format!(
                "    case {macro}_STATE_{name}: /* {dsl} */\n",
                macro = self.ctx.macro_prefix(),
                name = rec.c_name,
                dsl = rec.dsl_name,
            ));
            // Sort by priority (ascending = lower priority number wins per
            // Doc 08 §4.2), then document order. The IR already preserves
            // document order via Vec, so sort_by_key with priority alone
            // suffices.
            let mut sorted: Vec<&TransitionObject> = transitions.iter().collect();
            sorted.sort_by(|a, b| a.priority.cmp(&b.priority));
            // Group cases by trigger id for cleaner switch output.
            self.out.push_str("        switch (ev->id) {\n");
            for t in &sorted {
                emit_one_case(t, self.ctx, self.out);
            }
            self.out.push_str("        default: break;\n");
            self.out.push_str("        }\n");
            self.out.push_str("        break;\n");
        }
        walk_state(self, s);
    }
}

fn emit_one_case(t: &TransitionObject, ctx: &MachineEmitCtx<'_>, out: &mut String) {
    let trigger_id = match &t.trigger {
        Some(fsm_ir::Trigger::Event { event_id, .. }) => event_id.clone(),
        // `done -> Y` / pre-P0-4 timer triggers come in with no trigger;
        // map to the reserved completion event id.
        Some(fsm_ir::Trigger::Completion { .. }) | None => {
            format!("{}_EVENT__COMPLETION", ctx.macro_prefix())
        }
        // P0-4: timer triggers carry the IR timer id; resolve to the
        // distinct per-timer event variant so this transition is
        // dispatched only on its own timer's fire, not on EVENT__COMPLETION.
        Some(fsm_ir::Trigger::After { timer_id, .. })
        | Some(fsm_ir::Trigger::Every { timer_id, .. }) => {
            super::timer::timer_event_c(ctx, timer_id)
                .unwrap_or_else(|| format!("{}_EVENT__COMPLETION", ctx.macro_prefix()))
        }
    };
    let event_c = if trigger_id.starts_with(&ctx.macro_prefix()) {
        trigger_id.clone()
    } else {
        ctx.event_c_enum(&trigger_id)
    };

    // Resolve the event-specific payload root. The payload union is keyed
    // by event name (`ev->__payload.FAULT`), so `payload.code` in the DSL
    // must lower to `ev->__payload.FAULT.code` inside FAULT's case body.
    // Falling back to the bare union root keeps the code shape sane for
    // events without a declared payload.
    let payload_prefix = trigger_event_payload_prefix(ctx, &trigger_id);

    out.push_str(&format!("        case {}: {{\n", event_c));
    super::transition::emit_transition_body(
        t,
        ctx,
        &payload_prefix,
        /*indent_spaces=*/ 12,
        /*on_guard_fail=*/ "break;",
        out,
    );
    out.push_str("            return true;\n");
    out.push_str("        }\n");
}

fn emit_outer_dispatch(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    format!(
        r#"void {prefix}_dispatch({prefix}_t *m, const {prefix}_Event_t *ev) {{
    /* B-10 + B-11: per-region ancestor walk. For each active leaf in
     * `_active[]`, walk leaf-to-root via parent_table; the first ancestor
     * with a matching transition fires it. Regions iterate independently,
     * so a single event can drive every region in a parallel state in the
     * same RTC step.
     *
     * Audit P0-5 option-b (2026-05-14): the prior `defer_mask` short-circuit
     * lived here and silently dropped events — a documented "store and
     * return" that never actually stored. The analyzer now rejects every
     * `defer EVENT` with FSM-E0903 (Doc 02 G1 compliance), so this path is
     * gone. The mask table is still emitted for inspection / future v1.1
     * defer queue, but no runtime read survives. */
    bool fired_any = false;
    bool fired_in_region[{macro}_MAX_PARALLEL_REGIONS] = {{ false }};
    /* Snapshot active region count up front so transition side effects
     * that change `_active_count` (e.g. cross-out-of-parallel) do not
     * shrink the iteration mid-walk. Doc 08 §4.1: process innermost
     * leaves first (slots 1..N are nested below slot 0), so iterate
     * from high to low. */
    uint8_t initial_active = m->_active_count;
    for (int8_t r = (int8_t)initial_active - 1; r >= 0; r--) {{
        /* Slot may have been cleared by a sibling-region transition
         * (cross-out-of-parallel). */
        if ((uint8_t)r >= m->_active_count) continue;
        /* Skip slots whose region already fired in this RTC step. */
        if (fired_in_region[r]) continue;
        {prefix}_StateId_t s = m->_active[r];
        while (1) {{
            if ({prefix}_try_transitions_in_state(m, s, ev)) {{
                fired_any = true;
                fired_in_region[r] = true;
                break;
            }}
            if (s == {macro}_STATE_ROOT) break;
            s = {prefix}_parent_table[s];
        }}
    }}
    if (fired_any) {{
        {prefix}_handle_completion(m);
    }}
    /* Otherwise: no ancestor handled the event — discard per Doc 08 §3.1. */
}}
"#,
        prefix = prefix,
        macro = macro_prefix,
    )
}

/// Build the per-transition payload prefix. Each event with a non-empty
/// payload schema gets its own member inside the `__payload` union (see
/// `header.rs::emit_payload_structs`), so `payload.X` must address through
/// that member. When the event has no payload schema, fall back to the
/// bare union root — the codegen still emits the dereference but the user
/// guard/action shouldn't reference it.
fn trigger_event_payload_prefix(ctx: &MachineEmitCtx<'_>, trigger_id: &str) -> String {
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
