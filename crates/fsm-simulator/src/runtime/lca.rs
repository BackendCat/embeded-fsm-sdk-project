//! LCA helpers — adapter over the simulator's [`super::MachineIndex`] so the
//! interpreter does not need to rebuild a parallel parent map in the analyzer
//! crate just to ask "what is the effective LCA?". The B-09 rule is local
//! here, but the parent-link computation reuses our rich machine index.
//!
//! AD-1 (2026-05-15): the walk + the self-transition rule are no longer
//! hand-rolled — they live once in [`fsm_ir::lca`]. `MachineIndex`
//! implements [`fsm_ir::ParentResolver`] (see `machine_index.rs`) with the
//! exact same one-step logic the prior `ancestors()` loop used
//! (state → containing region → parent state → … → root region). These
//! wrappers just adapt the `&str` API onto the generic. Behaviour is
//! byte-identical to the deleted bespoke impl — pinned by the tests below
//! and the `fsm-ir` equivalence suite.

use fsm_ir::TransitionObject;

use super::machine_index::MachineIndex;

/// LCA in the "a state is its own ancestor" sense (Doc 08 §5.1).
pub fn lca_inclusive(a: &str, b: &str, idx: &MachineIndex) -> String {
    fsm_ir::lca_inclusive(&a.to_string(), &b.to_string(), idx)
}

/// Effective LCA per Doc 00 §7.7 (B-09).
///
/// External self-transition (`S -> S`): the effective LCA is the **region**
/// that contains S, NOT a parent state. Doc 08 §6.1 specifies exit_set walks
/// `s ≠ lca` up the tree, and the entry path walks LCA down — using the
/// containing region as the lift keeps the exit/entry sequence to exactly
/// `{S}` for both phases. The shared `effective_lca` realises this lift via
/// `ParentResolver::parent(source)`, which for this carrier returns the
/// containing region (identical to the old `node.parent_region` read).
pub fn effective_lca(t: &TransitionObject, idx: &MachineIndex) -> String {
    fsm_ir::effective_lca(t, idx, |s| s.to_string())
}

#[cfg(test)]
mod tests {
    //! AD-1 behaviour-equivalence guard. Re-derives the expected LCA
    //! answers with a hand-rolled transcription of the deleted loop and
    //! asserts the shared algorithm agrees on a nested + parallel
    //! topology — sibling, nested, external-self, local-self,
    //! cross-parallel.
    use super::*;
    use crate::runtime::machine_index::MachineIndex;
    use fsm_diagnostics::{SourceLocation, Span};
    use fsm_ir::{
        CompositeState, ContextSchema, InitialPseudo, MachineObject, ParallelState, QueueConfig,
        RegionObject, SimpleState, StateNode, TransitionKind, Trigger,
    };
    use std::sync::Arc;

    fn loc() -> SourceLocation {
        SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
    }

    #[allow(deprecated)]
    fn trans(source: &str, target: &str, kind: TransitionKind) -> TransitionObject {
        TransitionObject {
            id: "t".into(),
            stable_id: "t".into(),
            source: source.into(),
            target: target.into(),
            trigger: Some(Trigger::Event {
                event_id: "e".into(),
                payload_binding: None,
            }),
            guard: None,
            actions: vec![],
            priority: 0,
            kind,
            internal: matches!(kind, TransitionKind::Internal),
            hint: None,
            loc: loc(),
        }
    }

    fn simple(id: &str) -> StateNode {
        StateNode::Simple(SimpleState {
            id: id.into(),
            stable_id: id.into(),
            name: id.into(),
            entry: vec![],
            exit: vec![],
            transitions: vec![],
            timers: vec![],
            defers: vec![],
            loc: loc(),
        })
    }

    /// root → Outer(composite)/r-outer → {A, B, Mid(composite)/r-mid → Leaf}
    /// plus root → Par(parallel)/{r-x → X, r-y → Y}
    fn idx() -> MachineIndex {
        let r_mid = RegionObject {
            id: "r-mid".into(),
            stable_id: None,
            name: "rm".into(),
            initial: "ps-mid".into(),
            states: vec![
                StateNode::Initial(InitialPseudo {
                    id: "ps-mid".into(),
                    target: "Leaf".into(),
                    loc: loc(),
                }),
                simple("Leaf"),
            ],
            priority: 0,
            loc: loc(),
        };
        let mid = StateNode::Composite(CompositeState {
            id: "Mid".into(),
            stable_id: "Mid".into(),
            name: "Mid".into(),
            entry: vec![],
            exit: vec![],
            transitions: vec![],
            timers: vec![],
            defers: vec![],
            regions: vec![r_mid],
            history: None,
            loc: loc(),
        });
        let r_outer = RegionObject {
            id: "r-outer".into(),
            stable_id: None,
            name: "ro".into(),
            initial: "ps-outer".into(),
            states: vec![
                StateNode::Initial(InitialPseudo {
                    id: "ps-outer".into(),
                    target: "A".into(),
                    loc: loc(),
                }),
                simple("A"),
                simple("B"),
                mid,
            ],
            priority: 0,
            loc: loc(),
        };
        let outer = StateNode::Composite(CompositeState {
            id: "Outer".into(),
            stable_id: "Outer".into(),
            name: "Outer".into(),
            entry: vec![],
            exit: vec![],
            transitions: vec![],
            timers: vec![],
            defers: vec![],
            regions: vec![r_outer],
            history: None,
            loc: loc(),
        });
        let par = StateNode::Parallel(ParallelState {
            id: "Par".into(),
            stable_id: "Par".into(),
            name: "Par".into(),
            entry: vec![],
            exit: vec![],
            transitions: vec![],
            timers: vec![],
            defers: vec![],
            regions: vec![
                RegionObject {
                    id: "r-x".into(),
                    stable_id: None,
                    name: "rx".into(),
                    initial: "".into(),
                    states: vec![simple("X")],
                    priority: 0,
                    loc: loc(),
                },
                RegionObject {
                    id: "r-y".into(),
                    stable_id: None,
                    name: "ry".into(),
                    initial: "".into(),
                    states: vec![simple("Y")],
                    priority: 0,
                    loc: loc(),
                },
            ],
            loc: loc(),
        });
        let m = MachineObject {
            id: "M".into(),
            stable_id: "M".into(),
            name: "M".into(),
            context: ContextSchema::default(),
            events: vec![],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-root".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-root".into(),
                        target: "Outer".into(),
                        loc: loc(),
                    }),
                    outer,
                    par,
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
        MachineIndex::build(Arc::new(m))
    }

    /// Hand-rolled transcription of the deleted `ancestors()` loop body —
    /// the oracle the shared algorithm must match exactly.
    fn oracle_ancestors(idx: &MachineIndex, id: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut cur = id.to_string();
        for _ in 0..1024 {
            if out.iter().any(|x| x == &cur) {
                break;
            }
            out.push(cur.clone());
            if let Some(node) = idx.node(&cur) {
                if let Some(region) = &node.parent_region {
                    cur = region.clone();
                } else {
                    break;
                }
            } else if let Some(region) = idx.region(&cur) {
                if cur == idx.root_region_id {
                    break;
                }
                match &region.parent_state {
                    Some(s) => cur = s.clone(),
                    None => break,
                }
            } else {
                break;
            }
        }
        out
    }

    fn oracle_lca(idx: &MachineIndex, a: &str, b: &str) -> String {
        let a_anc = oracle_ancestors(idx, a);
        let b_anc: std::collections::HashSet<_> = oracle_ancestors(idx, b).into_iter().collect();
        for cand in &a_anc {
            if b_anc.contains(cand) {
                return cand.clone();
            }
        }
        a.to_string()
    }

    #[test]
    fn ancestors_and_lca_match_oracle_for_all_canonical_pairs() {
        let idx = idx();
        for id in ["Leaf", "A", "B", "Mid", "Outer", "X", "Y", "r-mid", "Par"] {
            assert_eq!(
                idx.ancestors(id),
                oracle_ancestors(&idx, id),
                "ancestors mismatch for {id}"
            );
        }
        for (a, b) in [
            ("A", "B"),       // sibling → r-outer
            ("Leaf", "A"),    // nested  → r-outer
            ("X", "Y"),       // cross-parallel → Par
            ("Leaf", "Leaf"), // self
        ] {
            assert_eq!(
                lca_inclusive(a, b, &idx),
                oracle_lca(&idx, a, b),
                "lca mismatch for ({a},{b})"
            );
        }
        // Concrete expected values (independent of the oracle).
        assert_eq!(lca_inclusive("A", "B", &idx), "r-outer");
        assert_eq!(lca_inclusive("Leaf", "A", &idx), "r-outer");
        assert_eq!(lca_inclusive("X", "Y", &idx), "Par");
    }

    #[test]
    fn external_self_lifts_to_containing_region() {
        let idx = idx();
        let t = trans("Leaf", "Leaf", TransitionKind::External);
        assert_eq!(effective_lca(&t, &idx), "r-mid");
    }

    #[test]
    fn local_self_stays_at_self() {
        let idx = idx();
        let t = trans("Leaf", "Leaf", TransitionKind::Local);
        assert_eq!(effective_lca(&t, &idx), "Leaf");
    }
}
