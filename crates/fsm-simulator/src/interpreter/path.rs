//! Structural-query helpers — Doc 08 §5 (LCA), §6.1 (exit set computation),
//! §7.1 (entry path computation), plus pure read-only walkers used by the
//! history and active-config presentation logic.
//!
//! Every function here is a pure query over [`MachineIndex`] /
//! [`RuntimeState`] — no semantics, no transition selection. Routines that
//! *run* entry / exit actions live in [`super::run`].

use std::collections::HashSet;

use crate::runtime::{MachineIndex, NodeKind, RuntimeState};

use super::StepError;

/// Maximum ancestor-walk depth before we treat the parent table as cyclic
/// and bail out with an internal error. UML statecharts in practice nest
/// ≤15 levels — see Doc 08 §2.1 (no normative cap, but every realistic
/// model is far shallower). 256 covers the wildest legitimate nest and
/// still detects malformed-IR cycles before they blow the stack.
pub(super) const MAX_STATE_DEPTH: u32 = 256;

/// Compute exit set: every state from `source` walking up the parent chain
/// until we reach (but do not include) `lca`. Innermost-first ordering.
///
/// Defence-in-depth against malformed IR with a cyclic parent table — at
/// most [`MAX_STATE_DEPTH`] iterations before we bail out with
/// `StepError::Internal`. UML statecharts in practice nest ≤15 levels.
pub(super) fn exit_set(
    idx: &MachineIndex,
    source: &str,
    lca: &str,
) -> Result<Vec<String>, StepError> {
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
pub(super) fn entry_path(
    idx: &MachineIndex,
    lca: &str,
    target: &str,
) -> Result<Vec<String>, StepError> {
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

/// Inflate an "abstract" exit set into the actual exit sequence: any
/// parallel-state ancestor is preceded by its region leaves in reverse
/// region-declaration order (Doc 08 §6.3).
pub(super) fn expand_exit_with_parallel(idx: &MachineIndex, base: &[String]) -> Vec<String> {
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

/// Expand initial substates of a composite / parallel state. Pushes the
/// expanded state IDs (in entry-action order) into `out`. The original
/// `state` is NOT included.
pub(super) fn expand_initial(idx: &MachineIndex, state: &str, out: &mut Vec<String>) {
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

pub(super) fn find_active_in_subtree(rt: &RuntimeState, root: &str) -> Vec<String> {
    let mut out = Vec::new();
    for a in &rt.active_states {
        let ancestors = rt.machine.ancestors(a);
        if ancestors.iter().any(|x| x == root) {
            out.push(a.clone());
        }
    }
    out
}

pub(super) fn direct_child_of(idx: &MachineIndex, ancestor: &str, descendant: &str) -> String {
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

pub(super) fn is_leaflike(idx: &MachineIndex, state: &str) -> bool {
    matches!(
        idx.node(state).map(|n| &n.kind),
        Some(NodeKind::Simple) | Some(NodeKind::Final) | Some(NodeKind::SubmachineRef)
    )
}

/// Build a "Parent.Child" dot path for a state ID, recursing up through
/// states (skipping regions).
///
/// Display-only path-builder, so a cyclic parent table (malformed IR) is
/// truncated at [`MAX_STATE_DEPTH`] rather than returned as an error — the
/// generated display string would be visibly broken anyway, and the
/// upstream callers (`current_states_named`) intentionally tolerate
/// partial-init state.
pub(super) fn dot_path(rt: &RuntimeState, state_id: &str) -> String {
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
