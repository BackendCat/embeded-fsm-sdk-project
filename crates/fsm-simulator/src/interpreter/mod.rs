//! The central RTC step interpreter.
//!
//! Implements Doc 08 (FSM-SPEC-SEM) faithfully — see the section references
//! sprinkled through the code (and the per-submodule headers):
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
//!
//! AUDIT_2026_06_06 §2.2 P1.1 — split from a single 1899-LOC `interpreter.rs`
//! into Doc 08-aligned submodules: `step` (§3/§4/§14), `transitions` (§4),
//! `path` (§5/§6.1/§7.1), `run` (§6.2/§7.2), `history` (§8), `submachine`
//! (§12), `defer` (§10), `timer` (§13). Public API surface (this `impl
//! Interpreter` block) and re-exports in `crate::lib` unchanged.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use fsm_ir::Ir;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::eval::{EvalError, ExecOutcome, ExternRegistry, StmtError};
use crate::runtime::{
    check_and_enqueue_completion, EventKind, InterpreterSnapshot, MachineIndex, NodeKind,
    QueueError, QueuedEvent, RuntimeState, Value,
};
use crate::trace::{StepKind, StepRecord};

mod defer;
mod history;
mod path;
mod run;
mod step;
mod submachine;
mod timer;
mod transitions;

use self::path::dot_path;
use self::run::enter_state_path;
use self::step::drain_queue_rt;

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
    /// free-function core ([`step::drain_queue_rt`]) so a submachine
    /// sub-instance drains its own queue through the *identical* algorithm
    /// (Doc 08 §12 — no duplicated RTC).
    fn drain_internal_queue(&mut self) -> Result<Vec<StepRecord>, StepError> {
        let rt = self.runtime.as_mut().ok_or(StepError::NotInitialized)?;
        drain_queue_rt(rt, &self.externs, 0)
    }
}
