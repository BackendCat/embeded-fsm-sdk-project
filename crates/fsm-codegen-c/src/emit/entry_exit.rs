//! Entry / exit sequencing — Doc 08 §6, §7 and Doc 00 §7.7 (`effective_lca`).
//!
//! For a transition `t` with `source`, `target`, and `kind`:
//!
//! ```text
//! effective_lca(t) =
//!     if t.kind == External and t.source == t.target: parent_of(t.source)
//!     else:                                           lca_inclusive(source, target)
//!
//! exit_set(t)  = path(t.source ← effective_lca)   minus the effective_lca itself
//! entry_set(t) = path(effective_lca → t.target)   minus the effective_lca itself
//! ```
//!
//! Codegen emits an inline sequence of `Motor_exit_X(m); ...; m->_state = T;
//! Motor_entry_T(m);` lines that walks the exit set leaf-first and the
//! entry set root-first. For `Local` and `Internal` transitions the sets
//! are empty (or empty on the source side); the LCA is the source itself.

use fsm_ir::{TransitionKind, TransitionObject};

use crate::parent_table::ParentTable;
use crate::state_index::{StateIndex, ROOT_SENTINEL};

/// Compute the effective LCA index for a transition, per B-09.
pub fn effective_lca(t: &TransitionObject, index: &StateIndex, parents: &ParentTable) -> u8 {
    let source = index.must_lookup(&t.source);
    let target = index.must_lookup(&t.target);
    let base = lca_inclusive(source, target, parents);

    match t.kind {
        TransitionKind::External if source == target => parents.parents[source as usize],
        TransitionKind::Local | TransitionKind::Internal => source,
        _ => base,
    }
}

/// Standard inclusive LCA — a state is its own ancestor. Doc 08 §5.1.
pub fn lca_inclusive(a: u8, b: u8, parents: &ParentTable) -> u8 {
    // Build the ancestor chain of `a` including a itself.
    let mut chain = Vec::with_capacity(8);
    let mut cur = a;
    loop {
        chain.push(cur);
        if cur == ROOT_SENTINEL {
            break;
        }
        cur = parents.parents[cur as usize];
        if cur == ROOT_SENTINEL {
            chain.push(ROOT_SENTINEL);
            break;
        }
    }
    // Walk `b`'s chain until we hit a state in `chain`.
    let mut cur = b;
    loop {
        if chain.contains(&cur) {
            return cur;
        }
        if cur == ROOT_SENTINEL {
            return ROOT_SENTINEL;
        }
        cur = parents.parents[cur as usize];
    }
}

/// Compute the ordered exit set (innermost first) for a transition.
pub fn exit_path(t: &TransitionObject, index: &StateIndex, parents: &ParentTable) -> Vec<u8> {
    let source = index.must_lookup(&t.source);
    let lca = effective_lca(t, index, parents);
    let mut path = Vec::new();

    // For an internal transition there is no exit at all.
    if matches!(t.kind, TransitionKind::Internal) {
        return path;
    }
    // For a local transition the source is NOT exited; only its descendants
    // along the path to the target are. v1.0 codegen treats local self
    // transitions as no-op exit; nested local transitions inherit the
    // descendant-only invariant from B-09.
    if matches!(t.kind, TransitionKind::Local) {
        return path;
    }

    // External transition: walk source upward until we hit (but do NOT
    // include) the effective LCA.
    let mut cur = source;
    while cur != lca && cur != ROOT_SENTINEL {
        path.push(cur);
        cur = parents.parents[cur as usize];
    }
    path
}

/// Compute the ordered entry set (root-first) for a transition.
pub fn entry_path(t: &TransitionObject, index: &StateIndex, parents: &ParentTable) -> Vec<u8> {
    let target = index.must_lookup(&t.target);
    let lca = effective_lca(t, index, parents);

    // Internal transitions never enter anything.
    if matches!(t.kind, TransitionKind::Internal) {
        return Vec::new();
    }
    // Local transitions enter from (but not including) the LCA down to the
    // target.
    let mut chain = Vec::new();
    let mut cur = target;
    while cur != lca && cur != ROOT_SENTINEL {
        chain.push(cur);
        cur = parents.parents[cur as usize];
    }
    chain.reverse();
    chain
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_diagnostics::{SourceLocation, Span};
    use fsm_ir::{
        CompositeState, ContextSchema, InitialPseudo, MachineObject, QueueConfig, RegionObject,
        SimpleState, StateNode, TransitionKind, TransitionObject, Trigger,
    };

    fn loc() -> SourceLocation {
        SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
    }

    #[allow(deprecated)]
    fn t(source: &str, target: &str, kind: TransitionKind) -> TransitionObject {
        TransitionObject {
            id: format!("t-{source}-{target}"),
            stable_id: "T".into(),
            source: source.into(),
            target: target.into(),
            trigger: Some(Trigger::Event {
                event_id: "e".into(),
                payload_binding: None,
            }),
            guard: None,
            actions: vec![],
            priority: 100,
            kind,
            internal: false,
            loc: loc(),
        }
    }

    fn nested_index() -> (
        crate::state_index::StateIndex,
        crate::parent_table::ParentTable,
    ) {
        // root → Op (composite) → Running (simple)
        let m = MachineObject {
            id: "m".into(),
            stable_id: "M".into(),
            name: "M".into(),
            context: ContextSchema::default(),
            events: vec![],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-init".into(),
                        target: "s-op".into(),
                        loc: loc(),
                    }),
                    StateNode::Composite(CompositeState {
                        id: "s-op".into(),
                        stable_id: "M:Op".into(),
                        name: "Op".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![],
                        timers: vec![],
                        defers: vec![],
                        regions: vec![RegionObject {
                            id: "r-op".into(),
                            stable_id: None,
                            name: "Op".into(),
                            initial: "ps-op-init".into(),
                            states: vec![
                                StateNode::Initial(InitialPseudo {
                                    id: "ps-op-init".into(),
                                    target: "s-running".into(),
                                    loc: loc(),
                                }),
                                StateNode::Simple(SimpleState {
                                    id: "s-running".into(),
                                    stable_id: "M:Running".into(),
                                    name: "Running".into(),
                                    entry: vec![],
                                    exit: vec![],
                                    transitions: vec![],
                                    timers: vec![],
                                    defers: vec![],
                                    loc: loc(),
                                }),
                            ],
                            priority: 0,
                            loc: loc(),
                        }],
                        history: None,
                        loc: loc(),
                    }),
                    StateNode::Simple(SimpleState {
                        id: "s-faulted".into(),
                        stable_id: "M:Faulted".into(),
                        name: "Faulted".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![],
                        timers: vec![],
                        defers: vec![],
                        loc: loc(),
                    }),
                ],
                priority: 0,
                loc: loc(),
            },
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            queue: QueueConfig::default(),
            targets: vec![],
            loc: loc(),
        };
        let index = crate::state_index::build_state_index(&m);
        let parents = crate::parent_table::build_parent_table(&index);
        (index, parents)
    }

    #[test]
    fn external_self_transition_lca_is_parent() {
        // B-09: external `s -> s` must exit and re-enter; LCA = parent.
        let (index, parents) = nested_index();
        let tt = t("s-running", "s-running", TransitionKind::External);
        let lca = effective_lca(&tt, &index, &parents);
        let parent = index.lookup("s-op").unwrap();
        assert_eq!(lca, parent);
        // Exit path is [Running]; entry path is [Running].
        assert_eq!(
            exit_path(&tt, &index, &parents),
            vec![index.lookup("s-running").unwrap()]
        );
        assert_eq!(
            entry_path(&tt, &index, &parents),
            vec![index.lookup("s-running").unwrap()]
        );
    }

    #[test]
    fn local_self_transition_has_empty_exit_set() {
        let (index, parents) = nested_index();
        let tt = t("s-running", "s-running", TransitionKind::Local);
        assert_eq!(exit_path(&tt, &index, &parents).len(), 0);
        assert_eq!(entry_path(&tt, &index, &parents).len(), 0);
    }

    #[test]
    fn internal_transition_has_no_entry_or_exit() {
        let (index, parents) = nested_index();
        let tt = t("s-running", "s-running", TransitionKind::Internal);
        assert!(exit_path(&tt, &index, &parents).is_empty());
        assert!(entry_path(&tt, &index, &parents).is_empty());
    }

    #[test]
    fn nested_transition_exits_through_composite() {
        // Running -> Faulted should exit Running AND Op (since Faulted is
        // outside Op), then enter Faulted.
        let (index, parents) = nested_index();
        let tt = t("s-running", "s-faulted", TransitionKind::External);
        let exits = exit_path(&tt, &index, &parents);
        let running = index.lookup("s-running").unwrap();
        let op = index.lookup("s-op").unwrap();
        let faulted = index.lookup("s-faulted").unwrap();
        assert_eq!(exits, vec![running, op]);
        assert_eq!(entry_path(&tt, &index, &parents), vec![faulted]);
    }
}
