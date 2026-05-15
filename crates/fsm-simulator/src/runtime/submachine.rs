//! Submachine sub-instance lifecycle — Doc 08 §12.
//!
//! A `StateNode::Submachine` ("submachine reference") state is a leaf in
//! the parent's active configuration, but while it is active it owns an
//! independent nested execution context: a sub-[`RuntimeState`] over the
//! referenced template (Doc 08 §12.1 — separate context, events not
//! auto-forwarded at the formal level; the simulator drives the sub by the
//! parent-dispatched event *by name* when no parent transition consumed it,
//! the established transition-wins ordering).
//!
//! This module is intentionally small and free-function-shaped so the
//! parent RTC ([`crate::interpreter`]) composes with it without duplicating
//! the existing defer / done-autofire / `ParentResolver` machinery:
//!
//! - [`instantiate_on_entry`] — entering a ref-state builds + inits the
//!   nested instance at the template's initial pseudo-state (Doc 08 §12.2).
//!   Empty `entry_points` ⇒ implicit-initial entry (W2b emits them empty by
//!   design; named entry-points are a future sub-wave).
//! - [`delegate_event`] — a parent-unconsumed event is routed into the sub
//!   instance's run-to-completion step (Doc 08 §3 inside the sub context).
//! - [`sub_reached_final`] — completion detection: the sub's active leaf is
//!   a `Final` (or a named exit-point; none in v1.1). The *parent* then
//!   enqueues `Completion(ref_state)` so the ref-state's `done -> Target`
//!   fires through the **existing** R1 completion path — this module never
//!   re-implements completion, it only supplies the trigger.
//! - [`teardown_on_exit`] — exiting the ref-state drops the nested instance
//!   deterministically (no leak).
//!
//! Bounded nesting: each sub-instance adds one [`RuntimeState`] frame. The
//! static template graph is acyclic (FSM-E0502 rejects instantiation cycles
//! at analysis), so the live nesting depth is bounded by the template DAG
//! height; the recursion-prevention guard ([`MAX_SUBMACHINE_DEPTH`]) is a
//! defence-in-depth ceiling against a malformed IR that slipped the
//! analyzer, mirroring the interpreter's `MAX_STATE_DEPTH` intent.

use std::sync::Arc;

use crate::runtime::machine_index::{MachineIndex, NodeKind};
use crate::runtime::state::RuntimeState;

/// Defence-in-depth cap on live submachine nesting (Doc 08 §12 has no
/// normative cap; FSM-E0502 already rejects static template cycles). A
/// realistic submachine DAG nests only a handful of levels; this ceiling
/// detects malformed-IR recursion before it blows the stack — the same
/// role `interpreter::MAX_STATE_DEPTH` plays for the state tree.
pub const MAX_SUBMACHINE_DEPTH: u32 = 64;

/// Build (uninitialised) a nested sub-`RuntimeState` for the template a
/// `StateNode::Submachine` references. Returns `None` if `ref_state_id` is
/// not a submachine-ref in `parent`'s index, or its `submachine_id` does
/// not resolve to a template index (unresolved ⇒ FSM-E0103 at analysis;
/// the simulator degrades to a leaf no-op rather than panicking — Doc 09
/// §1 partial-IR principle).
///
/// The caller (the parent RTC) is responsible for *initialising* the
/// returned instance (entry sequence) and inserting it into
/// `parent_rt.submachines[ref_state_id]` — kept here as a pure factory so
/// the interpreter owns the entry-sequence wiring it already implements.
pub fn build_sub_runtime(parent_index: &MachineIndex, ref_state_id: &str) -> Option<RuntimeState> {
    let node = parent_index.node(ref_state_id)?;
    if !matches!(node.kind, NodeKind::SubmachineRef) {
        return None;
    }
    let submachine_id = parent_index
        .machine
        .root
        .states
        .iter()
        .find_map(|s| match s {
            fsm_ir::StateNode::Submachine(sr) if sr.id == ref_state_id => {
                Some(sr.submachine_id.clone())
            }
            _ => None,
        })
        .or_else(|| {
            // The ref-state may be nested inside a composite/parallel; fall
            // back to a full tree scan via the IR model.
            find_submachine_id_deep(&parent_index.machine, ref_state_id)
        })?;
    let tmpl: Arc<MachineIndex> = parent_index.submachine_index(&submachine_id)?;
    Some(RuntimeState::new(tmpl))
}

/// Recursive `submachine_id` lookup for a ref-state nested anywhere in the
/// machine's region tree (the common case — a top-level ref-state — is
/// handled by the cheap root scan in [`build_sub_runtime`]).
fn find_submachine_id_deep(m: &fsm_ir::MachineObject, ref_state_id: &str) -> Option<String> {
    fn walk(region: &fsm_ir::RegionObject, target: &str) -> Option<String> {
        for s in &region.states {
            match s {
                fsm_ir::StateNode::Submachine(sr) if sr.id == target => {
                    return Some(sr.submachine_id.clone());
                }
                fsm_ir::StateNode::Composite(c) => {
                    for r in &c.regions {
                        if let Some(found) = walk(r, target) {
                            return Some(found);
                        }
                    }
                }
                fsm_ir::StateNode::Parallel(p) => {
                    for r in &p.regions {
                        if let Some(found) = walk(r, target) {
                            return Some(found);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    walk(&m.root, ref_state_id)
}

/// Resolve the template's entry target (Doc 08 §12.2). With no named
/// entry-points (W2b emits `entry_points` empty by design) entry is the
/// template root region's initial pseudo-state — exactly how
/// `Interpreter::init` enters any machine. Returns `(root_region_id,
/// initial_target_state_id)` so the caller reuses its existing
/// entry-sequence routine unchanged.
pub fn entry_target(sub_index: &MachineIndex) -> Option<(String, String)> {
    let root_id = sub_index.root_region_id.clone();
    let initial_pseudo = sub_index
        .region(&root_id)
        .map(|r| r.initial_pseudo.clone())?;
    match &sub_index.node(&initial_pseudo)?.kind {
        NodeKind::Initial { target } => Some((root_id, target.clone())),
        _ => None,
    }
}

/// True when the sub-instance has reached completion (Doc 08 §12.3): its
/// active configuration's leaf is a `Final` state. (Named exit-points are a
/// future sub-wave — W2b emits `exit_points` empty; a single-region
/// template completes on its `final`.) The parent uses this to decide
/// whether to fire the ref-state's `done ->` completion via the existing
/// R1 path.
pub fn sub_reached_final(sub_rt: &RuntimeState) -> bool {
    if !sub_rt.initialized || sub_rt.active_states.is_empty() {
        return false;
    }
    sub_rt.active_states.iter().all(|leaf| {
        matches!(
            sub_rt.machine.node(leaf).map(|n| &n.kind),
            Some(NodeKind::Final)
        )
    })
}
