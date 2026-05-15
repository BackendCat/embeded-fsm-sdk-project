//! Completion event helper — Doc 08 §9, Doc 00 §7.6 (B-08).
//!
//! When a state is entered, the interpreter calls
//! [`check_and_enqueue_completion`] which:
//!
//! 1. If the just-entered state itself declares a `done -> Target`
//!    transition (UML "completion event" / auto-transition, Doc 04 §8.4),
//!    enqueue `Completion(just_entered)`. This fires the auto-transition
//!    after entry actions complete, without waiting for an external event.
//!
//! 2. Walks the just-entered state's ancestor chain and enqueues a
//!    completion event for the deepest enclosing composite/parallel whose
//!    completion condition is satisfied:
//!    - **Composite parent**: if its single region's active leaf is `Final`,
//!      enqueue `Completion(parent.id)`.
//!    - **Parallel parent**: if EVERY region's active leaf is `Final`,
//!      enqueue `Completion(parent.id)`.
//!
//! Doc 08 §9.3 propagates outward — propagation is naturally handled by
//! firing the completion event for the inner state; when the inner state is
//! exited and its parent becomes (recursively) final, the outer
//! `check_and_enqueue_completion` call from the next entry sequence catches it.

use fsm_ir::TransitionKind;

use crate::runtime::event::{EventKind, QueuedEvent};
use crate::runtime::machine_index::NodeKind;
use crate::runtime::queue::QueueError;
use crate::runtime::state::RuntimeState;

/// Walk ancestors of `just_entered`, enqueueing at most one Completion
/// event. Priority order:
///
/// 1. If `just_entered` itself has a `done -> X` transition, enqueue
///    `Completion(just_entered)` so the auto-transition fires next step.
/// 2. Otherwise, find the deepest enclosing composite/parallel whose
///    region(s) have all reached Final and enqueue a parent-completion.
pub fn check_and_enqueue_completion(
    rt: &mut RuntimeState,
    just_entered: &str,
) -> Result<(), QueueError> {
    // (1) Self-completion: `done -> X` on a basic state fires after entry
    // actions complete, regardless of Final-state status. Doc 04 §8.4 +
    // Doc 08 §3.1: the completion event for a non-final state with a `done`
    // transition is the same auto-transition mechanism UML uses for
    // "completion events".
    if state_has_done_transition(rt, just_entered) {
        rt.queue.push_front(QueuedEvent::new(
            EventKind::Completion {
                state_id: just_entered.to_string(),
            },
            None,
        ))?;
        return Ok(());
    }
    let ancestors = rt.machine.ancestors(just_entered);
    // The first ancestor is `just_entered` itself; skip it for the check
    // unless it is itself a `Final` state (in which case its enclosing
    // composite may complete).
    for anc in ancestors.iter() {
        let Some(node) = rt.machine.node(anc) else {
            continue;
        };
        let parent_id = match &node.parent_state {
            Some(p) => p.clone(),
            None => continue, // top-level state; no parent to complete.
        };
        let Some(parent) = rt.machine.node(&parent_id) else {
            continue;
        };
        let satisfied = match &parent.kind {
            NodeKind::Composite => is_final_active_leaf(rt, &parent_id),
            NodeKind::Parallel => all_regions_final(rt, parent),
            _ => false,
        };
        if satisfied {
            rt.queue.push_front(QueuedEvent::new(
                EventKind::Completion {
                    state_id: parent_id,
                },
                None,
            ))?;
            return Ok(());
        }
    }
    Ok(())
}

/// True iff `state_id` is a **basic** (Simple) state that declares at
/// least one `done -> X` transition (`TransitionKind::Completion`).
///
/// Composite and Parallel states also use `done`, but their completion
/// semantics are gated by region/Final-state status — handled in the
/// existing ancestor walk below. Self-completion for non-basic states
/// would fire prematurely (before all regions reach Final). Pseudo-states
/// never carry transitions, so they always return `false`.
fn state_has_done_transition(rt: &RuntimeState, state_id: &str) -> bool {
    let Some(node) = rt.machine.node(state_id) else {
        return false;
    };
    if !matches!(node.kind, NodeKind::Simple) {
        return false;
    }
    node.transitions
        .iter()
        .any(|t| t.kind == TransitionKind::Completion)
}

/// True iff the single-region composite `state_id` has its active leaf in a
/// `Final` state.
fn is_final_active_leaf(rt: &RuntimeState, state_id: &str) -> bool {
    // Composite has exactly one region per Doc 09 §4.2. Find which active
    // state lives in that region.
    let Some(parent) = rt.machine.node(state_id) else {
        return false;
    };
    let Some(region_id) = parent.regions.first() else {
        return false;
    };
    let leaf = active_leaf_in_region(rt, region_id);
    let Some(leaf) = leaf else { return false };
    matches!(
        rt.machine.node(&leaf).map(|n| &n.kind),
        Some(NodeKind::Final)
    )
}

fn all_regions_final(rt: &RuntimeState, parent: &crate::runtime::machine_index::NodeRef) -> bool {
    if parent.regions.is_empty() {
        return false;
    }
    parent.regions.iter().all(|r| {
        active_leaf_in_region(rt, r)
            .as_deref()
            .map(|leaf| {
                matches!(
                    rt.machine.node(leaf).map(|n| &n.kind),
                    Some(NodeKind::Final)
                )
            })
            .unwrap_or(false)
    })
}

/// Find the active state ID whose containing region is `region_id` or whose
/// ancestor chain reaches `region_id`. Returns the first match.
pub fn active_leaf_in_region(rt: &RuntimeState, region_id: &str) -> Option<String> {
    for s in &rt.active_states {
        for anc in rt.machine.ancestors(s) {
            if anc == region_id {
                return Some(s.clone());
            }
        }
    }
    None
}
