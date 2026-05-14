//! Timer codegen — Doc 11 §11.
//!
//! Per Doc 00 §10.3 the runtime uses `fsm_hal_clock_now_ms()` for the
//! current time. `Motor_advance_clock(m, elapsed_ms)` is the user-facing
//! tick function (formerly `Motor_tick`); it accepts a pre-computed
//! interval, decrements every active timer, and fires any expired ones.
//!
//! Zero-duration timers are rejected at compile time per B-13. Codegen
//! still emits the `> 0` guard in the tick body so the runtime is robust
//! to any timer the analyzer somehow let through.

use fsm_ir::StateNode;

use super::MachineEmitCtx;

/// Collect every timer declared in the machine in document order. Returns
/// `(state_record_index, timer_field_name, kind, duration_ms, target_state_id_or_none)`.
pub fn collect_timers(ctx: &MachineEmitCtx<'_>) -> Vec<TimerEntry> {
    let mut out = Vec::new();
    fn walk(
        states: &[StateNode],
        index: &crate::state_index::StateIndex,
        out: &mut Vec<TimerEntry>,
    ) {
        for state in states {
            let (id, timers) = match state {
                StateNode::Simple(s) => (&s.id, &s.timers),
                StateNode::Composite(s) => (&s.id, &s.timers),
                StateNode::Parallel(s) => (&s.id, &s.timers),
                _ => continue,
            };
            for t in timers {
                let owner_idx = index.must_lookup(id);
                let target_idx = t.target.as_deref().and_then(|t| index.lookup(t));
                out.push(TimerEntry {
                    field_name: format!(
                        "{}_{}",
                        index.get(owner_idx).c_name,
                        t.stable_id.replace([':', '-'], "_")
                    ),
                    owner_state: owner_idx,
                    kind: t.kind,
                    duration_ms: t.duration_ms,
                    target_state: target_idx,
                });
            }
            // Recurse.
            match state {
                StateNode::Composite(c) => {
                    for r in &c.regions {
                        walk(&r.states, index, out);
                    }
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
                        walk(&r.states, index, out);
                    }
                }
                _ => {}
            }
        }
    }
    walk(&ctx.machine.root.states, ctx.index, &mut out);
    out
}

/// One timer's codegen-relevant facts.
pub struct TimerEntry {
    pub field_name: String,
    pub owner_state: u8,
    pub kind: fsm_ir::TimerKind,
    pub duration_ms: u32,
    pub target_state: Option<u8>,
}

/// Emit the `Motor_advance_clock` body — the tick function.
pub fn emit_advance_clock(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "void {prefix}_advance_clock({prefix}_t *m, uint32_t elapsed_ms) {{\n",
        prefix = prefix,
    ));
    let timers = collect_timers(ctx);
    if timers.is_empty() {
        s.push_str("    (void)m; (void)elapsed_ms;\n");
        s.push_str("}\n");
        let _ = macro_prefix;
        return s;
    }
    // Read clock once for trace correlation (Doc 00 §10.3).
    s.push_str("    (void)fsm_hal_clock_now_ms();\n");
    for timer in &timers {
        let owner = ctx.index.get(timer.owner_state);
        let slot = ctx.layout.slot(timer.owner_state);
        s.push_str(&format!(
            "    if (m->_active[{slot}] == {macro}_STATE_{name} && m->_timer_{tname}_remaining_ms > 0) {{\n",
            slot = slot,
            macro = ctx.macro_prefix(),
            name = owner.c_name,
            tname = timer.field_name,
        ));
        s.push_str(&format!(
            "        if (elapsed_ms >= m->_timer_{tname}_remaining_ms) {{\n",
            tname = timer.field_name,
        ));
        s.push_str(&format!(
            "            m->_timer_{tname}_remaining_ms = 0;\n",
            tname = timer.field_name,
        ));
        s.push_str("            /* fire: build internal completion-style event */\n");
        s.push_str(&format!(
            "            {prefix}_Event_t timer_ev;\n            timer_ev.id = {macro}_EVENT__COMPLETION;\n",
            prefix = prefix,
            macro = ctx.macro_prefix(),
        ));
        s.push_str(&format!(
            "            {prefix}_dispatch(m, &timer_ev);\n",
            prefix = prefix,
        ));
        // Periodic timers re-arm.
        if matches!(
            timer.kind,
            fsm_ir::TimerKind::Every | fsm_ir::TimerKind::EveryInternal
        ) {
            s.push_str(&format!(
                "            m->_timer_{tname}_remaining_ms = {dur}u;\n",
                tname = timer.field_name,
                dur = timer.duration_ms,
            ));
        }
        s.push_str("        } else {\n");
        s.push_str(&format!(
            "            m->_timer_{tname}_remaining_ms -= elapsed_ms;\n",
            tname = timer.field_name,
        ));
        s.push_str("        }\n");
        s.push_str("    }\n");
    }
    s.push_str("}\n");
    s
}
