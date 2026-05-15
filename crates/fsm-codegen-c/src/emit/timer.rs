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
//!
//! P0-4: each timer is assigned a distinct event variant
//! `MOTOR_EVENT_TIMER_<suffix>_FIRED` so timer-driven transitions are
//! dispatchable independently of `done` completion events. The suffix is
//! derived from the IR timer's owner-state c-name and the kind/index so
//! that two timers in the same state still produce unique enum names.

use fsm_ir::StateNode;

use super::MachineEmitCtx;

/// Sanitise an arbitrary string into an uppercase C identifier suffix.
fn timer_suffix(timer_id: &str) -> String {
    let mut s: String = timer_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    s.make_ascii_uppercase();
    s
}

/// Collect every timer declared in the machine in document order.
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
                let event_suffix = format!(
                    "TIMER_{}_FIRED",
                    timer_suffix(&t.stable_id.replace([':', '-'], "_"))
                );
                out.push(TimerEntry {
                    timer_id: t.id.clone(),
                    field_name: format!(
                        "{}_{}",
                        index.get(owner_idx).c_name,
                        t.stable_id.replace([':', '-'], "_")
                    ),
                    event_suffix,
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
    /// IR-level stable id (Doc 09 §13 `TimerObject.id`). Codegen uses this
    /// to match `Trigger::After { timer_id }` / `Trigger::Every { timer_id }`
    /// to the timer that owns the event variant.
    pub timer_id: String,
    pub field_name: String,
    /// The per-timer event-enum suffix — `TIMER_<X>_FIRED`. Combined with
    /// the machine macro prefix produces e.g. `MOTOR_EVENT_TIMER_X_FIRED`.
    pub event_suffix: String,
    pub owner_state: u8,
    pub kind: fsm_ir::TimerKind,
    pub duration_ms: u32,
    pub target_state: Option<u8>,
}

/// Look up the per-timer C event-enum name (including the machine prefix)
/// from the IR timer id.
pub fn timer_event_c(ctx: &MachineEmitCtx<'_>, timer_id: &str) -> Option<String> {
    let timers = collect_timers(ctx);
    timers
        .iter()
        .find(|t| t.timer_id == timer_id)
        .map(|t| format!("{}_EVENT_{}", ctx.macro_prefix(), t.event_suffix))
}

/// Emit the `Motor_advance_clock` body — the tick function.
pub fn emit_advance_clock(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "void {prefix}_advance_clock({prefix}_t *m, uint32_t elapsed_ms) {{\n",
        prefix = prefix,
    ));
    let timers = collect_timers(ctx);
    if timers.is_empty() {
        s.push_str("    (void)m; (void)elapsed_ms;\n");
        s.push_str("}\n");
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
        // P0-4: dispatch the timer's own event variant so the transition
        // is distinguishable from `done` completion.
        s.push_str(&format!(
            "            {prefix}_Event_t timer_ev;\n            timer_ev.id = {macro}_EVENT_{esuffix};\n",
            prefix = prefix,
            macro = ctx.macro_prefix(),
            esuffix = timer.event_suffix,
        ));
        // Periodic timers re-arm BEFORE dispatch — Doc 08 §13.3 zero-drift:
        // schedule next fire at `expiry + period`. Re-arm first so a
        // transition that swaps state in dispatch does not undo the arm.
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
        s.push_str(&format!(
            "            {prefix}_dispatch(m, &timer_ev);\n",
            prefix = prefix,
        ));
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
