//! Event queue codegen — Doc 11 §12.
//!
//! Fixed-capacity ring buffer indexed with bitwise-AND modulo. The queue
//! capacity is a power of two enforced at codegen time (see
//! `emit::is_power_of_two`).

use super::MachineEmitCtx;

/// Emit the `Motor_post`, `Motor_dequeue`, internal `Motor_queue_push`,
/// and `Motor_queue_push_front` helpers.
pub fn emit_queue(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        r#"static void {prefix}_queue_push({prefix}_t *m, const {prefix}_Event_t *ev) {{
    if (m->_queue_count >= {macro}_QUEUE_CAPACITY) {{
#if {macro}_QUEUE_OVERFLOW == FSM_QUEUE_ASSERT
        FSM_ASSERT(0);
#else
        return; /* DROP_NEWEST */
#endif
    }}
    m->_queue[m->_queue_tail] = *ev;
    m->_queue_tail = (uint8_t)((m->_queue_tail + 1u) & ({macro}_QUEUE_CAPACITY - 1u));
    m->_queue_count++;
}}

void {prefix}_post({prefix}_t *m, const {prefix}_Event_t *ev) {{
#ifdef {macro}_QUEUE_ISR_SAFE
    FSM_ENTER_CRITICAL();
#endif
    {prefix}_queue_push(m, ev);
#ifdef {macro}_QUEUE_ISR_SAFE
    FSM_EXIT_CRITICAL();
#endif
}}

bool {prefix}_dequeue({prefix}_t *m, {prefix}_Event_t *out) {{
    if (m->_queue_count == 0u) return false;
    *out = m->_queue[m->_queue_head];
    m->_queue_head = (uint8_t)((m->_queue_head + 1u) & ({macro}_QUEUE_CAPACITY - 1u));
    m->_queue_count--;
    return true;
}}
"#,
        prefix = prefix,
        macro = macro_prefix,
    ));
    s
}
