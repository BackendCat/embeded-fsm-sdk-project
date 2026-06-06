//! Deferred-event bookkeeping — Doc 08 §10. The decision to *hold* an event
//! (vs discard) happens in [`super::step::rtc_step`]; the *release* sweep
//! after a transition lives here so the FIFO + recursion-prevention rules
//! (§10.2 / §10.4) are stated once.

use std::collections::HashSet;

use crate::runtime::{EventKind, QueuedEvent, RuntimeState};

use super::StepError;

/// Returns the set of event ids deferred by *any* state in the current
/// active configuration (each active leaf plus all of its ancestors).
/// Doc 08 §10.1: an event is held only if a state that is currently
/// active declares `defer` for it.
pub(super) fn active_config_deferred_ids(rt: &RuntimeState) -> HashSet<String> {
    rt.active_states
        .iter()
        .flat_map(|s| {
            rt.machine.ancestors(s).into_iter().flat_map(|id| {
                rt.machine
                    .node(&id)
                    .map(|n| n.defers.clone())
                    .unwrap_or_default()
            })
        })
        .map(|d| d.event_id)
        .collect()
}

/// Whether `event_id` is deferred by some state in the active
/// configuration. Used by `run_step` to decide hold-vs-discard for an
/// unconsumed event (Doc 08 §10.1).
pub(super) fn active_config_defers(rt: &RuntimeState, event_id: &str) -> bool {
    active_config_deferred_ids(rt).contains(event_id)
}

/// Doc 08 §10.2 — when the machine exits a deferring state, any deferred
/// events that no longer match an active deferring state are released to
/// the front of the queue in FIFO order (the order they were deferred).
///
/// Doc 08 §10.4 recursion prevention: an event still deferred by a state
/// that remains active after the transition is NOT released — it stays in
/// `defer_set` so it is not immediately re-deferred / churned. Only events
/// no longer covered by any active deferring state are released.
///
/// Released events are flagged `redispatched` so the subsequent
/// reprocessing step records as `StepKind::EventRedispatched` (the
/// defer→release round-trip is visible in the trace; Doc 13 §11).
pub(super) fn release_deferred(rt: &mut RuntimeState) -> Result<(), StepError> {
    if rt.defer_set.is_empty() {
        return Ok(());
    }
    let active_deferred = active_config_deferred_ids(rt);
    // Preserve FIFO: `defer_set` is in deferral order, so `release` keeps
    // that order and `keep` keeps the relative order of still-deferred
    // events.
    let mut release: Vec<String> = Vec::new();
    let mut keep: Vec<String> = Vec::new();
    for ev in rt.defer_set.drain(..) {
        if active_deferred.contains(&ev) {
            keep.push(ev);
        } else {
            release.push(ev);
        }
    }
    rt.defer_set = keep;
    // Prepend in FIFO order (Doc 08 §10.3): queue becomes
    // [D, E, <prior queue contents>] when D was deferred before E.
    let evs: Vec<QueuedEvent> = release
        .into_iter()
        .map(|event_id| QueuedEvent {
            kind: EventKind::Dispatched { event_id },
            payload: None,
            redispatched: true,
        })
        .collect();
    rt.queue.prepend(evs)?;
    Ok(())
}
