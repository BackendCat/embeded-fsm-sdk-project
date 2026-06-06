//! RTC step orchestration — Doc 08 §3.1 (event processing), §3.2 (queue
//! drain policy), §4.1 (the per-step algorithm), §9.4 (completion-loop
//! cap), §14 (queue drain). The submachine sync points around each step
//! live in [`super::submachine`].

use crate::eval::ExternRegistry;
use crate::runtime::{check_and_enqueue_completion, EventKind, QueuedEvent, RuntimeState};
use crate::trace::{EventReceivedRecord, StepKind, StepRecord, TransitionTakenRecord};

use super::defer::{active_config_defers, release_deferred};
use super::submachine::{sync_submachines, try_delegate_to_submachine};
use super::transitions::{execute_one_transition, select_transitions};
use super::StepError;

/// Drain `rt`'s internal queue to quiescence, RTC-stepping each event and
/// keeping every sub-instance in sync afterward. Recursive: a sub-instance
/// reached via [`sync_submachines`] drains through this same function (one
/// `depth` deeper), so the submachine runtime *composes* with — never
/// re-implements — the defer / done-autofire / completion machinery.
pub(super) fn drain_queue_rt(
    rt: &mut RuntimeState,
    externs: &ExternRegistry,
    depth: u32,
) -> Result<Vec<StepRecord>, StepError> {
    let mut out = Vec::new();
    // After init / before the first dispatched event, freshly-entered
    // ref-states already need their sub-instances (Doc 08 §12.2). The
    // caller runs the entry sequence; this is the single sync point.
    out.extend(sync_submachines(rt, externs, depth)?);
    loop {
        let Some(ev) = rt.queue.pop_front() else {
            rt.completion_run = 0;
            break;
        };
        // Track completion-event runs (Doc 08 §9.4).
        if matches!(ev.kind, EventKind::Completion { .. }) {
            rt.completion_run += 1;
            if rt.completion_run > 100 {
                return Err(StepError::CompletionLoop(rt.completion_run));
            }
        } else {
            rt.completion_run = 0;
        }
        // A delegated event yields a marker + the sub-instance's own
        // re-tagged records; every other event yields exactly one record.
        out.extend(rtc_step(rt, externs, ev, depth)?);
        // A step may have entered or exited a ref-state (instantiate /
        // tear down its sub-instance) or progressed a sub to Final
        // (enqueue the parent `Completion` so the existing R1 path fires
        // `done ->`). Doc 08 §12.2 / §12.3.
        out.extend(sync_submachines(rt, externs, depth)?);
    }
    Ok(out)
}

/// Single RTC step (Doc 08 §3.1) over any `RuntimeState` (top-level or a
/// submachine sub-instance). `depth` is the live submachine nesting level
/// (0 at the top); it bounds delegation recursion. Returns a `Vec` because
/// a submachine-delegated event surfaces the delegation marker *plus* the
/// sub-instance's own nested step records (re-tagged); all other events
/// return a single-element `Vec`.
fn rtc_step(
    rt: &mut RuntimeState,
    externs: &ExternRegistry,
    event: QueuedEvent,
    depth: u32,
) -> Result<Vec<StepRecord>, StepError> {
    {
        let config_before = rt.active_states.clone();
        let virtual_clock_ms = rt.virtual_clock_ms;
        rt.current_payload = event.payload.clone();

        // 1) Transition selection — one per region, innermost-first walk.
        // A parent-level transition on a submachine-ref state (its
        // `SubmachineRef.transitions`, surfaced on the node like any
        // state's) is selected here and therefore *wins* over delegating
        // the event to the sub-instance — the established
        // transition-wins-over-defer ordering generalised to submachines
        // (Doc 08 §12.1, UML 2.5.1 §14.2.3.9.1).
        let selected = select_transitions(rt, &event, externs)?;
        if selected.is_empty() {
            // No parent transition consumed the event. Before defer /
            // discard, Doc 08 §12.1: if the active leaf is a
            // submachine-ref with a live sub-instance and the event
            // resolves in the sub's own event table, route it into the
            // sub-instance's RTC (run-to-completion *inside* the sub).
            if let Some(recs) = try_delegate_to_submachine(
                rt,
                externs,
                &event,
                depth,
                &config_before,
                virtual_clock_ms,
            )? {
                rt.current_payload = None;
                return Ok(recs);
            }
            // No transition consumed the event. Doc 08 §10.1: if the
            // event's id is in the defer set of any state in the active
            // configuration (the state or an active ancestor), it is HELD
            // rather than discarded. Transition-wins is already satisfied
            // because we only reach here after selection returned empty
            // (UML 2.5.1 §14.2.3.9.1). Completion / timer events have no
            // DSL-level event id and are never deferrable.
            let deferrable_event_id = match &event.kind {
                EventKind::Dispatched { event_id } | EventKind::Raised { event_id } => {
                    Some(event_id.clone())
                }
                EventKind::Completion { .. } | EventKind::TimerFire { .. } => None,
            };
            let is_deferred = deferrable_event_id
                .as_ref()
                .map(|eid| active_config_defers(rt, eid))
                .unwrap_or(false);

            rt.current_payload = None;
            if is_deferred {
                // Hold the event. FIFO order is preserved by appending to
                // `defer_set` (Doc 08 §10.3). The configuration is
                // unchanged: config_before == config_after.
                let event_id =
                    deferrable_event_id.expect("is_deferred implies a deferrable event id");
                rt.defer_set.push(event_id);
                let rec = StepRecord {
                    trace_id: rt.next_trace_id,
                    kind: StepKind::EventDeferred,
                    virtual_clock_ms,
                    event_received: event_received_for(rt, &event),
                    transition_taken: None,
                    exited_states: vec![],
                    entered_states: vec![],
                    actions_executed: vec![],
                    config_before,
                    config_after: rt.active_states.clone(),
                    submachine: None,
                };
                rt.next_trace_id += 1;
                return Ok(vec![rec]);
            }
            // Genuinely unconsumed and not deferred — discard per Doc 08
            // §3.1. A redispatched event that finds no transition AND is
            // no longer deferred (its deferring state already exited) is a
            // normal discard.
            let rec = StepRecord {
                trace_id: rt.next_trace_id,
                kind: kind_for_event(&event),
                virtual_clock_ms,
                event_received: event_received_for(rt, &event),
                transition_taken: None,
                exited_states: vec![],
                entered_states: vec![],
                actions_executed: vec![],
                config_before,
                config_after: rt.active_states.clone(),
                submachine: None,
            };
            rt.next_trace_id += 1;
            return Ok(vec![rec]);
        }

        // 2) Execute every selected transition. Doc 08 §6 / §7.
        let mut exited_all: Vec<String> = Vec::new();
        let mut entered_all: Vec<String> = Vec::new();
        let mut outcome = crate::eval::ExecOutcome::default();
        // Pre-pick the "primary" transition for trace recording (the first
        // selected one). Multi-transition selections occur only across
        // parallel regions and the trace can still show "the" transition by
        // pointing at the first.
        let primary = selected[0].clone();
        for t in selected {
            execute_one_transition(
                rt,
                &t,
                &mut exited_all,
                &mut entered_all,
                &mut outcome,
                externs,
            )?;
        }

        // 3) Release deferred events on every state we exited that no longer
        //    has an active descendant deferring it.
        release_deferred(rt)?;

        // 4) Check completion (Doc 08 §9, B-08).
        for s in entered_all.clone() {
            check_and_enqueue_completion(rt, &s)?;
        }

        rt.current_payload = None;
        let rec = StepRecord {
            trace_id: rt.next_trace_id,
            kind: kind_for_event(&event),
            virtual_clock_ms,
            event_received: event_received_for(rt, &event),
            transition_taken: Some(TransitionTakenRecord {
                stable_id: primary.stable_id.clone(),
                source: primary.source.clone(),
                target: primary.target.clone(),
            }),
            exited_states: exited_all,
            entered_states: entered_all,
            actions_executed: outcome.actions_executed,
            config_before,
            config_after: rt.active_states.clone(),
            submachine: None,
        };
        rt.next_trace_id += 1;
        Ok(vec![rec])
    }
}

pub(super) fn kind_for_event(ev: &QueuedEvent) -> StepKind {
    // A released deferred event reprocesses as `EventRedispatched` so the
    // trace shows the defer→release round-trip (Doc 08 §10.2). Only
    // dispatched/raised events can have been deferred; the flag is never
    // set on completion / timer events.
    if ev.redispatched {
        return StepKind::EventRedispatched;
    }
    match &ev.kind {
        EventKind::Dispatched { .. } => StepKind::Dispatched,
        EventKind::Raised { .. } => StepKind::Raised,
        EventKind::TimerFire { .. } => StepKind::TimerFired,
        EventKind::Completion { .. } => StepKind::Completion,
    }
}

pub(super) fn event_received_for(
    rt: &RuntimeState,
    ev: &QueuedEvent,
) -> Option<EventReceivedRecord> {
    match &ev.kind {
        EventKind::Dispatched { event_id } | EventKind::Raised { event_id } => {
            let name = rt
                .machine
                .event_name_by_id(event_id)
                .map(str::to_string)
                .unwrap_or_else(|| event_id.clone());
            let stable_id = rt
                .machine
                .events_by_id
                .get(event_id)
                .map(|e| e.stable_id.clone());
            Some(EventReceivedRecord {
                name,
                stable_id,
                payload: ev.payload.clone(),
            })
        }
        EventKind::Completion { state_id } => Some(EventReceivedRecord {
            name: format!("__completion__:{state_id}"),
            stable_id: None,
            payload: None,
        }),
        EventKind::TimerFire { timer_id, .. } => Some(EventReceivedRecord {
            name: format!("__timer__:{timer_id}"),
            stable_id: None,
            payload: None,
        }),
    }
}
