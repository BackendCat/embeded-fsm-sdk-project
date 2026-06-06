//! Submachine sub-instance lifecycle wiring — Doc 08 §12.
//!
//! Composes the parent RTC with nested sub-`RuntimeState`s without
//! duplicating defer / done-autofire / completion: instantiation reuses the
//! same entry-sequence routine as `init`; completion only *enqueues the
//! parent `Completion`* and lets the existing R1 path fire `done ->`;
//! teardown is a deterministic drop of the nested runtime.

use crate::eval::{ExecOutcome, ExternRegistry};
use crate::runtime::{
    check_and_enqueue_completion, EventKind, NodeKind, QueuedEvent, RuntimeState, Value,
};
use crate::trace::{StepKind, StepRecord};

use super::run::enter_state_path;
use super::step::drain_queue_rt;
use super::StepError;

/// Reconcile sub-instances with the active configuration after an RTC step
/// (or the initial entry sequence). Three jobs, all Doc 08 §12:
///
/// 1. **Instantiate** — every active `StateNode::Submachine` ref-state with
///    no live sub-instance gets one, initialised at the template's initial
///    pseudo-state (§12.2). Records `SubmachineEntered`.
/// 2. **Complete** — a sub-instance whose active leaf is `Final` (§12.3)
///    causes the parent to enqueue `Completion(ref_state)` so the
///    ref-state's `done -> Target` fires through the *existing* R1
///    completion path. Records `SubmachineCompleted` once (idempotent: a
///    completed-but-not-yet-exited sub is not re-enqueued).
/// 3. **Teardown** — a sub-instance whose ref-state is no longer active is
///    dropped deterministically (no leak).
///
/// Recursive: a freshly-instantiated / event-driven sub-instance is itself
/// reconciled (its own nested refs) via `drain_queue_rt`, one `depth`
/// deeper. `MAX_SUBMACHINE_DEPTH` bounds this against malformed IR that
/// slipped FSM-E0502.
pub(super) fn sync_submachines(
    rt: &mut RuntimeState,
    externs: &ExternRegistry,
    depth: u32,
) -> Result<Vec<StepRecord>, StepError> {
    if depth >= crate::runtime::MAX_SUBMACHINE_DEPTH {
        return Err(StepError::Internal(format!(
            "submachine nesting exceeded {} — malformed IR (FSM-E0502 should reject template cycles)",
            crate::runtime::MAX_SUBMACHINE_DEPTH
        )));
    }
    let mut out = Vec::new();

    // (3) Teardown first: drop sub-instances whose ref-state left the
    // active configuration. Deterministic order via the BTreeMap keys.
    let stale: Vec<String> = rt
        .submachines
        .keys()
        .filter(|ref_id| !rt.active_states.iter().any(|a| a == *ref_id))
        .cloned()
        .collect();
    for ref_id in stale {
        rt.submachines.remove(&ref_id);
    }

    // (1) Instantiate newly-active ref-states. `active_states` order is
    // deterministic (region-declaration order) so records are stable.
    let new_refs: Vec<String> = rt
        .active_states
        .iter()
        .filter(|s| {
            matches!(
                rt.machine.node(s).map(|n| &n.kind),
                Some(NodeKind::SubmachineRef)
            ) && !rt.submachines.contains_key(*s)
        })
        .cloned()
        .collect();
    for ref_id in new_refs {
        let Some(mut sub_rt) = crate::runtime::build_sub_runtime(&rt.machine.clone(), &ref_id)
        else {
            // Unresolved ref (FSM-E0103 at analysis) — degrade to a leaf
            // no-op rather than panic (Doc 09 §1 partial-IR principle).
            continue;
        };
        let Some((root_id, initial_target)) = crate::runtime::entry_target(&sub_rt.machine.clone())
        else {
            continue;
        };
        // Initialise the sub like `Interpreter::init` does a machine:
        // default-populate context, run the entry sequence to the leaf.
        init_sub_context(&mut sub_rt);
        sub_rt.initialized = true;
        let mut sub_outcome = ExecOutcome::default();
        let mut sub_entered = Vec::new();
        enter_state_path(
            &mut sub_rt,
            &root_id,
            &initial_target,
            &mut sub_entered,
            &mut sub_outcome,
            externs,
        )?;
        // The sub's own initially-entered states may carry `done ->` or
        // reach a nested completion — run its completion check + drain so
        // the sub settles before the parent records the enter.
        for s in sub_entered.clone() {
            check_and_enqueue_completion(&mut sub_rt, &s)?;
        }
        let sub_config_after = sub_rt.active_states.clone();
        out.push(StepRecord {
            trace_id: rt.next_trace_id,
            kind: StepKind::SubmachineEntered,
            virtual_clock_ms: rt.virtual_clock_ms,
            event_received: None,
            transition_taken: None,
            exited_states: vec![],
            entered_states: vec![],
            actions_executed: sub_outcome.actions_executed,
            config_before: rt.active_states.clone(),
            config_after: rt.active_states.clone(),
            submachine: Some(crate::trace::SubmachineRecord {
                ref_state_id: ref_id.clone(),
                sub_config_before: vec![],
                sub_config_after,
                delegated_event: None,
            }),
        });
        rt.next_trace_id += 1;
        // Drain the sub's internal queue (completion / raised) recursively
        // one depth deeper, then store it nested under the parent.
        let sub_records = drain_queue_rt(&mut sub_rt, externs, depth + 1)?;
        out.extend(reparent_sub_records(rt, sub_records, &ref_id));
        rt.submachines.insert(ref_id.clone(), Box::new(sub_rt));
        // The just-instantiated sub may already be Final (a trivial
        // template) — fall through to the completion sweep below.
    }

    // (2) Completion sweep: any live sub at Final drives the parent's
    // `done ->` through the existing R1 completion machinery. We mark the
    // ref-state as completion-pending by enqueuing `Completion(ref_id)`
    // once; the next `drain_queue_rt` loop iteration fires the transition
    // and the subsequent teardown sweep drops the spent sub-instance.
    let final_refs: Vec<String> = rt
        .submachines
        .iter()
        .filter(|(_, sub)| crate::runtime::sub_reached_final(sub))
        .map(|(k, _)| k.clone())
        .collect();
    for ref_id in final_refs {
        // Idempotency: only enqueue if a `Completion(ref_id)` is not
        // already queued (a completed sub stays Final until its ref-state
        // exits; without this guard every sync would re-enqueue and trip
        // the §9.4 completion-loop cap).
        let already_queued = rt
            .queue
            .iter()
            .any(|q| matches!(&q.kind, EventKind::Completion { state_id } if state_id == &ref_id));
        if already_queued {
            continue;
        }
        let sub_cfg = rt
            .submachines
            .get(&ref_id)
            .map(|s| s.active_states.clone())
            .unwrap_or_default();
        out.push(StepRecord {
            trace_id: rt.next_trace_id,
            kind: StepKind::SubmachineCompleted,
            virtual_clock_ms: rt.virtual_clock_ms,
            event_received: None,
            transition_taken: None,
            exited_states: vec![],
            entered_states: vec![],
            actions_executed: vec![],
            config_before: rt.active_states.clone(),
            config_after: rt.active_states.clone(),
            submachine: Some(crate::trace::SubmachineRecord {
                ref_state_id: ref_id.clone(),
                sub_config_before: sub_cfg.clone(),
                sub_config_after: sub_cfg,
                delegated_event: None,
            }),
        });
        rt.next_trace_id += 1;
        // Doc 08 §12.3 → the parent receives a synthetic completion; reuse
        // the established completion event so the ref-state's
        // `done -> Target` (a `TransitionKind::Completion` edge on the
        // SubmachineRef) fires via the *same* selection path that handles
        // every other completion. We do NOT re-implement `done` here.
        rt.queue.push_front(QueuedEvent::new(
            EventKind::Completion {
                state_id: ref_id.clone(),
            },
            None,
        ))?;
    }

    Ok(out)
}

/// Try to delegate a parent-unconsumed event into the active ref-state's
/// sub-instance (Doc 08 §12.1). Returns `Some(record)` if delegated (the
/// event resolved in the sub's event table and a sub-RTC step ran),
/// `None` if there is nothing to delegate to (no live sub, or the sub
/// doesn't declare this event — the caller then falls through to the
/// normal defer / discard path, so a non-submachine model is unaffected).
pub(super) fn try_delegate_to_submachine(
    rt: &mut RuntimeState,
    externs: &ExternRegistry,
    event: &QueuedEvent,
    depth: u32,
    config_before: &[String],
    virtual_clock_ms: u64,
) -> Result<Option<Vec<StepRecord>>, StepError> {
    // Only externally-meaningful events delegate; completion / timer
    // events have no DSL name and are sub-context-local concerns.
    let event_id = match &event.kind {
        EventKind::Dispatched { event_id } | EventKind::Raised { event_id } => event_id.clone(),
        EventKind::Completion { .. } | EventKind::TimerFire { .. } => return Ok(None),
    };
    // Resolve the parent event's *name* — the sub-template has an
    // independent event-id namespace (Doc 08 §12.1), so delegation is by
    // name, re-resolved against the sub's own event table.
    let event_name = match rt.machine.event_name_by_id(&event_id) {
        Some(n) => n.to_string(),
        None => return Ok(None),
    };
    // Find an active ref-state with a live sub-instance that declares this
    // event. Active-config order is deterministic.
    let target_ref: Option<String> = rt
        .active_states
        .iter()
        .find(|s| {
            rt.submachines
                .get(*s)
                .is_some_and(|sub| sub.machine.event_id_by_name(&event_name).is_some())
        })
        .cloned();
    let Some(ref_id) = target_ref else {
        return Ok(None);
    };

    let mut sub = rt
        .submachines
        .remove(&ref_id)
        .expect("target_ref came from submachines keys");
    let sub_config_before = sub.active_states.clone();
    let sub_event_id = sub
        .machine
        .event_id_by_name(&event_name)
        .expect("filter guaranteed the sub declares this event")
        .to_string();
    // Route the event into the sub. `Dispatched` vs `Raised` is preserved
    // so the sub's own queue-position semantics (Doc 08 §3.2) hold.
    let queued = match &event.kind {
        EventKind::Raised { .. } => sub.queue.push_front(QueuedEvent::new(
            EventKind::Raised {
                event_id: sub_event_id,
            },
            event.payload.clone(),
        )),
        _ => sub.queue.push_back(QueuedEvent::new(
            EventKind::Dispatched {
                event_id: sub_event_id,
            },
            event.payload.clone(),
        )),
    };
    if let Err(e) = queued {
        rt.submachines.insert(ref_id.clone(), sub);
        return Err(e.into());
    }
    // Run the sub to quiescence through the identical RTC core, one depth
    // deeper. This recursively syncs the sub's own nested submachines.
    let sub_records = drain_queue_rt(&mut sub, externs, depth + 1)?;
    let sub_config_after = sub.active_states.clone();
    rt.submachines.insert(ref_id.clone(), sub);

    // The delegation marker carries the sub config before/after — the
    // behaviourally load-bearing surface (assertions + W2d sim≡codegen
    // matching key on this).
    let marker = StepRecord {
        trace_id: rt.next_trace_id,
        kind: StepKind::SubmachineEventDelegated,
        virtual_clock_ms,
        event_received: super::step::event_received_for(rt, event),
        transition_taken: None,
        exited_states: vec![],
        entered_states: vec![],
        actions_executed: vec![],
        config_before: config_before.to_vec(),
        config_after: rt.active_states.clone(),
        submachine: Some(crate::trace::SubmachineRecord {
            ref_state_id: ref_id.clone(),
            sub_config_before,
            sub_config_after,
            delegated_event: Some(event_name),
        }),
    };
    rt.next_trace_id += 1;
    // Surface the marker followed by the sub-instance's own step records
    // (re-tagged with the ref-state, trace ids re-stamped from the
    // parent's monotonic counter). The combined stream stays gap-free and
    // byte-deterministic — W2d's gcc sim≡codegen matching walks exactly
    // this sequence.
    let mut out = Vec::with_capacity(1 + sub_records.len());
    out.push(marker);
    out.extend(reparent_sub_records(rt, sub_records, &ref_id));
    Ok(Some(out))
}

/// Default-populate a sub-instance's context exactly as `Interpreter::init`
/// does for a top-level machine (Doc 08 §2.2). Factored out so the
/// submachine entry path reuses the identical field-defaulting rule.
pub(super) fn init_sub_context(sub_rt: &mut RuntimeState) {
    for f in &sub_rt.machine.machine.context.fields.clone() {
        let v = match &f.default {
            Some(lit) => crate::eval::arith::literal_to_value(lit),
            None => match &f.ty {
                fsm_ir::Type::Primitive { name } => Value::default_for_primitive(name),
                fsm_ir::Type::Enum { .. } => Value::Enum(String::new(), String::new()),
                fsm_ir::Type::Opaque { .. } => Value::I32(0),
                fsm_ir::Type::Array { .. } => Value::I32(0),
            },
        };
        sub_rt.context.insert(f.name.clone(), v);
    }
}

/// Re-tag a sub-instance's own step records so the parent trace shows the
/// nested execution as belonging to `ref_id`'s sub-instance. Trace ids are
/// re-stamped from the parent's monotonic counter so the combined stream
/// stays gap-free and deterministic. Records that are already submachine
/// records (deeper nesting) keep their innermost `ref_state_id`.
pub(super) fn reparent_sub_records(
    rt: &mut RuntimeState,
    sub_records: Vec<StepRecord>,
    ref_id: &str,
) -> Vec<StepRecord> {
    let mut out = Vec::with_capacity(sub_records.len());
    for mut r in sub_records {
        r.trace_id = rt.next_trace_id;
        rt.next_trace_id += 1;
        if r.submachine.is_none() {
            r.submachine = Some(crate::trace::SubmachineRecord {
                ref_state_id: ref_id.to_string(),
                sub_config_before: r.config_before.clone(),
                sub_config_after: r.config_after.clone(),
                delegated_event: None,
            });
        }
        out.push(r);
    }
    out
}
