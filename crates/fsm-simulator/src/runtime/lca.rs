//! LCA helpers — adapter over the simulator's [`super::MachineIndex`] so the
//! interpreter does not need to rebuild a parallel parent map in the analyzer
//! crate just to ask "what is the effective LCA?". The B-09 rule is local
//! here, but the parent-link computation reuses our rich machine index.

use fsm_ir::{TransitionKind, TransitionObject};

use super::machine_index::MachineIndex;

/// LCA in the "a state is its own ancestor" sense (Doc 08 §5.1).
pub fn lca_inclusive(a: &str, b: &str, idx: &MachineIndex) -> String {
    let a_anc = idx.ancestors(a);
    let b_anc: std::collections::HashSet<_> = idx.ancestors(b).into_iter().collect();
    for cand in &a_anc {
        if b_anc.contains(cand) {
            return cand.clone();
        }
    }
    a.to_string()
}

/// Effective LCA per Doc 00 §7.7 (B-09).
///
/// External self-transition (`S -> S`): the effective LCA is the **region**
/// that contains S, NOT a parent state. Doc 08 §6.1 specifies exit_set walks
/// `s ≠ lca` up the tree, and the entry path walks LCA down — using the
/// containing region as the lift keeps the exit/entry sequence to exactly
/// `{S}` for both phases.
pub fn effective_lca(t: &TransitionObject, idx: &MachineIndex) -> String {
    let base = lca_inclusive(&t.source, &t.target, idx);
    if t.source == t.target && matches!(t.kind, TransitionKind::External) {
        if let Some(node) = idx.node(&t.source) {
            if let Some(region) = &node.parent_region {
                return region.clone();
            }
        }
    }
    base
}
