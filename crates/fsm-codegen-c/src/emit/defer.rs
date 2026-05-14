//! Deferred event codegen — Doc 11 §22 (parked until v1.1).
//!
//! Per-state defer bitmask. Each state lists the events it defers. When
//! `Motor_dispatch` receives a deferred event, it stores it in a side
//! buffer rather than processing it. On state change, deferred events
//! whose new-state mask does not include them are released.
//!
//! Audit P0-5 option-b (2026-05-14): the v1.0 implementation never had a
//! working queue — the dispatch path silently dropped events on a defer
//! hit, contradicting Doc 02 G1 ("no undefined behaviour"). Until the v1.1
//! queue ships, `defer EVENT` is rejected at analysis time with
//! FSM-E0903. The mask emitter below is preserved for v1.1 revival but is
//! not invoked by `source.rs` in v1.0; the `#[allow(dead_code)]` is
//! deliberate (Doc 00 §6 / §10 follow-up).

#![allow(dead_code)]

use fsm_ir::StateNode;

use super::MachineEmitCtx;

/// Emit `Motor_defer_mask[]` — one bitmask per state. Parked: see module
/// docs.
pub fn emit_defer_mask(ctx: &MachineEmitCtx<'_>) -> String {
    let prefix = ctx.type_prefix();
    let macro_prefix = ctx.macro_prefix();
    let mut s = String::new();
    s.push_str(&format!(
        "/* Per-state event defer mask (Doc 11 §22). Bit N = event ID N is deferred. */\n",
    ));
    s.push_str(&format!(
        "static const uint32_t {prefix}_defer_mask[{macro}_STATE__COUNT] = {{\n",
        prefix = prefix,
        macro = macro_prefix,
    ));
    // Always emit an entry for the root sentinel (zero) so the initializer
    // is non-empty — `static const int arr[] = {{}}` is an ISO C9-pedantic
    // error. The remaining states list themselves only when they defer
    // anything; designated initialisers default everything else to zero.
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
            .sum();
        s.push_str(&format!(
            "    [{macro}_STATE_{name}] = 0x{mask:08x}u,\n",
            macro = macro_prefix,
            name = rec.c_name,
            mask = mask,
        ));
    }
    s.push_str("};\n");
    s
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
