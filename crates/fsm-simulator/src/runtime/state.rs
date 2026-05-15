//! Mutable interpreter state — the part that changes between RTC steps.
//!
//! Held as a single owned struct on [`crate::Interpreter`] so callers can
//! `snapshot()` / `restore()` to replay traces without re-loading the IR
//! (the IR plus the [`MachineIndex`] is shared via `Arc`).

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::machine_index::MachineIndex;
use super::queue::EventQueue;
use super::timer::TimerSet;
use super::value::Value;

/// Map of context-field name → current [`Value`]. Held as a `BTreeMap` so
/// every iteration (and every serde serialization) emits keys in sorted
/// lexicographic order — the wire-format byte-exactness contract in
/// Doc 13 §11 depends on this. `HashMap` would randomise the order per
/// process via `RandomState` and break golden-trace replay.
pub type ContextValues = BTreeMap<String, Value>;

#[derive(Clone, Debug)]
pub struct RuntimeState {
    pub machine: Arc<MachineIndex>,
    /// Active leaf-state IDs. One per active region (Doc 08 §2.1 invariant
    /// 1). Order is region-declaration order within each parallel.
    pub active_states: Vec<String>,
    /// History pseudo-state ID → last active leaf path (Doc 08 §8).
    /// For shallow history: stores a single direct-child state ID.
    /// For deep history: stores the path leaf (the simulator reconstructs
    /// the deeper substates by replaying the configuration at exit).
    ///
    /// `BTreeMap` so serialised snapshots are byte-deterministic — see
    /// `ContextValues` rationale above.
    pub history: BTreeMap<String, Vec<String>>,
    /// Set of currently deferred events — Doc 08 §10. Keyed by event ID.
    /// Stored in insertion order so release respects FIFO.
    pub defer_set: Vec<String>,
    /// Event queue. Capacity / overflow come from `MachineObject.queue`.
    pub queue: EventQueue,
    /// Virtual clock — see Doc 08 §13.4. The wall-clock fallback is not
    /// implemented for v1.0 (per Doc 00 §10.3 HAL is not a sim concern).
    pub virtual_clock_ms: u64,
    /// Armed timer set. Cleared on `cancel_owned_by(state_id)` at exit.
    pub timers: TimerSet,
    /// Mutable context fields.
    pub context: ContextValues,
    /// Auto-incremented step counter; populates `StepRecord.traceId`.
    pub next_trace_id: u64,
    /// Last current event's payload, accessible to action statements via
    /// `payload.field` references. `None` between steps. `BTreeMap` for
    /// the same wire-format determinism reason as `context`.
    pub current_payload: Option<BTreeMap<String, Value>>,
    /// Set of completion events fired without an intervening external event.
    /// Doc 08 §9.4 caps consecutive completions at 100.
    pub completion_run: u32,
    /// Initialization flag — every method except [`super::super::Interpreter::init`]
    /// requires this to be true.
    pub initialized: bool,
    /// Live submachine sub-instances, keyed by the **referencing**
    /// `StateNode::Submachine` state id (e.g. `s-Device-Connecting`) — Doc
    /// 08 §12. A nested `RuntimeState` is value-owned here while the
    /// ref-state is the active leaf: entering the ref-state instantiates +
    /// inits one at the template's initial; exiting the ref-state drops it
    /// (deterministic teardown, no leak). `Box` keeps `RuntimeState`'s size
    /// flat under recursion. `BTreeMap` (not `HashMap`) so any snapshot /
    /// serialised form iterates keys in sorted order — the same wire-format
    /// determinism contract as `context` / `history`.
    pub submachines: BTreeMap<String, Box<RuntimeState>>,
}

impl RuntimeState {
    pub fn new(machine: Arc<MachineIndex>) -> Self {
        let queue_cfg = &machine.machine.queue;
        let queue = EventQueue::new(queue_cfg.capacity, queue_cfg.overflow_policy);
        Self {
            machine,
            active_states: Vec::new(),
            history: BTreeMap::new(),
            defer_set: Vec::new(),
            queue,
            virtual_clock_ms: 0,
            timers: TimerSet::new(),
            context: ContextValues::new(),
            next_trace_id: 0,
            current_payload: None,
            completion_run: 0,
            initialized: false,
            submachines: BTreeMap::new(),
        }
    }

    /// Returns true if `state_id` (or any of its descendants by region) is in
    /// the active configuration. Used to evaluate join readiness and to check
    /// whether transition sources are reachable.
    pub fn is_active(&self, state_id: &str) -> bool {
        if self.active_states.iter().any(|s| s == state_id) {
            return true;
        }
        // Active descendants count as the state being active.
        for active in &self.active_states {
            let ancestors = self.machine.ancestors(active);
            if ancestors.iter().any(|a| a == state_id) {
                return true;
            }
        }
        false
    }

    /// Set of currently active states **including** ancestors. Doc 08 §4
    /// transition selection walks innermost-first across the entire ancestor
    /// chain so callers need both leaves and their parents.
    pub fn active_with_ancestors(&self) -> HashSet<String> {
        let mut set = HashSet::new();
        for a in &self.active_states {
            for anc in self.machine.ancestors(a) {
                set.insert(anc);
            }
        }
        set
    }
}

/// Snapshot serialisable form — used by `Interpreter::snapshot` /
/// `Interpreter::restore` for replay support (Doc 13 §8, deferred WS layer,
/// but the snapshot API is generally useful).
///
/// `BTreeMap` (not `HashMap`) so JSON encoding is byte-deterministic across
/// runs — Doc 13 §11 wire-format contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterpreterSnapshot {
    pub active_states: Vec<String>,
    pub history: BTreeMap<String, Vec<String>>,
    pub defer_set: Vec<String>,
    pub virtual_clock_ms: u64,
    pub context: ContextValues,
    pub next_trace_id: u64,
    pub initialized: bool,
}
