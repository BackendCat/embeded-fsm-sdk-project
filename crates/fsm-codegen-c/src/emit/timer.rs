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
///
/// **Semantics: an expiry-walk that *consumes* the elapsed budget, byte-for-
/// byte mirroring `fsm_simulator::Interpreter::advance_clock`** (the shipped
/// simulator is the correct, unforked reference the W1 host-trace
/// differential — Doc 32 §1 W1 / §2 — trusts). The simulator computes
/// `target = clock + Δ`, then loops: find the *soonest* armed-timer expiry;
/// if it is `> target` stop; advance the virtual clock **to that expiry**,
/// fire every timer due at that instant, then drain to quiescence so a
/// transition's entry actions arm fresh timers **relative to the
/// already-advanced clock** (`expiry = now + duration`). A timer armed
/// *during* the walk therefore only fires on a *later* iteration, and only
/// if its own duration fits in the *remaining* (not the original) budget.
///
/// In the generated runtime's decrementing `_timer_<f>_remaining_ms`
/// representation there is no absolute per-timer clock; `remaining = expiry
/// - now`. The simulator's "advance the virtual clock to the next expiry"
/// is *exactly* "subtract that next-expiry delta from **every** active
/// timer's remaining" in this representation. So the faithful mirror is:
///
/// ```text
/// budget := elapsed_ms
/// loop {
///     step := min positive _remaining over the *active* armed timers
///     if no such step OR step > budget {
///         subtract `budget` from every active timer's _remaining; stop
///     }
///     subtract `step` from every active timer's _remaining   // = advance clock to next expiry
///     budget -= step
///     for each active timer now at _remaining == 0 {          // = pop_fired_through(now)
///         periodic? re-arm to `duration`
///         dispatch its timer event                            // entry actions arm fresh
///     }                                                        //   timers at `duration`,
/// }                                                            //   anchored to *here*
/// ```
///
/// The **previous** implementation iterated the *static document-order*
/// timer list once and tested `elapsed_ms >= remaining` per timer with the
/// **full, un-consumed** `elapsed_ms`. After a timer fired, its dispatched
/// transition's entry set armed the next state's timer to its full
/// `duration`; the loop then reached *that* timer still holding the whole
/// `elapsed_ms`, so a timer **armed during this very `advance_clock`** was
/// re-fired by budget the first fire had already spent — the elapsed budget
/// was *re-used per timer instead of consumed*. On `traffic-light`
/// (`advance_clock(2001)` from `Red`) it cascaded `Red→GreenAccelerating→
/// Green→…` in a *single* call and landed in the wrong state. This rewrite
/// consumes the budget step-by-step so a freshly-armed timer is anchored to
/// the advanced point — one fire per `advance_clock`, exactly as the
/// simulator does.
///
/// The fire-loop iteration count is bounded by the number of timers that
/// can sequentially expire within `elapsed_ms`; the guard is the
/// budget-decrementing `while` (each iteration consumes a strictly positive
/// `step`, or the no-`step`/`step > budget` branch stops), so it always
/// terminates.
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
        return s;
    }
    // Read clock once for trace correlation (Doc 00 §10.3).
    s.push_str("    (void)fsm_hal_clock_now_ms();\n");

    // `budget` is the still-unconsumed slice of `elapsed_ms` — the
    // simulator's `target - virtual_clock_ms`. Each loop iteration either
    // walks the clock forward to the next expiry (consuming `step` of the
    // budget and firing the timers due there) or, when nothing more expires
    // within the budget, drains the rest into every active timer and stops.
    s.push_str("    uint32_t budget = elapsed_ms;\n");
    s.push_str("    for (;;) {\n");

    // (1) `step` = the soonest expiry among the *active* armed timers (the
    // simulator's `TimerSet::next_expiry_ms`, restricted — as the codegen
    // runtime always was — to timers whose owner state is active). 0 ⇒ none
    // armed. A timer is "armed & active" iff its owner slot holds its state
    // AND its remaining is > 0.
    s.push_str("        uint32_t step = 0u;\n");
    for timer in &timers {
        let owner = ctx.index.get(timer.owner_state);
        let slot = ctx.layout.slot(timer.owner_state);
        s.push_str(&format!(
            "        if (m->_active[{slot}] == {macro}_STATE_{name} && m->_timer_{tname}_remaining_ms > 0u\n            && (step == 0u || m->_timer_{tname}_remaining_ms < step)) {{ step = m->_timer_{tname}_remaining_ms; }}\n",
            slot = slot,
            macro = macro_prefix,
            name = owner.c_name,
            tname = timer.field_name,
        ));
    }

    // (2) Nothing armed, or the soonest expiry is beyond the remaining
    // budget: drain the rest of the budget into every active timer
    // (the simulator's "finally, advance virtual clock to target" with no
    // further fire) and stop.
    s.push_str("        if (step == 0u || step > budget) {\n");
    for timer in &timers {
        let owner = ctx.index.get(timer.owner_state);
        let slot = ctx.layout.slot(timer.owner_state);
        s.push_str(&format!(
            "            if (m->_active[{slot}] == {macro}_STATE_{name} && m->_timer_{tname}_remaining_ms > 0u) {{ m->_timer_{tname}_remaining_ms -= budget; }}\n",
            slot = slot,
            macro = macro_prefix,
            name = owner.c_name,
            tname = timer.field_name,
        ));
    }
    s.push_str("            break;\n");
    s.push_str("        }\n");

    // (3) Advance the virtual clock to that next expiry: subtract `step`
    // from EVERY active armed timer's remaining (the relative-representation
    // equivalent of `rt.virtual_clock_ms = next`), and consume it from the
    // budget. Timers whose remaining reaches 0 here are now due.
    s.push_str("        budget -= step;\n");
    for timer in &timers {
        let owner = ctx.index.get(timer.owner_state);
        let slot = ctx.layout.slot(timer.owner_state);
        s.push_str(&format!(
            "        if (m->_active[{slot}] == {macro}_STATE_{name} && m->_timer_{tname}_remaining_ms > 0u) {{ m->_timer_{tname}_remaining_ms -= step; }}\n",
            slot = slot,
            macro = macro_prefix,
            name = owner.c_name,
            tname = timer.field_name,
        ));
    }

    // (4) Fire every timer now at remaining == 0 (the simulator's
    // `pop_fired_through(now)` then drain-to-quiescent). Document order is
    // the stable tiebreak for any same-instant ties — matching the
    // simulator's stable `sort_by_key(expiry)` over its armed `Vec`. Each
    // `_dispatch` may transition; its entry actions arm the next state's
    // timer to its full `duration` (transition.rs), anchored to THIS
    // advanced point — so it can only fire on a LATER iteration, and only
    // if `duration <= budget`. That is precisely the simulator's
    // armed-during-the-walk semantics; the over-fire is gone.
    for timer in &timers {
        let owner = ctx.index.get(timer.owner_state);
        let slot = ctx.layout.slot(timer.owner_state);
        s.push_str(&format!(
            "        if (m->_active[{slot}] == {macro}_STATE_{name} && m->_timer_{tname}_remaining_ms == 0u) {{\n",
            slot = slot,
            macro = macro_prefix,
            name = owner.c_name,
            tname = timer.field_name,
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
        // P0-4: dispatch the timer's own event variant so the transition
        // is distinguishable from `done` completion.
        s.push_str(&format!(
            "            {prefix}_Event_t timer_ev;\n            timer_ev.id = {macro}_EVENT_{esuffix};\n",
            prefix = prefix,
            macro = macro_prefix,
            esuffix = timer.event_suffix,
        ));
        s.push_str(&format!(
            "            {prefix}_dispatch(m, &timer_ev);\n",
            prefix = prefix,
        ));
        s.push_str("        }\n");
    }

    s.push_str("    }\n");
    s.push_str("}\n");
    s
}
