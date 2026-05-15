//! Lowest-Common-Ancestor utilities — per Doc 00 §7.7 (B-09).
//!
//! `lca_inclusive(a, b)` is the LCA function from UML 2.5.1: a state is its
//! own ancestor. `effective_lca(t)` then adjusts the result for the corner
//! case of an **external** self-transition (`S -> S`), where exit/entry must
//! still fire — UML 2.5.1 §14.2.3.9 demands the effective LCA be
//! `S.parent` (the containing region for this carrier). The `kind`
//! discriminator on `TransitionObject` keeps the rule local.
//!
//! All paths walk through an in-memory parent map (`state_id ->
//! parent_state_id`) built by [`LcaIndex::build`], so LCA is an
//! O(depth) walk.
//!
//! AD-1 (2026-05-15): the walk + the self-transition rule are no longer
//! hand-rolled here — they live once in [`fsm_ir::lca`]. This module only
//! supplies the carrier (`LcaIndex` implements
//! [`fsm_ir::ParentResolver`] over `String` ids) and the thin public
//! wrappers the rest of the toolchain calls. Behaviour is byte-identical
//! to the prior bespoke impl (proven by this module's tests + the
//! `fsm-ir` equivalence suite).

use std::collections::HashMap;

use fsm_ir::{Ir, MachineObject, ParentResolver, StateNode, TransitionObject};

/// Lazily-built parent map for one [`MachineObject`]. Construct via
/// [`LcaIndex::build`] before calling [`lca_inclusive`] or
/// [`effective_lca`].
#[derive(Clone, Debug, Default)]
pub struct LcaIndex {
    parents: HashMap<String, String>,
    /// Set of every state ID known in the machine — used to short-circuit
    /// "unknown id" so the LCA walk does not loop on malformed IR.
    known: HashMap<String, ()>,
}

impl LcaIndex {
    /// Build a parent map for `m`. Walks the machine root region recursively.
    pub fn build(m: &MachineObject) -> Self {
        let mut idx = LcaIndex::default();
        idx.known.insert(m.root.id.clone(), ());
        for s in &m.root.states {
            idx.record_state(s, &m.root.id);
        }
        idx
    }

    fn record_state(&mut self, state: &StateNode, parent: &str) {
        let (id, regions): (&String, Option<&Vec<fsm_ir::RegionObject>>) = match state {
            StateNode::Simple(s) => (&s.id, None),
            StateNode::Composite(s) => (&s.id, Some(&s.regions)),
            StateNode::Parallel(s) => (&s.id, Some(&s.regions)),
            StateNode::Initial(s) => (&s.id, None),
            StateNode::Final(s) => (&s.id, None),
            StateNode::Choice(s) => (&s.id, None),
            StateNode::Junction(s) => (&s.id, None),
            StateNode::History(s) => (&s.id, None),
            StateNode::Fork(s) => (&s.id, None),
            StateNode::Join(s) => (&s.id, None),
            StateNode::Submachine(s) => (&s.id, None),
            StateNode::EntryPoint(s) => (&s.id, None),
            StateNode::ExitPoint(s) => (&s.id, None),
        };
        self.parents.insert(id.clone(), parent.to_string());
        self.known.insert(id.clone(), ());
        if let Some(regions) = regions {
            for region in regions {
                self.parents.insert(region.id.clone(), id.clone());
                self.known.insert(region.id.clone(), ());
                for child in &region.states {
                    self.record_state(child, &region.id);
                }
            }
        }
    }

    /// Direct parent of `id`. Returns `None` for the machine root or unknown
    /// IDs.
    pub fn parent_of(&self, id: &str) -> Option<&str> {
        self.parents.get(id).map(String::as_str)
    }

    /// Ordered list of ancestors of `id`, **including `id` itself** at index
    /// 0. Root-most ancestor is at the tail.
    pub fn ancestors(&self, id: &str) -> Vec<String> {
        // Unknown ids have no chain at all (preserves the prior behaviour
        // where the walk broke before pushing anything).
        if !self.known.contains_key(id) {
            return Vec::new();
        }
        fsm_ir::ancestors(&id.to_string(), self)
    }
}

/// `LcaIndex` is the `String`-id carrier for the shared LCA algorithm.
/// A node missing from the parent map (the machine root, or an unknown id)
/// has no parent — the upward walk terminates there, exactly as the prior
/// bespoke loop did.
impl ParentResolver for LcaIndex {
    type Id = String;

    fn parent(&self, id: &String) -> Option<String> {
        self.parents.get(id).cloned()
    }
}

/// LCA in the "a state is its own ancestor" sense — per Doc 08 §5.1 / Doc 00
/// §7.7 base rule. If the two paths share no ancestor (which would indicate a
/// malformed IR), returns `a` itself so callers do not crash.
pub fn lca_inclusive(a: &str, b: &str, idx: &LcaIndex) -> String {
    fsm_ir::lca_inclusive(&a.to_string(), &b.to_string(), idx)
}

/// Effective LCA per Doc 00 §7.7 (B-09).
///
/// For external self-transitions (`source == target` and `kind == External`),
/// the effective LCA is `source.parent` (the containing region for this
/// carrier) so the exit/entry sequence still fires on S. For every other
/// case, the base `lca_inclusive` value applies.
pub fn effective_lca(t: &TransitionObject, idx: &LcaIndex) -> String {
    fsm_ir::effective_lca(t, idx, |s| s.to_string())
}

/// Convenience — build the index from `ir` for machine index `m_idx` and
/// return the effective LCA of `t`. Returns the source ID if `m_idx` is out of
/// range.
pub fn effective_lca_in_ir(t: &TransitionObject, ir: &Ir, m_idx: usize) -> String {
    let Some(m) = ir.machines.get(m_idx) else {
        return t.source.clone();
    };
    let idx = LcaIndex::build(m);
    effective_lca(t, &idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_diagnostics::{SourceLocation, Span};
    use fsm_ir::{
        CompositeState, MachineObject, QueueConfig, RegionObject, SimpleState, StateNode,
        TransitionKind, Trigger,
    };

    fn loc() -> SourceLocation {
        SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
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

    fn composite(id: &str, regions: Vec<RegionObject>) -> StateNode {
        StateNode::Composite(CompositeState {
            id: id.into(),
            stable_id: id.into(),
            name: id.into(),
            entry: vec![],
            exit: vec![],
            transitions: vec![],
            timers: vec![],
            defers: vec![],
            regions,
            history: None,
            loc: loc(),
        })
    }

    fn region(id: &str, states: Vec<StateNode>) -> RegionObject {
        RegionObject {
            id: id.into(),
            stable_id: None,
            name: id.into(),
            initial: "".into(),
            states,
            priority: 0,
            loc: loc(),
        }
    }

    fn machine_with_root(root: RegionObject) -> MachineObject {
        MachineObject {
            id: "M".into(),
            stable_id: "M".into(),
            name: "M".into(),
            context: Default::default(),
            events: vec![],
            externs: vec![],
            root,
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            queue: QueueConfig::default(),
            targets: vec![],
            loc: loc(),
        }
    }

    fn trans(source: &str, target: &str, kind: TransitionKind) -> TransitionObject {
        #[allow(deprecated)]
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
            loc: loc(),
        }
    }

    #[test]
    fn external_self_transition_lifts_to_parent() {
        let inner = region("R0", vec![simple("S")]);
        let outer = region("Root", vec![composite("Outer", vec![inner])]);
        let m = machine_with_root(outer);
        let idx = LcaIndex::build(&m);
        let t = trans("S", "S", TransitionKind::External);
        assert_eq!(effective_lca(&t, &idx), "R0");
    }

    #[test]
    fn local_self_transition_stays_at_self() {
        let inner = region("R0", vec![simple("S")]);
        let outer = region("Root", vec![composite("Outer", vec![inner])]);
        let m = machine_with_root(outer);
        let idx = LcaIndex::build(&m);
        let t = trans("S", "S", TransitionKind::Local);
        // local self-transition: base lca is inclusive S.
        assert_eq!(effective_lca(&t, &idx), "S");
    }

    #[test]
    fn lca_of_siblings_is_their_region() {
        let r = region("R0", vec![simple("A"), simple("B")]);
        let root = region("Root", vec![composite("Outer", vec![r])]);
        let m = machine_with_root(root);
        let idx = LcaIndex::build(&m);
        assert_eq!(lca_inclusive("A", "B", &idx), "R0");
    }

    #[test]
    fn lca_of_nested_is_deepest_common_ancestor() {
        let leaf_r = region("R0", vec![simple("Leaf")]);
        let middle = composite("Mid", vec![leaf_r]);
        let outer_r = region("OuterR", vec![middle, simple("Sib")]);
        let root = region("Root", vec![composite("Outer", vec![outer_r])]);
        let m = machine_with_root(root);
        let idx = LcaIndex::build(&m);
        // Leaf is in Mid > OuterR. Sib is directly in OuterR. LCA = OuterR.
        assert_eq!(lca_inclusive("Leaf", "Sib", &idx), "OuterR");
    }

    /// AD-1 guard: the shared algorithm must agree with a from-scratch
    /// hand walk of the same parent map (a transcription of the deleted
    /// pre-refactor body). If they ever diverge, the shared generic
    /// changed observable behaviour — which this wave forbids.
    #[test]
    fn shared_algo_matches_handrolled_oracle() {
        let leaf_r = region("R0", vec![simple("Leaf")]);
        let middle = composite("Mid", vec![leaf_r]);
        let outer_r = region("OuterR", vec![middle, simple("Sib")]);
        let root = region("Root", vec![composite("Outer", vec![outer_r])]);
        let m = machine_with_root(root);
        let idx = LcaIndex::build(&m);

        fn oracle_ancestors(idx: &LcaIndex, id: &str) -> Vec<String> {
            let mut out = Vec::new();
            let mut cur = id.to_string();
            loop {
                if !idx.known.contains_key(&cur) {
                    break;
                }
                out.push(cur.clone());
                match idx.parents.get(&cur) {
                    Some(p) => cur = p.clone(),
                    None => break,
                }
            }
            out
        }
        fn oracle_lca(idx: &LcaIndex, a: &str, b: &str) -> String {
            let a_anc = oracle_ancestors(idx, a);
            let b_anc: std::collections::HashSet<_> =
                oracle_ancestors(idx, b).into_iter().collect();
            for cand in &a_anc {
                if b_anc.contains(cand) {
                    return cand.clone();
                }
            }
            a.to_string()
        }

        for (a, b) in [
            ("Leaf", "Sib"),
            ("Sib", "Leaf"),
            ("Leaf", "Leaf"),
            ("Mid", "Sib"),
            ("R0", "OuterR"),
        ] {
            assert_eq!(
                lca_inclusive(a, b, &idx),
                oracle_lca(&idx, a, b),
                "mismatch for ({a}, {b})"
            );
        }
    }
}
