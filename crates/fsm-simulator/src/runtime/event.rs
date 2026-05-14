//! Internal event representation — what actually sits in the interpreter
//! queue. Doc 08 §3.2 + §14 enumerate the four flavours: dispatched (external
//! event from `M_dispatch`), raised (internal `raise`), timer (synthetic from
//! `M_tick`), and completion (synthetic when a `final` state is reached, Doc
//! 08 §9 / Doc 00 §7.6).

use std::collections::HashMap;

use super::value::Value;

/// One queued event. `payload` carries the actual payload field values; the
/// trace recorder dumps them under `eventReceived.payload` (Doc 13 §11).
#[derive(Clone, Debug)]
pub struct QueuedEvent {
    pub kind: EventKind,
    /// Map of payload-field name → value. `None` for completion / timer
    /// events that do not carry a payload.
    pub payload: Option<HashMap<String, Value>>,
}

/// Three concrete event kinds.
///
/// Timers are NOT a separate kind — when the virtual clock advances they are
/// resolved to `Dispatched { event_id }` so the existing transition-selection
/// logic on `Trigger::Event` handles them uniformly. (B-09 explicitly notes
/// that timers are "synthetic events".)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// External event — pushed by `Interpreter::dispatch`.
    Dispatched { event_id: String },
    /// `raise` statement — internal; inserted at the front of the queue per
    /// Doc 08 §3.2 and §14.
    Raised { event_id: String },
    /// Synthetic completion event — Doc 08 §9, Doc 00 §7.6. Fires the
    /// completion transition of `state_id` (or its enclosing composite for
    /// `final` leaves).
    Completion { state_id: String },
    /// Synthetic timer event. The transition that fires is identified by
    /// its IR id rather than an event name because timer triggers carry no
    /// event reference.
    TimerFire {
        timer_id: String,
        transition_id: String,
        source_state: String,
    },
}

impl EventKind {
    /// Event-name representation for `StepRecord.eventReceived.name`. Returns
    /// `None` for completion / timer events (they have no DSL-level name).
    pub fn event_id(&self) -> Option<&str> {
        match self {
            EventKind::Dispatched { event_id } | EventKind::Raised { event_id } => {
                Some(event_id.as_str())
            }
            EventKind::Completion { .. } | EventKind::TimerFire { .. } => None,
        }
    }
}
