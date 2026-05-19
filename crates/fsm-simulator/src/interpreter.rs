//! The central RTC step interpreter.
//!
//! Implements Doc 08 (FSM-SPEC-SEM) faithfully — see the section references
//! sprinkled through the code:
//! - §3.1 / §4 — event processing + transition selection
//! - §5 + B-09 — effective LCA for self-transitions
//! - §6 — exit sequence (innermost-first, parallel regions reverse)
//! - §7 — entry sequence (outermost-first, expand initial substates)
//! - §8 — history (shallow / deep, default fallback)
//! - §9 + B-08 — completion events (parallel = all-regions-final)
//! - §10 — deferred events (released on exit from deferring state)
//! - §13 — timer lifecycle
//! - §14 — queue drain policy
//!
//! The two architectural invariants that make this tractable:
//! 1. **Collect then execute** (B-11). Every parallel region gets a chance
//!    to select a transition; we run them all in region-declaration order.
//! 2. **Re-arm timers on entry, cancel on exit.** Pure book-keeping; no
//!    cross-cutting timer-state required.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use fsm_ir::{HistoryKind, Ir, TimerKind, TransitionKind, TransitionObject, Trigger};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::eval::{
    eval_guard, execute_statements, EvalCtx, EvalError, ExecOutcome, ExternRegistry, StmtCtx,
    StmtError,
};
use crate::runtime::{
    check_and_enqueue_completion, effective_lca, EventKind, InterpreterSnapshot, MachineIndex,
    NodeKind, QueueError, QueuedEvent, RuntimeState, Timer, TimerFire, TimerSet, Value,
};
use crate::trace::{EventReceivedRecord, StepKind, StepRecord, TransitionTakenRecord};

/// Maximum ancestor-walk depth before we treat the parent table as cyclic
/// and bail out with an internal error. UML statecharts in practice nest
/// ≤15 levels — see Doc 08 §2.1 (no normative cap, but every realistic
/// model is far shallower). 256 covers the wildest legitimate nest and
/// still detects malformed-IR cycles before they blow the stack.
const MAX_STATE_DEPTH: u32 = 256;

/// Options for [`Interpreter::init`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitOptions {
    pub machine_name: String,
    #[serde(default)]
    pub initial_context: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    pub virtual_clock_start_ms: u64,
}

#[derive(Debug, Error)]
pub enum StepError {
    #[error("not initialized — call Interpreter::init first")]
    NotInitialized,
    #[error("already initialized")]
    AlreadyInitialized,
    #[error("unknown machine name: {0}")]
    UnknownMachine(String),
    #[error("ir contains no machines")]
    NoMachines,
    #[error("invalid event: {name}")]
    InvalidEvent { name: String },
    #[error("queue overflow: {0}")]
    QueueOverflow(#[from] QueueError),
    #[error("eval: {0}")]
    Eval(#[from] EvalError),
    /// Guard expression failed at runtime (overflow, type mismatch, missing
    /// extern, etc). Distinguished from `Eval` (which covers action-language
    /// failures) so callers can report "transition guard errored" cleanly.
    /// Prior audit P1-7 / Audit D §"Anti-patterns" 1: pre-fix sites silently
    /// downgraded guard errors to `false`, masking real defects.
    #[error("guard evaluation: {0}")]
    GuardEval(EvalError),
    #[error("stmt: {0}")]
    Stmt(#[from] StmtError),
    #[error(
        "completion loop detected — more than {0} consecutive completion events without an external event"
    )]
    CompletionLoop(u32),
    #[error("internal: {0}")]
    Internal(String),
}

/// The interpreter — owns an `Arc<MachineIndex>` and a mutable
/// [`RuntimeState`]. Cloning an interpreter is cheap (everything shareable
/// is in `Arc`s) but the `RuntimeState` is cloned by value.
#[derive(Debug)]
pub struct Interpreter {
    indexes: Vec<Arc<MachineIndex>>,
    runtime: Option<RuntimeState>,
    externs: ExternRegistry,
}

impl Interpreter {
    /// Build an interpreter over the IR. Indexes every machine so that
    /// `init()` can pick by name. Returns `Err` if the IR has zero machines.
    pub fn new(ir: &Ir) -> Result<Self, StepError> {
        if ir.machines.is_empty() {
            return Err(StepError::NoMachines);
        }
        let indexes = ir
            .machines
            .iter()
            .map(|m| Arc::new(MachineIndex::build(Arc::new(m.clone()))))
            .collect();
        Ok(Self {
            indexes,
            runtime: None,
            externs: ExternRegistry::new(),
        })
    }

    /// Mutable access to the extern registry for host-side registration.
    pub fn externs_mut(&mut self) -> &mut ExternRegistry {
        &mut self.externs
    }

    pub fn externs(&self) -> &ExternRegistry {
        &self.externs
    }

    /// Initialize the machine. Doc 08 §2.2:
    /// `C = ∅; execute_entry_sequence(root.initial_target, ctx)`.
    pub fn init(&mut self, opts: InitOptions) -> Result<Vec<StepRecord>, StepError> {
        if self
            .runtime
            .as_ref()
            .map(|r| r.initialized)
            .unwrap_or(false)
        {
            return Err(StepError::AlreadyInitialized);
        }
        let idx = self
            .indexes
            .iter()
            .find(|m| m.machine.name == opts.machine_name)
            .cloned()
            .ok_or_else(|| StepError::UnknownMachine(opts.machine_name.clone()))?;
        let mut rt = RuntimeState::new(idx);
        rt.virtual_clock_ms = opts.virtual_clock_start_ms;
        // Default-populate context.
        for f in &rt.machine.machine.context.fields {
            let v = match &f.default {
                Some(lit) => crate::eval::arith::literal_to_value(lit),
                None => match &f.ty {
                    fsm_ir::Type::Primitive { name } => Value::default_for_primitive(name),
                    fsm_ir::Type::Enum { .. } => Value::Enum(String::new(), String::new()),
                    fsm_ir::Type::Opaque { .. } => Value::I32(0),
                    fsm_ir::Type::Array { .. } => Value::I32(0),
                },
            };
            rt.context.insert(f.name.clone(), v);
        }
        if let Some(extra) = opts.initial_context {
            for (k, v) in extra {
                rt.context.insert(k, v);
            }
        }
        rt.initialized = true;

        // Entry sequence: dive from root's initial pseudo-state to the
        // leaf-most basic state and run every entry action along the way.
        let mut outcome = ExecOutcome::default();
        let config_before: Vec<String> = Vec::new();
        let mut entered = Vec::new();
        let root_id = rt.machine.root_region_id.clone();
        let initial = rt
            .machine
            .region(&root_id)
            .map(|r| r.initial_pseudo.clone())
            .ok_or_else(|| StepError::Internal("root region missing".into()))?;
        let initial_target = match &rt.machine.node(&initial).map(|n| &n.kind) {
            Some(NodeKind::Initial { target }) => target.clone(),
            _ => return Err(StepError::Internal("root initial is not Initial".into())),
        };
        enter_state_path(
            &mut rt,
            &root_id,
            &initial_target,
            &mut entered,
            &mut outcome,
            &self.externs,
        )?;

        // Doc 08 §2.2 / §9.1: after the initial entry sequence, run the
        // completion check so any `done -> X` on initially-entered states
        // auto-fires before external dispatching begins. Without this,
        // machines whose root-initial state carries `done` would idle until
        // the first external event.
        for s in entered.clone() {
            check_and_enqueue_completion(&mut rt, &s)?;
        }

        let rec = StepRecord {
            trace_id: rt.next_trace_id,
            kind: StepKind::Init,
            virtual_clock_ms: rt.virtual_clock_ms,
            event_received: None,
            transition_taken: None,
            exited_states: vec![],
            entered_states: entered,
            actions_executed: outcome.actions_executed,
            config_before,
            config_after: rt.active_states.clone(),
            submachine: None,
        };
        rt.next_trace_id += 1;
        let mut out = vec![rec];

        self.runtime = Some(rt);

        // Run any completion events / internal raises queued during entry
        // actions to completion (Doc 08 §3.2). The first thing
        // `drain_queue_rt` does is `sync_submachines`, so a root-initial
        // configuration that lands directly on a `StateNode::Submachine`
        // ref-state instantiates its sub-instance here (Doc 08 §12.2)
        // before any external dispatch.
        out.extend(self.drain_internal_queue()?);
        Ok(out)
    }

    /// Dispatch an external event by name.
    pub fn dispatch(&mut self, event_name: &str) -> Result<Vec<StepRecord>, StepError> {
        self.dispatch_with_payload(event_name, None)
    }

    pub fn dispatch_with_payload(
        &mut self,
        event_name: &str,
        payload: Option<BTreeMap<String, Value>>,
    ) -> Result<Vec<StepRecord>, StepError> {
        let rt = self.runtime.as_mut().ok_or(StepError::NotInitialized)?;
        let event_id = rt
            .machine
            .event_id_by_name(event_name)
            .ok_or_else(|| StepError::InvalidEvent {
                name: event_name.to_string(),
            })?
            .to_string();
        // Doc 08 §10.1 + UML 2.5.1 §14.2.3.9.1: deferral is decided *during*
        // the RTC step, AFTER transition selection — an enabled transition
        // wins over `defer` (transition-wins). The pre-v1.1 simulator
        // deferred here (before the queue), which discarded the event with
        // no trace record AND let `defer` shadow a consuming transition.
        // The event now enters the queue normally; `run_step` holds it in
        // `defer_set` only if no transition consumes it (Doc 08 §10).
        rt.queue.push_back(QueuedEvent::new(
            EventKind::Dispatched { event_id },
            payload,
        ))?;
        self.drain_internal_queue()
    }

    /// Process an internal `raise EVENT` from the host side. Identical to
    /// `dispatch` except the event goes to the FRONT of the queue.
    pub fn raise(&mut self, event_name: &str) -> Result<Vec<StepRecord>, StepError> {
        self.raise_with_payload(event_name, None)
    }

    pub fn raise_with_payload(
        &mut self,
        event_name: &str,
        payload: Option<BTreeMap<String, Value>>,
    ) -> Result<Vec<StepRecord>, StepError> {
        let rt = self.runtime.as_mut().ok_or(StepError::NotInitialized)?;
        let event_id = rt
            .machine
            .event_id_by_name(event_name)
            .ok_or_else(|| StepError::InvalidEvent {
                name: event_name.to_string(),
            })?
            .to_string();
        rt.queue
            .push_front(QueuedEvent::new(EventKind::Raised { event_id }, payload))?;
        self.drain_internal_queue()
    }

    /// Advance the virtual clock and fire any timers that would have expired
    /// in the interval.
    pub fn advance_clock(&mut self, delta_ms: u64) -> Result<Vec<StepRecord>, StepError> {
        let mut out = Vec::new();
        let target = {
            let rt = self.runtime.as_ref().ok_or(StepError::NotInitialized)?;
            rt.virtual_clock_ms.saturating_add(delta_ms)
        };
        loop {
            // Walk the timer set in chronological order. Each fire enqueues
            // a synthetic event and we then drain the queue to a quiescent
            // state — this lets entry actions on the new state arm new
            // timers before we look at the rest of the elapsed interval.
            let next_expiry = {
                let rt = self.runtime.as_ref().ok_or(StepError::NotInitialized)?;
                rt.timers.next_expiry_ms()
            };
            let next = match next_expiry {
                Some(t) if t <= target => t,
                _ => break,
            };
            {
                let rt = self.runtime.as_mut().ok_or(StepError::NotInitialized)?;
                if next > rt.virtual_clock_ms {
                    rt.virtual_clock_ms = next;
                }
                let now = rt.virtual_clock_ms;
                let fired = rt.timers.pop_fired_through(now);
                for t in fired {
                    rt.queue.push_back(QueuedEvent::new(
                        EventKind::TimerFire {
                            timer_id: t.timer_id.clone(),
                            transition_id: t.transition_id.clone().unwrap_or_default(),
                            source_state: t.source_state.clone(),
                        },
                        None,
                    ))?;
                }
            }
            out.extend(self.drain_internal_queue()?);
        }
        // Finally, advance virtual clock to target.
        let rt = self.runtime.as_mut().ok_or(StepError::NotInitialized)?;
        if target > rt.virtual_clock_ms {
            rt.virtual_clock_ms = target;
        }
        Ok(out)
    }

    /// IDs of every state currently active.
    pub fn current_states(&self) -> Vec<String> {
        self.runtime
            .as_ref()
            .map(|r| r.active_states.clone())
            .unwrap_or_default()
    }

    /// Active configuration of the live submachine sub-instance owned by
    /// the given `StateNode::Submachine` ref-state (Doc 08 §12). `None`
    /// when no sub-instance exists for that ref-state — either the
    /// ref-state is not active (so it was torn down / never instantiated)
    /// or the id is not a submachine ref. Introspection for embedders /
    /// tests; the parent's own `current_states()` reports the ref-state
    /// itself (a leaf in the parent configuration).
    pub fn submachine_active_states(&self, ref_state_id: &str) -> Option<Vec<String>> {
        self.runtime
            .as_ref()?
            .submachines
            .get(ref_state_id)
            .map(|sub| sub.active_states.clone())
    }

    /// Human-readable dot-paths of currently active states, including every
    /// composite ancestor (Doc 13 §11 `ActiveConfiguration.activeStates`).
    pub fn current_states_named(&self) -> Vec<String> {
        let Some(rt) = self.runtime.as_ref() else {
            return vec![];
        };
        let mut names: HashSet<String> = HashSet::new();
        let mut ordered: Vec<String> = Vec::new();
        for leaf in &rt.active_states {
            for anc in rt.machine.ancestors(leaf).into_iter().rev() {
                if anc == rt.machine.root_region_id {
                    continue;
                }
                let Some(node) = rt.machine.node(&anc) else {
                    continue;
                };
                // Region nodes are not states, skip those.
                if !rt.machine.nodes.contains_key(&anc) {
                    continue;
                }
                if matches!(
                    node.kind,
                    NodeKind::Initial { .. }
                        | NodeKind::History { .. }
                        | NodeKind::EntryPoint
                        | NodeKind::ExitPoint
                ) {
                    continue;
                }
                let qname = format!("{}.{}", rt.machine.machine.name, dot_path(rt, &anc));
                if names.insert(qname.clone()) {
                    ordered.push(qname);
                }
            }
        }
        ordered
    }

    /// Borrow of the live context map. Errors with [`StepError::NotInitialized`]
    /// if [`Interpreter::init`] has not been called yet — prior audit D P1-A
    /// (the three `expect("not initialized")` sites in this file). Embedders
    /// reading state before init now get a typed error instead of a panic.
    pub fn context(&self) -> Result<&BTreeMap<String, Value>, StepError> {
        self.runtime
            .as_ref()
            .map(|r| &r.context)
            .ok_or(StepError::NotInitialized)
    }

    /// Force-set a single context field **without running any step** — the
    /// debug-interface `sim/setContext` backing (DBGUX §3.3; the SINGLE
    /// sanctioned additive `crates/fsm-simulator` delta of the debug epic).
    ///
    /// This is **not** a transition, a guard evaluation, an RTC step, or a
    /// completion synthesis: it is the *same nature* as
    /// [`InitOptions::initial_context`], which already pokes the live context
    /// map at `init` (the `rt.context.insert` in [`Interpreter::init`]) —
    /// here a single key is overwritten *after* init for interactive
    /// test-setup. It therefore emits **zero** [`StepRecord`] (the return
    /// type carries no record by construction) and the active configuration,
    /// timer set, defer buffer, history and virtual clock are untouched. The
    /// KEYSTONE-IN-DEBUG invariant (Doc 33 §2) is preserved: no second
    /// transition/guard/step semantics — a guarded raw field write that runs
    /// *nothing* sets state, it does not implement semantics.
    ///
    /// Errors with [`StepError::NotInitialized`] if [`Interpreter::init`] has
    /// not run (same guard as [`Interpreter::context`] — no live runtime, no
    /// context to write).
    pub fn set_context_field(&mut self, name: &str, value: Value) -> Result<(), StepError> {
        let rt = self.runtime.as_mut().ok_or(StepError::NotInitialized)?;
        rt.context.insert(name.to_owned(), value);
        Ok(())
    }

    pub fn virtual_clock_ms(&self) -> u64 {
        self.runtime
            .as_ref()
            .map(|r| r.virtual_clock_ms)
            .unwrap_or(0)
    }

    /// Snapshot the runtime for replay / time-travel. Errors with
    /// [`StepError::NotInitialized`] if the interpreter has not been initialised.
    /// Pre-fix this site panicked — embedders calling `snapshot()` before
    /// `init()` aborted the process.
    pub fn snapshot(&self) -> Result<InterpreterSnapshot, StepError> {
        let rt = self.runtime.as_ref().ok_or(StepError::NotInitialized)?;
        Ok(InterpreterSnapshot {
            active_states: rt.active_states.clone(),
            history: rt.history.clone(),
            defer_set: rt.defer_set.clone(),
            virtual_clock_ms: rt.virtual_clock_ms,
            // v1.4-W2 losslessness (audit D-2): the armed timer set and the
            // recursive submachine sub-instance configs are *configuration*
            // — capturing them makes `snapshot → restore → re-snapshot`
            // byte-identical for timer/submachine machines, so the
            // verifier's `ConfigDigest` can no longer conflate
            // behaviourally-distinct configs. `snapshot_sorted()` /
            // `capture_submachines()` produce the canonical (sorted /
            // BTreeMap) forms ⇒ byte-determinism preserved (Doc 13 §11,
            // extended not broken). Flat machines have an empty timer set +
            // empty submachine map ⇒ their snapshots are byte-unchanged.
            timers: rt.timers.snapshot_sorted(),
            context: rt.context.clone(),
            next_trace_id: rt.next_trace_id,
            initialized: rt.initialized,
            submachines: rt.capture_submachines(),
        })
    }

    /// Restore from a previously captured snapshot. Errors with
    /// [`StepError::NotInitialized`] if the interpreter has not been initialised
    /// (call `init` first to instantiate the runtime; `restore` then replaces
    /// the runtime's mutable state with the snapshot).
    pub fn restore(&mut self, snap: InterpreterSnapshot) -> Result<(), StepError> {
        let rt = self.runtime.as_mut().ok_or(StepError::NotInitialized)?;
        rt.active_states = snap.active_states;
        rt.history = snap.history;
        rt.defer_set = snap.defer_set;
        rt.virtual_clock_ms = snap.virtual_clock_ms;
        // v1.4-W2: restore the armed timer set and recursively rebuild the
        // submachine sub-instances. `restore_submachines` reconstructs each
        // sub's `MachineIndex` skeleton from `rt.machine` + the ref-state
        // id (the same `build_sub_runtime` path `sync_submachines` uses)
        // then overlays the captured mutable config — so a config restored
        // mid-sub-instance resurrects the *exact* nested state, not a
        // silently-torn-down one (the pre-W2 lossy-restore bug the audit
        // flagged).
        rt.timers.restore_from(snap.timers);
        rt.context = snap.context;
        rt.next_trace_id = snap.next_trace_id;
        rt.initialized = snap.initialized;
        rt.restore_submachines(snap.submachines);
        Ok(())
    }

    /// Drain the internal queue, processing each event through a single RTC
    /// step until the queue is empty. Thin wrapper over the recursive
    /// free-function core ([`drain_queue_rt`]) so a submachine sub-instance
    /// drains its own queue through the *identical* algorithm (Doc 08 §12 —
    /// no duplicated RTC).
    fn drain_internal_queue(&mut self) -> Result<Vec<StepRecord>, StepError> {
        let rt = self.runtime.as_mut().ok_or(StepError::NotInitialized)?;
        drain_queue_rt(rt, &self.externs, 0)
    }
}

/// Drain `rt`'s internal queue to quiescence, RTC-stepping each event and
/// keeping every sub-instance in sync afterward. Recursive: a sub-instance
/// reached via [`sync_submachines`] drains through this same function (one
/// `depth` deeper), so the submachine runtime *composes* with — never
/// re-implements — the defer / done-autofire / completion machinery.
fn drain_queue_rt(
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
        let mut outcome = ExecOutcome::default();
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

// ---------------------------------------------------------------------------
// Submachine sub-instance lifecycle wiring (Doc 08 §12).
//
// These compose the parent RTC with the nested sub-`RuntimeState`s without
// duplicating defer / done-autofire / completion: instantiation reuses the
// same entry-sequence routine as `init`; completion only *enqueues the
// parent `Completion`* and lets the existing R1 path fire `done ->`;
// teardown is a deterministic drop of the nested runtime.
// ---------------------------------------------------------------------------

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
fn sync_submachines(
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
fn try_delegate_to_submachine(
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
        event_received: event_received_for(rt, event),
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
fn init_sub_context(sub_rt: &mut RuntimeState) {
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
fn reparent_sub_records(
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

// ---------------------------------------------------------------------------
// Step helpers — these own the algorithm details.
// ---------------------------------------------------------------------------

fn kind_for_event(ev: &QueuedEvent) -> StepKind {
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

fn event_received_for(rt: &RuntimeState, ev: &QueuedEvent) -> Option<EventReceivedRecord> {
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

/// Doc 08 §4.1 — select transitions, innermost-first, one per region.
fn select_transitions(
    rt: &RuntimeState,
    event: &QueuedEvent,
    externs: &ExternRegistry,
) -> Result<Vec<TransitionObject>, StepError> {
    let mut selected: Vec<TransitionObject> = Vec::new();
    let mut done: HashSet<String> = HashSet::new();

    // For timer fires, the transition is already chosen by id — find and
    // return it directly. (Skip the entire walk.)
    if let EventKind::TimerFire {
        transition_id,
        source_state,
        ..
    } = &event.kind
    {
        if let Some(node) = rt.machine.node(source_state) {
            if let Some(t) = node.transitions.iter().find(|t| &t.id == transition_id) {
                // Guard still must pass (timer transitions may have guards).
                // No guard ⇒ fire unconditionally; Ok(true) ⇒ fire; Ok(false)
                // ⇒ skip; Err ⇒ propagate as `StepError::GuardEval` (was
                // silently `false` pre-fix — audit P1-7 / D anti-pattern 1).
                let fire = match &t.guard {
                    None => true,
                    Some(g) => {
                        let evctx = EvalCtx {
                            context: &rt.context,
                            payload: rt.current_payload.as_ref(),
                            externs,
                        };
                        match eval_guard(g, &evctx) {
                            Ok(b) => b,
                            Err(e) => return Err(StepError::GuardEval(e)),
                        }
                    }
                };
                if fire {
                    selected.push(t.clone());
                }
            }
        }
        return Ok(selected);
    }

    // For Completion events, the trigger is `Completion { from }` — match
    // transitions on the just-completed state's ancestors.
    let event_target_id: Option<String> = match &event.kind {
        EventKind::Dispatched { event_id } | EventKind::Raised { event_id } => {
            Some(event_id.clone())
        }
        EventKind::Completion { .. } | EventKind::TimerFire { .. } => None,
    };
    let completion_from: Option<String> = match &event.kind {
        EventKind::Completion { state_id } => Some(state_id.clone()),
        _ => None,
    };

    // For each active leaf, walk ancestors innermost-first.
    let actives = rt.active_states.clone();
    for s in actives {
        // Skip if this leaf or any ancestor is already covered by a
        // previously-selected transition's exit set (B-11 collect phase —
        // one transition per region; parallel-state-level transitions cover
        // every region simultaneously).
        let ancestors_s = rt.machine.ancestors(&s);
        if ancestors_s.iter().any(|a| done.contains(a)) {
            continue;
        }
        let ancestors = ancestors_s;
        let mut chose: Option<TransitionObject> = None;
        for anc in &ancestors {
            let Some(node) = rt.machine.node(anc) else {
                continue;
            };
            // Collect candidate transitions whose trigger matches.
            let mut candidates: Vec<&TransitionObject> = Vec::new();
            for t in &node.transitions {
                if let Some(trig) = &t.trigger {
                    let matches_trigger = match (trig, &event_target_id, &completion_from) {
                        (Trigger::Event { event_id, .. }, Some(eid), _) => event_id == eid,
                        (Trigger::Completion { from }, _, Some(cid)) => {
                            from == cid || {
                                // Allow completion from any descendant of `from`.
                                let cid_ancestors = rt.machine.ancestors(cid);
                                cid_ancestors.iter().any(|a| a == from)
                            }
                        }
                        _ => false,
                    };
                    if !matches_trigger {
                        continue;
                    }
                } else {
                    // No trigger = completion transition. Doc 08 §4.4.
                    let Some(cid) = &completion_from else {
                        continue;
                    };
                    // Fire only when the source state matches the completed
                    // state (or its ancestor).
                    if &t.source != cid && {
                        let cid_anc = rt.machine.ancestors(cid);
                        !cid_anc.iter().any(|a| a == &t.source)
                    } {
                        continue;
                    }
                }
                if let Some(g) = &t.guard {
                    let evctx = EvalCtx {
                        context: &rt.context,
                        payload: rt.current_payload.as_ref(),
                        externs,
                    };
                    // Ok(true) ⇒ candidate; Ok(false) ⇒ skip; Err ⇒ propagate
                    // (was silently `false` pre-fix — audit P1-7).
                    match eval_guard(g, &evctx) {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(e) => return Err(StepError::GuardEval(e)),
                    }
                }
                candidates.push(t);
            }
            if candidates.is_empty() {
                continue;
            }
            // Choose by (priority asc, document order asc).
            candidates.sort_by(|a, b| {
                a.priority
                    .cmp(&b.priority)
                    .then_with(|| std::cmp::Ordering::Equal)
            });
            chose = Some(candidates[0].clone());
            break;
        }
        if let Some(t) = chose {
            // Compute the exit set so other active leaves in the same region
            // don't double-select. For internal transitions, the exit set is
            // empty.
            if !matches!(t.kind, TransitionKind::Internal | TransitionKind::Local) {
                let lca = effective_lca(&t, &rt.machine);
                for ext in exit_set(&rt.machine, &t.source, &lca)? {
                    done.insert(ext);
                }
            }
            done.insert(t.source.clone());
            selected.push(t);
        }
    }
    Ok(selected)
}

/// Compute exit set: every state from `source` walking up the parent chain
/// until we reach (but do not include) `lca`. Innermost-first ordering.
///
/// Defence-in-depth against malformed IR with a cyclic parent table — at
/// most [`MAX_STATE_DEPTH`] iterations before we bail out with
/// `StepError::Internal`. UML statecharts in practice nest ≤15 levels.
fn exit_set(idx: &MachineIndex, source: &str, lca: &str) -> Result<Vec<String>, StepError> {
    let mut out = Vec::new();
    let mut cur = source.to_string();
    let mut guard: u32 = 0;
    while cur != lca {
        if guard >= MAX_STATE_DEPTH {
            return Err(StepError::Internal(
                "cycle in parent table — malformed IR".into(),
            ));
        }
        guard += 1;
        // For exit_set we only emit STATE ids, not regions.
        if idx.nodes.contains_key(&cur) {
            out.push(cur.clone());
        }
        let next = match idx.nodes.get(&cur) {
            Some(node) => node.parent_region.clone(),
            None => idx.regions.get(&cur).and_then(|r| r.parent_state.clone()),
        };
        match next {
            Some(p) => cur = p,
            None => break,
        }
    }
    Ok(out)
}

/// Walk ancestors of `target` from outer to inner, stopping at `lca`. Yields
/// state IDs only (no regions), outermost-first.
///
/// Same `MAX_STATE_DEPTH` cycle-detection contract as [`exit_set`].
fn entry_path(idx: &MachineIndex, lca: &str, target: &str) -> Result<Vec<String>, StepError> {
    let mut path: Vec<String> = Vec::new();
    let mut cur = target.to_string();
    let mut guard: u32 = 0;
    while cur != lca {
        if guard >= MAX_STATE_DEPTH {
            return Err(StepError::Internal(
                "cycle in parent table — malformed IR".into(),
            ));
        }
        guard += 1;
        if idx.nodes.contains_key(&cur) {
            path.push(cur.clone());
        }
        let next = match idx.nodes.get(&cur) {
            Some(node) => node.parent_region.clone(),
            None => idx.regions.get(&cur).and_then(|r| r.parent_state.clone()),
        };
        match next {
            Some(p) => cur = p,
            None => break,
        }
    }
    path.reverse();
    Ok(path)
}

/// Execute one transition's full exit / action / entry sequence.
fn execute_one_transition(
    rt: &mut RuntimeState,
    t: &TransitionObject,
    exited_all: &mut Vec<String>,
    entered_all: &mut Vec<String>,
    outcome: &mut ExecOutcome,
    externs: &ExternRegistry,
) -> Result<(), StepError> {
    // Internal transitions: just run actions, no exit / entry.
    if matches!(t.kind, TransitionKind::Internal) {
        let sctx = StmtCtx {
            machine: &rt.machine.clone(),
            externs,
            current_payload: rt.current_payload.clone(),
        };
        execute_statements(rt, &t.actions, &sctx, outcome)?;
        return Ok(());
    }

    let lca = effective_lca(t, &rt.machine);
    let exits = exit_set(&rt.machine, &t.source, &lca)?;

    // 1) Record history BEFORE running exit actions. Doc 08 §6.4.
    record_history_before_exit(rt, &exits);

    // 2) Compute the full set of states to exit, including any active
    //    descendants of `exits` (parallel children, composite leaves).
    //    Then order them innermost-first; for parallel-state exits the inner
    //    region leaves come BEFORE the parallel state itself, in reverse
    //    region-declaration order (Doc 08 §6.3).
    let mut full_exits: Vec<String> = Vec::new();
    let active_snapshot = rt.active_states.clone();
    for ex in &exits {
        // Include every active descendant of `ex` (active states whose
        // ancestor chain runs through `ex`).
        for a in &active_snapshot {
            if a == ex {
                continue;
            }
            let anc = rt.machine.ancestors(a);
            if anc.iter().any(|x| x == ex) && !full_exits.iter().any(|x| x == a) {
                full_exits.push(a.clone());
            }
        }
        if !full_exits.iter().any(|x| x == ex) {
            full_exits.push(ex.clone());
        }
    }

    for sid in expand_exit_with_parallel(&rt.machine.clone(), &full_exits) {
        run_exit(rt, &sid, outcome, externs)?;
        exited_all.push(sid);
    }
    // After exit, drop the affected active state(s).
    let exit_set_set: HashSet<&String> = exited_all.iter().collect();
    rt.active_states.retain(|s| !exit_set_set.contains(s));

    // 3) Transition action block.
    {
        let sctx = StmtCtx {
            machine: &rt.machine.clone(),
            externs,
            current_payload: rt.current_payload.clone(),
        };
        execute_statements(rt, &t.actions, &sctx, outcome)?;
    }

    // 4) Resolve the target (could be a pseudo-state — choice / junction /
    //    history / fork). Recurse through pseudo-states until we hit a basic /
    //    composite / parallel / final state.
    let resolved = resolve_target(rt, &t.target, externs, outcome)?;
    for tgt in resolved {
        // 5) Entry path from LCA down to `tgt`, then expand initial substates.
        let path = entry_path(&rt.machine.clone(), &lca, &tgt)?;
        for sid in &path {
            run_entry(rt, sid, outcome, externs)?;
            entered_all.push(sid.clone());
        }
        // 6) For composite / parallel / final targets, expand initial.
        let mut leaves = vec![tgt.clone()];
        let mut deeper: Vec<String> = Vec::new();
        expand_initial(&rt.machine.clone(), &tgt, &mut deeper);
        for sid in &deeper {
            run_entry(rt, sid, outcome, externs)?;
            entered_all.push(sid.clone());
        }
        leaves.extend(deeper);
        // 7) Update active_states with the leaf-most basic / final state.
        for leaf in &leaves {
            if is_leaflike(&rt.machine, leaf) {
                rt.active_states.push(leaf.clone());
            }
        }
        // 8) Re-arm timers in newly-entered states only (path + expanded).
        let now = rt.virtual_clock_ms;
        let mut seen_arm: HashSet<String> = HashSet::new();
        for sid in path.into_iter().chain(leaves.into_iter()) {
            if !seen_arm.insert(sid.clone()) {
                continue;
            }
            arm_timers_on_entry(&mut rt.timers, &rt.machine.clone(), &sid, now);
        }
    }
    Ok(())
}

/// Inflate an "abstract" exit set into the actual exit sequence: any
/// parallel-state ancestor is preceded by its region leaves in reverse
/// region-declaration order (Doc 08 §6.3).
fn expand_exit_with_parallel(idx: &MachineIndex, base: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let base_set: HashSet<&String> = base.iter().collect();
    for s in base {
        let node = match idx.node(s) {
            Some(n) => n,
            None => continue,
        };
        if matches!(node.kind, NodeKind::Parallel) {
            // Exit each region's active leaves first, in reverse order.
            // The "active leaf" cannot be derived from `base` alone — caller
            // supplies the inner-region active states through `base` already.
            // Iterate `base` and harvest those whose containing region is one
            // of this parallel's regions; emit in reverse declaration order.
            let mut buckets: Vec<Vec<String>> = node.regions.iter().map(|_| Vec::new()).collect();
            for sub in base {
                if sub == s {
                    continue;
                }
                if let Some(region) = idx.containing_region(sub) {
                    if let Some(pos) = node.regions.iter().position(|r| r == region) {
                        buckets[pos].push(sub.clone());
                    }
                }
            }
            // Reverse region order: last declared region first.
            for bucket in buckets.into_iter().rev() {
                for sub in bucket {
                    if base_set.contains(&sub) {
                        out.push(sub);
                    }
                }
            }
            out.push(s.clone());
        } else if !out.iter().any(|x| x == s) {
            // Filter: skip states already emitted by a parallel ancestor
            // expansion. Detection: if this state's containing region's
            // parent is a parallel that's in `base`, it was already pushed.
            let mut emitted_by_parallel = false;
            if let Some(region) = idx.containing_region(s) {
                if let Some(reg) = idx.region(region) {
                    if let Some(parent) = &reg.parent_state {
                        if base.contains(parent) {
                            if let Some(parent_node) = idx.node(parent) {
                                if matches!(parent_node.kind, NodeKind::Parallel) {
                                    emitted_by_parallel = true;
                                }
                            }
                        }
                    }
                }
            }
            if !emitted_by_parallel {
                out.push(s.clone());
            }
        }
    }
    out
}

fn run_entry(
    rt: &mut RuntimeState,
    state_id: &str,
    outcome: &mut ExecOutcome,
    externs: &ExternRegistry,
) -> Result<(), StepError> {
    let machine = rt.machine.clone();
    let stmts = machine
        .node(state_id)
        .map(|n| n.entry.clone())
        .unwrap_or_default();
    let sctx = StmtCtx {
        machine: &machine,
        externs,
        current_payload: rt.current_payload.clone(),
    };
    execute_statements(rt, &stmts, &sctx, outcome)?;
    Ok(())
}

fn run_exit(
    rt: &mut RuntimeState,
    state_id: &str,
    outcome: &mut ExecOutcome,
    externs: &ExternRegistry,
) -> Result<(), StepError> {
    let machine = rt.machine.clone();
    let stmts = machine
        .node(state_id)
        .map(|n| n.exit.clone())
        .unwrap_or_default();
    let sctx = StmtCtx {
        machine: &machine,
        externs,
        current_payload: rt.current_payload.clone(),
    };
    execute_statements(rt, &stmts, &sctx, outcome)?;
    // Cancel timers owned by this state per Doc 08 §13.2.
    rt.timers.cancel_owned_by(state_id);
    // Doc 08 §12.3 — parent-triggered exit of a submachine-ref state fully
    // exits (drops) its sub-instance. Done here, at the exit point, so a
    // *self-transition* on the ref-state (exit then re-enter in one step)
    // tears the stale sub down; the post-step `sync_submachines` then
    // re-instantiates a FRESH one on the re-entry (no stale sub-state
    // across an exit/enter cycle). Deterministic drop, no leak.
    if matches!(
        machine.node(state_id).map(|n| &n.kind),
        Some(NodeKind::SubmachineRef)
    ) {
        rt.submachines.remove(state_id);
    }
    Ok(())
}

/// Doc 08 §6.4 — record history just before exit. For each exiting state,
/// if any ancestor composite has a `history` pseudo-state, record the leaf
/// (shallow) or the deeper path (deep).
fn record_history_before_exit(rt: &mut RuntimeState, exits: &[String]) {
    // Walk every composite that is being exited and check whether it has a
    // history pseudo-state.
    for ex in exits {
        if let Some(node) = rt.machine.node(ex) {
            if let Some(h) = &node.history {
                let active_in_subtree = find_active_in_subtree(rt, ex);
                match h.history_kind {
                    HistoryKind::Shallow => {
                        // Direct child of `ex`.
                        if let Some(leaf) = active_in_subtree.first() {
                            // Walk back up from leaf to the direct child of `ex`.
                            let direct = direct_child_of(&rt.machine, ex, leaf);
                            rt.history.insert(h.id.clone(), vec![direct]);
                        }
                    }
                    HistoryKind::Deep => {
                        // Full active descendant set.
                        rt.history.insert(h.id.clone(), active_in_subtree);
                    }
                }
            }
        }
    }
}

fn find_active_in_subtree(rt: &RuntimeState, root: &str) -> Vec<String> {
    let mut out = Vec::new();
    for a in &rt.active_states {
        let ancestors = rt.machine.ancestors(a);
        if ancestors.iter().any(|x| x == root) {
            out.push(a.clone());
        }
    }
    out
}

fn direct_child_of(idx: &MachineIndex, ancestor: &str, descendant: &str) -> String {
    // Walk ancestors (state nodes only — skip intermediate region IDs) and
    // return the state that lives directly inside `ancestor`. Shallow
    // history records this direct child, not the region container.
    let path = idx.ancestors(descendant);
    let mut prev_state: Option<&String> = None;
    for id in &path {
        if id == ancestor {
            if let Some(p) = prev_state {
                return p.clone();
            }
            return descendant.to_string();
        }
        if idx.nodes.contains_key(id) {
            prev_state = Some(id);
        }
    }
    descendant.to_string()
}

/// Resolve a transition target through pseudo-states. Choice / Junction
/// branches are evaluated; History returns the recorded state or its default;
/// Fork returns multiple targets.
fn resolve_target(
    rt: &mut RuntimeState,
    target: &str,
    externs: &ExternRegistry,
    outcome: &mut ExecOutcome,
) -> Result<Vec<String>, StepError> {
    let Some(node) = rt.machine.node(target) else {
        return Ok(vec![target.to_string()]);
    };
    match &node.kind {
        NodeKind::Choice { branches } | NodeKind::Junction { branches } => {
            // Pick the first branch whose guard evaluates true; `Else` wins
            // if no other branch matched.
            let mut pick: Option<fsm_ir::ChoiceBranch> = None;
            let evctx = EvalCtx {
                context: &rt.context,
                payload: rt.current_payload.as_ref(),
                externs,
            };
            let mut else_branch: Option<fsm_ir::ChoiceBranch> = None;
            for b in branches {
                if matches!(b.guard, fsm_ir::GuardExpr::Else) {
                    else_branch = Some(b.clone());
                    continue;
                }
                // Ok(true) ⇒ pick; Ok(false) ⇒ try next; Err ⇒ propagate
                // (was silently `false` pre-fix — audit P1-7).
                match eval_guard(&b.guard, &evctx) {
                    Ok(true) => {
                        pick = Some(b.clone());
                        break;
                    }
                    Ok(false) => continue,
                    Err(e) => return Err(StepError::GuardEval(e)),
                }
            }
            let branch = pick.or(else_branch).ok_or_else(|| {
                StepError::Internal(format!("choice {target} has no matching branch"))
            })?;
            // Run branch actions.
            let sctx = StmtCtx {
                machine: &rt.machine.clone(),
                externs,
                current_payload: rt.current_payload.clone(),
            };
            execute_statements(rt, &branch.actions, &sctx, outcome)?;
            resolve_target(rt, &branch.target, externs, outcome)
        }
        NodeKind::History { history } => {
            let saved = rt.history.get(&history.id).cloned();
            match saved {
                Some(path) if !path.is_empty() => Ok(path),
                _ => Ok(vec![history.default_target.clone()]),
            }
        }
        NodeKind::Fork { targets } => Ok(targets.clone()),
        NodeKind::Initial { target } => resolve_target(rt, &target.clone(), externs, outcome),
        _ => Ok(vec![target.to_string()]),
    }
}

/// Expand initial substates of a composite / parallel state. Pushes the
/// expanded state IDs (in entry-action order) into `out`. The original
/// `state` is NOT included.
fn expand_initial(idx: &MachineIndex, state: &str, out: &mut Vec<String>) {
    let Some(node) = idx.node(state) else { return };
    match &node.kind {
        NodeKind::Composite => {
            let Some(region_id) = node.regions.first() else {
                return;
            };
            let Some(reg) = idx.region(region_id) else {
                return;
            };
            let init = &reg.initial_pseudo;
            let Some(init_node) = idx.node(init) else {
                return;
            };
            if let NodeKind::Initial { target } = &init_node.kind {
                out.push(target.clone());
                expand_initial(idx, target, out);
            }
        }
        NodeKind::Parallel => {
            for region_id in &node.regions {
                let Some(reg) = idx.region(region_id) else {
                    continue;
                };
                let init = &reg.initial_pseudo;
                let Some(init_node) = idx.node(init) else {
                    continue;
                };
                if let NodeKind::Initial { target } = &init_node.kind {
                    out.push(target.clone());
                    expand_initial(idx, target, out);
                }
            }
        }
        _ => {}
    }
}

/// Entry into one state at init time, following the entry sequence from
/// `lca` down to `target`. Used by [`Interpreter::init`].
fn enter_state_path(
    rt: &mut RuntimeState,
    lca: &str,
    target: &str,
    entered_all: &mut Vec<String>,
    outcome: &mut ExecOutcome,
    externs: &ExternRegistry,
) -> Result<(), StepError> {
    // Resolve pseudo-state targets (initial transitions can point at
    // composite states whose own initial expansion runs).
    let resolved = resolve_target(rt, target, externs, outcome)?;
    for tgt in resolved {
        let path = entry_path(&rt.machine.clone(), lca, &tgt)?;
        for sid in &path {
            run_entry(rt, sid, outcome, externs)?;
            entered_all.push(sid.clone());
        }
        let mut deeper: Vec<String> = Vec::new();
        expand_initial(&rt.machine.clone(), &tgt, &mut deeper);
        for sid in &deeper {
            run_entry(rt, sid, outcome, externs)?;
            entered_all.push(sid.clone());
        }
        let mut leaves = vec![tgt.clone()];
        leaves.extend(deeper);
        for leaf in &leaves {
            if is_leaflike(&rt.machine, leaf) {
                rt.active_states.push(leaf.clone());
            }
        }
        // Re-arm timers in the newly-entered states only.
        let now = rt.virtual_clock_ms;
        let mut seen_arm: HashSet<String> = HashSet::new();
        for sid in path.into_iter().chain(leaves.into_iter()) {
            if !seen_arm.insert(sid.clone()) {
                continue;
            }
            arm_timers_on_entry(&mut rt.timers, &rt.machine.clone(), &sid, now);
        }
    }
    Ok(())
}

fn is_leaflike(idx: &MachineIndex, state: &str) -> bool {
    matches!(
        idx.node(state).map(|n| &n.kind),
        Some(NodeKind::Simple) | Some(NodeKind::Final) | Some(NodeKind::SubmachineRef)
    )
}

fn arm_timers_on_entry(timers: &mut TimerSet, idx: &MachineIndex, state: &str, now: u64) {
    let Some(node) = idx.node(state) else { return };
    for t in &node.timers {
        let fire = match t.kind {
            TimerKind::After => TimerFire::OneShot,
            TimerKind::Every | TimerKind::EveryInternal => TimerFire::Periodic {
                period_ms: t.duration_ms,
            },
        };
        // P0-4: link timer to its transition by `timer_id` (set by analyzer
        // in `Trigger::After { timer_id }` / `Trigger::Every { timer_id }`).
        // Fall back to legacy (source + target) match for IR docs produced
        // by older test fixtures that don't supply timer_id.
        let transition_id = node.transitions.iter().find_map(|tr| match &tr.trigger {
            Some(Trigger::After { timer_id, .. }) | Some(Trigger::Every { timer_id, .. })
                if !timer_id.is_empty() =>
            {
                if timer_id == &t.id {
                    Some(tr.id.clone())
                } else {
                    None
                }
            }
            Some(Trigger::After { .. }) | Some(Trigger::Every { .. }) => {
                if tr.source == state && Some(tr.target.clone()) == t.target.clone() {
                    Some(tr.id.clone())
                } else {
                    None
                }
            }
            _ => None,
        });
        timers.arm(Timer {
            timer_id: t.id.clone(),
            source_state: state.to_string(),
            transition_id,
            expiry_ms: now + t.duration_ms as u64,
            fires: fire,
        });
    }
}

/// Returns the set of event ids deferred by *any* state in the current
/// active configuration (each active leaf plus all of its ancestors).
/// Doc 08 §10.1: an event is held only if a state that is currently
/// active declares `defer` for it.
fn active_config_deferred_ids(rt: &RuntimeState) -> HashSet<String> {
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
fn active_config_defers(rt: &RuntimeState, event_id: &str) -> bool {
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
fn release_deferred(rt: &mut RuntimeState) -> Result<(), StepError> {
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

/// Build a "Parent.Child" dot path for a state ID, recursing up through
/// states (skipping regions).
///
/// Display-only path-builder, so a cyclic parent table (malformed IR) is
/// truncated at [`MAX_STATE_DEPTH`] rather than returned as an error — the
/// generated display string would be visibly broken anyway, and the
/// upstream callers (`current_states_named`) intentionally tolerate
/// partial-init state.
fn dot_path(rt: &RuntimeState, state_id: &str) -> String {
    let mut names: Vec<String> = Vec::new();
    let mut cur = state_id.to_string();
    let mut guard: u32 = 0;
    while guard < MAX_STATE_DEPTH {
        guard += 1;
        let Some(node) = rt.machine.node(&cur) else {
            break;
        };
        if !matches!(
            node.kind,
            NodeKind::Initial { .. }
                | NodeKind::EntryPoint
                | NodeKind::ExitPoint
                | NodeKind::History { .. }
        ) {
            names.push(node.name.clone());
        }
        match &node.parent_state {
            Some(p) => cur = p.clone(),
            None => break,
        }
    }
    names.reverse();
    names.join(".")
}
