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
use super::timer::{Timer, TimerSet};
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

    /// Recursively capture this runtime's submachine map into the
    /// serialisable [`SubmachineSnapshot`] form (v1.4-W2 losslessness). The
    /// recursion walks `submachines` (a `BTreeMap`, so deterministic key
    /// order) and, for each nested sub-instance, captures the same
    /// configuration-relevant fields the parent snapshot captures —
    /// including *its* `submachines` (the recursion). Transients
    /// (`queue`/`current_payload`/`completion_run`) are deliberately
    /// omitted, matching the parent-snapshot policy: the interpreter only
    /// snapshots quiescent configs.
    pub(crate) fn capture_submachines(&self) -> BTreeMap<String, SubmachineSnapshot> {
        self.submachines
            .iter()
            .map(|(ref_id, sub)| (ref_id.clone(), sub.capture_as_sub()))
            .collect()
    }

    /// Capture *this* runtime as a [`SubmachineSnapshot`] (used when this
    /// runtime is itself a nested sub-instance — the recursive step).
    fn capture_as_sub(&self) -> SubmachineSnapshot {
        SubmachineSnapshot {
            active_states: self.active_states.clone(),
            history: self.history.clone(),
            defer_set: self.defer_set.clone(),
            virtual_clock_ms: self.virtual_clock_ms,
            timers: self.timers.snapshot_sorted(),
            context: self.context.clone(),
            next_trace_id: self.next_trace_id,
            initialized: self.initialized,
            submachines: self.capture_submachines(),
        }
    }

    /// Rebuild this runtime's `submachines` map from a previously captured
    /// [`SubmachineSnapshot`] tree (the [`RuntimeState`] half of
    /// `Interpreter::restore`, v1.4-W2). For each captured sub-instance:
    /// reconstruct its skeleton via
    /// [`super::submachine::build_sub_runtime`] (the *same* path
    /// `sync_submachines` uses to instantiate one — the template is
    /// derivable from `self.machine` + the ref-state id), then overlay the
    /// snapshot's mutable fields and recurse into *its* nested
    /// `submachines`. A ref-state whose template no longer resolves (a
    /// degraded/partial IR — Doc 09 §1) is skipped, mirroring
    /// `sync_submachines`'s `else { continue }`.
    pub(crate) fn restore_submachines(&mut self, captured: BTreeMap<String, SubmachineSnapshot>) {
        let mut rebuilt: BTreeMap<String, Box<RuntimeState>> = BTreeMap::new();
        for (ref_id, sub_snap) in captured {
            let Some(mut sub_rt) = super::submachine::build_sub_runtime(&self.machine, &ref_id)
            else {
                // Unresolved ref (FSM-E0103 at analysis) — skip rather than
                // panic, exactly as `sync_submachines` degrades.
                continue;
            };
            sub_rt.overlay_from_sub(sub_snap);
            rebuilt.insert(ref_id, Box::new(sub_rt));
        }
        self.submachines = rebuilt;
    }

    /// Overlay a [`SubmachineSnapshot`]'s mutable config onto this freshly
    /// `build_sub_runtime`-built sub-instance, then recurse into its nested
    /// subs. `machine`/`queue`/`current_payload`/`completion_run` keep
    /// their freshly-built (empty/quiescent) values — the snapshot is taken
    /// at a quiescent boundary so this is behaviourally exact.
    fn overlay_from_sub(&mut self, snap: SubmachineSnapshot) {
        self.active_states = snap.active_states;
        self.history = snap.history;
        self.defer_set = snap.defer_set;
        self.virtual_clock_ms = snap.virtual_clock_ms;
        self.timers.restore_from(snap.timers);
        self.context = snap.context;
        self.next_trace_id = snap.next_trace_id;
        self.initialized = snap.initialized;
        self.restore_submachines(snap.submachines);
    }
}

/// Recursively-captured mutable state of one **submachine sub-instance**
/// (v1.4-W2 — closes the audit's D-2 / §1.3 snapshot-lossiness blocker).
///
/// A [`RuntimeState`]'s `submachines: BTreeMap<String, Box<RuntimeState>>`
/// holds the live nested sub-instance configs (Doc 08 §12). Before W2 the
/// [`InterpreterSnapshot`] dropped them, so the verifier's `ConfigDigest`
/// would conflate two reachable configs that differ *only* in a nested
/// sub-instance's active leaf (or its context / timers / history), prune
/// the second as already-visited, and could return a **false
/// `ProvenNoDeadlock`** (the cardinal verification sin). This type makes
/// the snapshot lossless: the same configuration-relevant fields as
/// [`InterpreterSnapshot`], **recursively** (a sub-instance can itself
/// embed sub-instances).
///
/// **Reconstruction note (why no `machine: Arc<MachineIndex>` field).** A
/// sub-instance's `MachineIndex` template is *deterministically derivable*
/// from the parent index + the referencing ref-state id (the
/// `submachines` `BTreeMap` key) via
/// [`super::submachine::build_sub_runtime`] — exactly how
/// `sync_submachines` instantiates one. So the snapshot stores **only the
/// mutable config**; `Interpreter::restore` rebuilds each sub's skeleton
/// from its key and overlays this. This keeps the snapshot serialisable
/// (`Arc<MachineIndex>` is not, nor should it be in a wire form) and
/// byte-deterministic, and matches the existing parent-snapshot policy of
/// omitting the shared, reconstructable index.
///
/// **Transient-field policy (mirrors the parent snapshot exactly).** The
/// per-RTC-step transients (`queue`, `current_payload`, `completion_run`)
/// are **not** captured — identical to [`InterpreterSnapshot`] for the
/// parent. The interpreter only ever hands back configurations drained to
/// quiescence (Doc 08 §3.2 / §9.2 / §14), so these are empty/0 at every
/// snapshot/restore boundary; capturing them would add non-configuration
/// bytes to the digest and risk splitting behaviourally-identical configs.
/// `timers` is serialised as a **sorted** `Vec<Timer>` (see
/// [`super::timer::TimerSet::snapshot_sorted`]) so the armed *set* — not
/// the arm order — is what the digest keys.
///
/// `BTreeMap` (not `HashMap`) throughout so JSON encoding is
/// byte-deterministic across runs — the Doc 13 §11 wire-format contract,
/// here *extended* (not broken) to the recursive sub-instance + timers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmachineSnapshot {
    pub active_states: Vec<String>,
    pub history: BTreeMap<String, Vec<String>>,
    pub defer_set: Vec<String>,
    pub virtual_clock_ms: u64,
    /// Sorted armed-timer set of this sub-instance (canonical form).
    pub timers: Vec<Timer>,
    pub context: ContextValues,
    pub next_trace_id: u64,
    pub initialized: bool,
    /// Nested sub-instances of *this* sub-instance — the recursion. Keyed
    /// by the (this-template-relative) referencing ref-state id, same as
    /// [`RuntimeState::submachines`].
    pub submachines: BTreeMap<String, SubmachineSnapshot>,
}

/// Snapshot serialisable form — used by `Interpreter::snapshot` /
/// `Interpreter::restore` for replay support (Doc 13 §8, deferred WS layer,
/// but the snapshot API is generally useful).
///
/// `BTreeMap` (not `HashMap`) so JSON encoding is byte-deterministic across
/// runs — Doc 13 §11 wire-format contract.
///
/// **v1.4-W2 losslessness extension (additive — closes audit D-2).** Prior
/// to W2 this captured only `active_states`/`history`/`defer_set`/
/// `virtual_clock_ms`/`context`/`next_trace_id`/`initialized`, silently
/// dropping `RuntimeState.timers` and `RuntimeState.submachines`. Two new
/// fields — `timers` (the armed set, canonical sorted form) and
/// `submachines` (the recursive nested sub-instance configs) — make the
/// snapshot **lossless**: a `snapshot → restore → re-snapshot` round-trip
/// is now byte-identical for hierarchical/parallel/timer/submachine configs
/// as well as flat ones. The new fields are **purely additive** — every
/// pre-W2 field keeps its name, type, and position, so an existing flat
/// snapshot serialises byte-identically *to its old form for those fields*
/// (the two new fields encode as an empty `[]` / `{}` when no timers /
/// submachines are armed). The Doc 13 §11 byte-stability contract is
/// thereby extended, not broken.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterpreterSnapshot {
    pub active_states: Vec<String>,
    pub history: BTreeMap<String, Vec<String>>,
    pub defer_set: Vec<String>,
    pub virtual_clock_ms: u64,
    /// Armed-timer set, canonical sorted form (v1.4-W2). The armed timers
    /// *are* configuration: a state armed-waiting on a timer is
    /// behaviourally distinct from the same state with no timer (a
    /// timer-fire edge can make progress from the former). Empty `[]` when
    /// nothing is armed ⇒ flat-machine snapshots are byte-unchanged.
    pub timers: Vec<Timer>,
    pub context: ContextValues,
    pub next_trace_id: u64,
    pub initialized: bool,
    /// Live submachine sub-instances, recursively captured (v1.4-W2).
    /// Keyed by the referencing `StateNode::Submachine` ref-state id, same
    /// as [`RuntimeState::submachines`]. Empty `{}` for any machine with no
    /// active submachine ⇒ flat-machine snapshots are byte-unchanged.
    pub submachines: BTreeMap<String, SubmachineSnapshot>,
}
