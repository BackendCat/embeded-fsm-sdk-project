//! Deferred-event codegen — Doc 08 §10 / Doc 11 §12 (v1.1).
//!
//! UML 2.5.1 deferred-event semantics. An event whose id appears in the
//! `defer` set of *some* state in the active configuration, and which no
//! transition consumes (transition-wins, checked AFTER the leaf-to-root
//! search fails), is HELD in a fixed-capacity buffer rather than
//! discarded. On every configuration change, deferred events no longer
//! covered by an active deferring state are released to the FRONT of the
//! main event queue in FIFO order (Doc 08 §10.2 / §10.3), respecting the
//! §10.4 recursion-prevention rule (an event still deferred by a state
//! that remains active is NOT released — it stays held, not churned).
//!
//! Storage (heap-free per Doc 02 G2): a bounded `_deferred[]` array in the
//! machine struct plus a `_deferred_count`. A true ring buffer is not used
//! because release filters *arbitrary* elements (some kept by §10.4, some
//! released) — that degenerates a ring to a linear scan anyway, so a plain
//! bounded array with in-place compaction is the simplest construction
//! that provably mirrors the simulator's `release_deferred` partition
//! (the simulator-trace-match gate depends on identical behavior).
//!
//! v1.1 supersedes the audit P0-5 option-b stopgap (docs/00 §11.7): the
//! analyzer no longer rejects `defer` and FSM-E0903 is retired.

use fsm_ir::StateNode;

use super::MachineEmitCtx;

/// Emit the per-state defer-set membership table. `Motor_defer_table[s]` is
/// a bitmask: bit N set ⇔ state `s` defers event id N. Event ids are the
/// machine's event index (same ordering as the `Motor_EventId_t` enum).
///
/// Why a bitmask and not a function-with-switch: the membership query runs
/// on the dispatch hot path for every otherwise-unconsumed event, and it
/// is also evaluated for every active leaf on every config change. A flat
/// `.rodata` table keyed by StateId is O(1) and branch-free.
pub fn emit_defer_table(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(
        "/* Per-state event defer mask (Doc 08 §10). Bit N = event id N is deferred\n \
         * by this state. Designated initialisers default non-deferring states to 0. */\n",
    );
    s.push_str(&format!(
        "static const uint32_t {prefix}_defer_table[{macro}_STATE__COUNT] = {{\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    // The root sentinel (index 0) never defers; emit an explicit 0 so the
    // initialiser is non-empty (ISO C99 forbids `= {}` for a const array
    // under -Wpedantic).
    s.push_str("    0u,\n");
    for rec in ctx.index.records.iter().skip(1) {
        let defers = collect_state_defers(ctx, &rec.ir_id);
        if defers.is_empty() {
            continue;
        }
        let mask: u32 = defers
            .iter()
            .filter_map(|eid| event_index(ctx, eid))
            .map(|idx| 1u32 << idx)
            .fold(0u32, |acc, bit| acc | bit);
        s.push_str(&format!(
            "    [{macro}_STATE_{name}] = 0x{mask:08x}u, /* {dsl} */\n",
            macro = macro_prefix,
            name = rec.c_name,
            mask = mask,
            dsl = rec.dsl_name,
        ));
    }
    s.push_str("};\n");
    s
}

/// Emit the defer-buffer runtime: membership predicate, push (hold),
/// release (drain matching to the front of the main queue), and the
/// front-insert queue primitive the release path needs.
///
/// Wired into both dispatch strategies (see `dispatch_switch.rs` /
/// `dispatch_table.rs`): push when an unconsumed event is deferred by the
/// active config; release after every configuration-changing transition.
pub fn emit_defer_runtime(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    format!(
        r#"/* True iff some state in the current active configuration defers
 * `event_id` (the active leaves plus all of their ancestors via the
 * parent table). Doc 08 §10.1. */
static bool {prefix}_active_config_defers({prefix}_t *m, {prefix}_EventId_t event_id) {{
    if ((unsigned)event_id >= 32u) return false; /* mask is 32-bit */
    uint32_t bit = (uint32_t)1u << (unsigned)event_id;
    for (uint8_t r = 0; r < m->_active_count; r++) {{
        {prefix}_StateId_t s = m->_active[r];
        while (1) {{
            if ({prefix}_defer_table[s] & bit) return true;
            if (s == {macro}_STATE_ROOT) break;
            s = {prefix}_parent_table[s];
        }}
    }}
    return false;
}}

/* Hold an unconsumed-but-deferred event. Doc 08 §10.1. Overflow honours
 * the same policy as the main queue (Doc 11 §6): assert, or drop-newest. */
static void {prefix}_defer_push({prefix}_t *m, const {prefix}_Event_t *ev) {{
    if (m->_deferred_count >= {macro}_DEFER_CAPACITY) {{
#if {macro}_QUEUE_OVERFLOW == FSM_QUEUE_ASSERT
        FSM_ASSERT(0);
#else
        return; /* DROP_NEWEST: the deferred event is dropped */
#endif
    }}
    m->_deferred[m->_deferred_count++] = *ev;
}}

/* Front-insert into the main ring queue (Doc 08 §10.3 / §14 — released
 * deferred events, `raise`, and completion events go to the head). Never
 * evicts the head; a full queue on a front-insert is a hard overflow
 * because dropping the head would violate the internal-event ordering
 * invariant. */
static void {prefix}_queue_push_front({prefix}_t *m, const {prefix}_Event_t *ev) {{
    if (m->_queue_count >= {macro}_QUEUE_CAPACITY) {{
        FSM_ASSERT(0);
        return;
    }}
    m->_queue_head = (uint8_t)((m->_queue_head + {macro}_QUEUE_CAPACITY - 1u)
                               & ({macro}_QUEUE_CAPACITY - 1u));
    m->_queue[m->_queue_head] = *ev;
    m->_queue_count++;
}}

/* Doc 08 §10.2 — after a configuration change, release every deferred
 * event no longer covered by an active deferring state to the FRONT of
 * the main queue, in FIFO order (the order they were deferred). Doc 08
 * §10.4 recursion prevention: an event still deferred by a state that
 * remains active is kept (compacted down), NOT released — so it is not
 * immediately re-deferred / churned.
 *
 * FIFO-to-front: iterate the held buffer back-to-front and front-insert
 * each released event, so the earliest-deferred ends up at the very head
 * (queue becomes [d0, d1, ..., <prior queue>]). Mirrors the simulator's
 * `queue.prepend(release_in_fifo_order)`. */
static void {prefix}_release_deferred({prefix}_t *m) {{
    if (m->_deferred_count == 0u) return;
    /* Partition in place: keep[] (still deferred) front, release order
     * captured separately. Both preserve relative (FIFO) order. */
    {prefix}_Event_t kept[{macro}_DEFER_CAPACITY];
    uint8_t kept_n = 0;
    /* First pass front-to-back: emit releases oldest-first by buffering
     * their indices; simplest correct construction is two ordered passes. */
    for (uint8_t i = 0; i < m->_deferred_count; i++) {{
        {prefix}_EventId_t eid = m->_deferred[i].id;
        if ({prefix}_active_config_defers(m, eid)) {{
            kept[kept_n++] = m->_deferred[i];
        }}
    }}
    /* Release pass back-to-front so front-insert yields oldest-at-head. */
    for (int16_t i = (int16_t)m->_deferred_count - 1; i >= 0; i--) {{
        {prefix}_EventId_t eid = m->_deferred[(uint8_t)i].id;
        if (!{prefix}_active_config_defers(m, eid)) {{
            {prefix}_queue_push_front(m, &m->_deferred[(uint8_t)i]);
        }}
    }}
    /* Compact the held buffer down to the kept set. */
    for (uint8_t i = 0; i < kept_n; i++) {{
        m->_deferred[i] = kept[i];
    }}
    m->_deferred_count = kept_n;
}}
"#,
        prefix = prefix,
        macro = macro_prefix,
    )
}

/// Whether any state in the machine declares at least one `defer`. When
/// false, the emitters skip the entire defer apparatus so non-defer
/// machines pay zero code size (and avoid `-Werror=unused` on the helpers).
pub fn machine_has_defer(ctx: &MachineEmitCtx<'_>) -> bool {
    fn walk(states: &[StateNode]) -> bool {
        states.iter().any(|s| match s {
            StateNode::Simple(ss) => !ss.defers.is_empty(),
            StateNode::Composite(c) => {
                !c.defers.is_empty() || c.regions.iter().any(|r| walk(&r.states))
            }
            StateNode::Parallel(p) => {
                !p.defers.is_empty() || p.regions.iter().any(|r| walk(&r.states))
            }
            _ => false,
        })
    }
    walk(&ctx.machine.root.states)
}

fn collect_state_defers(ctx: &MachineEmitCtx<'_>, ir_state_id: &str) -> Vec<String> {
    fn walk<'a>(states: &'a [StateNode], target_id: &str) -> Option<Vec<String>> {
        for s in states {
            match s {
                StateNode::Simple(ss) if ss.id == target_id => {
                    return Some(ss.defers.iter().map(|d| d.event_id.clone()).collect());
                }
                StateNode::Composite(c) => {
                    if c.id == target_id {
                        return Some(c.defers.iter().map(|d| d.event_id.clone()).collect());
                    }
                    for r in &c.regions {
                        if let Some(hit) = walk(&r.states, target_id) {
                            return Some(hit);
                        }
                    }
                }
                StateNode::Parallel(p) => {
                    if p.id == target_id {
                        return Some(p.defers.iter().map(|d| d.event_id.clone()).collect());
                    }
                    for r in &p.regions {
                        if let Some(hit) = walk(&r.states, target_id) {
                            return Some(hit);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&ctx.machine.root.states, ir_state_id).unwrap_or_default()
}

fn event_index(ctx: &MachineEmitCtx<'_>, ir_event_id: &str) -> Option<usize> {
    ctx.machine.events.iter().position(|e| e.id == ir_event_id)
}
