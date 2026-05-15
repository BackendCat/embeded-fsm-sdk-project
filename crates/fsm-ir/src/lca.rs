//! Generic Lowest-Common-Ancestor algorithm — one implementation, three
//! carriers. Per Doc 00 §7.7 (B-09) / Doc 08 §5.1, §6.1.
//!
//! Before this module the analyzer, simulator, and C codegen each carried
//! their own `lca_inclusive` + `effective_lca` against three different
//! carrier types (`String` parent-map, `String` rich index, `u8` index
//! array). That triplication is the exact divergence class that produced
//! the Wave-1.9 analyzer↔simulator contract bug (region.initial). This
//! module collapses the algorithm to a single generic over a
//! [`ParentResolver`] trait; each site implements the trait for its carrier
//! and shares one walk.
//!
//! ## What stays carrier-specific (intentionally)
//!
//! The *shape* of the parent chain is NOT unified — only the *algorithm*
//! is. Each carrier keeps its own notion of "the parent of X":
//!
//! - The analyzer's `MachineIndex` records the **containing region** as a
//!   state's parent (region IDs appear in the chain).
//! - The simulator's `MachineIndex` likewise threads region IDs between
//!   states (`parent_region`).
//! - The C codegen's `ParentTable` *skips* regions — a state's parent is
//!   the enclosing composite/parallel **state** index, with a `u8`
//!   `ROOT_SENTINEL` that is its own parent.
//!
//! These differ because each consumer's exit/entry-set walk is internally
//! consistent with its own chain. The generic must therefore be told, per
//! carrier, both "who is the parent" and "what is the root terminator".
//! Unifying the chain shape would be a behaviour change (forbidden here);
//! unifying the *algorithm* is not. See `effective_lca` for the one rule
//! that is genuinely shared (external self-transition → parent).

use crate::{TransitionKind, TransitionObject};

/// A carrier that can answer "what is the parent of this node?".
///
/// `Id` is the carrier's identity type — `String` for the analyzer /
/// simulator parent maps, `u8` for the C codegen index array. It must be
/// cheaply comparable and clonable so the ancestor walk can collect a
/// chain and intersect two chains.
///
/// The two `root`-related methods exist because the three carriers
/// terminate the upward walk differently: the `String` maps stop when a
/// lookup misses (no entry == root), whereas the `u8` array uses a
/// distinguished `ROOT_SENTINEL` value that is *its own parent*. Encoding
/// the terminator in the trait keeps the shared walk total for every
/// carrier without special-casing.
pub trait ParentResolver {
    /// The node-identity type carried by this resolver.
    type Id: Clone + Eq;

    /// Direct parent of `id`, or `None` if `id` is the root / unknown.
    ///
    /// For the `u8` codegen carrier this returns `Some(ROOT_SENTINEL)` for
    /// the sentinel's own parent; the walk relies on [`Self::is_root`] to
    /// stop rather than on a `None` here, so both conventions compose.
    fn parent(&self, id: &Self::Id) -> Option<Self::Id>;

    /// Whether `id` is the root terminator of the chain.
    ///
    /// Default: never (the `String` maps terminate via a `None` from
    /// [`Self::parent`]). The `u8` codegen carrier overrides this to test
    /// against `ROOT_SENTINEL`.
    fn is_root(&self, _id: &Self::Id) -> bool {
        false
    }
}

/// Ordered ancestor chain of `id`, **including `id` itself at index 0**.
///
/// Root-most ancestor is at the tail. The walk is defensively
/// cycle-guarded (a malformed IR with a parent cycle terminates instead of
/// looping forever) — it stops the first time a node would repeat. This
/// matches the prior hand-rolled `ancestors` walks in the analyzer and
/// simulator, which each had their own ad-hoc loop guard.
pub fn ancestors<R: ParentResolver>(id: &R::Id, r: &R) -> Vec<R::Id> {
    let mut out: Vec<R::Id> = Vec::new();
    let mut cur = id.clone();
    loop {
        if out.iter().any(|x| x == &cur) {
            break;
        }
        out.push(cur.clone());
        if r.is_root(&cur) {
            break;
        }
        match r.parent(&cur) {
            Some(p) => cur = p,
            None => break,
        }
    }
    out
}

/// LCA in the "a state is its own ancestor" sense — Doc 08 §5.1 / Doc 00
/// §7.7 base rule.
///
/// Returns the deepest node that is an ancestor of both `a` and `b`
/// (counting each node as its own ancestor). If the two chains share no
/// ancestor — which only happens on malformed IR — returns `a` itself so
/// callers do not crash. This fallback matches every prior impl
/// (`a.to_string()` / `a`).
pub fn lca_inclusive<R: ParentResolver>(a: &R::Id, b: &R::Id, r: &R) -> R::Id {
    let a_anc = ancestors(a, r);
    let b_anc = ancestors(b, r);
    for cand in &a_anc {
        if b_anc.iter().any(|x| x == cand) {
            return cand.clone();
        }
    }
    a.clone()
}

/// Effective LCA per Doc 00 §7.7 (B-09).
///
/// For an **external self-transition** (`source == target` and
/// `kind == External`), the effective LCA is `source`'s parent so the
/// exit/entry sequence still fires on the state (UML 2.5.1 §14.2.3.9). For
/// every other transition the base [`lca_inclusive`] value applies.
///
/// This is the one rule that is genuinely shared across all three
/// carriers. The carrier-specific part — *what* `parent(source)` is (a
/// region for the analyzer/simulator, a parent-state index for codegen) —
/// is delegated to [`ParentResolver::parent`], so the externally-observable
/// exit/entry sets are byte-identical to the three prior hand-rolled
/// impls.
///
/// `lookup` maps the transition's `String` source/target onto the
/// resolver's `Id` domain. For the `String` carriers this is the identity;
/// for the `u8` codegen carrier it is the state-index lookup. Returning
/// `None` from `lookup` (an unindexed id) degrades to the base LCA of the
/// raw ids — preserving the prior `must_lookup` ⇒ `ROOT_SENTINEL`
/// behaviour at the call site rather than here.
pub fn effective_lca<R, F>(t: &TransitionObject, r: &R, lookup: F) -> R::Id
where
    R: ParentResolver,
    F: Fn(&str) -> R::Id,
{
    let source = lookup(&t.source);
    let target = lookup(&t.target);
    let base = lca_inclusive(&source, &target, r);
    if t.source == t.target && matches!(t.kind, TransitionKind::External) {
        if let Some(parent) = r.parent(&source) {
            return parent;
        }
    }
    base
}

#[cfg(test)]
mod tests {
    //! Equivalence micro-tests (AD-1, 2026-05-15).
    //!
    //! These pin the shared algorithm against the five canonical LCA
    //! scenarios — sibling, nested, external-self, local-self,
    //! cross-parallel — using a trivial `HashMap<&str,&str>` resolver and
    //! a `u8`-array resolver so both carrier shapes are exercised here.
    //! The per-crate adoption tests (analyzer/simulator/codegen) prove the
    //! real carriers produce the same answers as their pre-refactor impls.

    use super::*;
    use crate::Trigger;
    use std::collections::HashMap;

    /// String-keyed resolver (analyzer / simulator carrier shape): a node
    /// with no entry in the map is the root (parent ⇒ `None`).
    struct MapResolver<'a>(HashMap<&'a str, &'a str>);
    impl ParentResolver for MapResolver<'_> {
        type Id = String;
        fn parent(&self, id: &String) -> Option<String> {
            self.0.get(id.as_str()).map(|s| s.to_string())
        }
    }

    /// `u8`-array resolver (codegen carrier shape): index 0 is the root
    /// sentinel and is its own parent; the walk terminates via `is_root`.
    struct ArrResolver(Vec<u8>);
    impl ParentResolver for ArrResolver {
        type Id = u8;
        fn parent(&self, id: &u8) -> Option<u8> {
            self.0.get(*id as usize).copied()
        }
        fn is_root(&self, id: &u8) -> bool {
            *id == 0
        }
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
            loc: crate::SourceLocation::new("t.fsm", crate::Span::new(0, 1), 1, 1),
        }
    }

    // Topology (String carrier):
    //   r-root → s-outer → r-outer → {s-a, s-b, s-mid → r-mid → s-leaf}
    fn map_idx() -> MapResolver<'static> {
        let mut m = HashMap::new();
        m.insert("s-outer", "r-root");
        m.insert("r-outer", "s-outer");
        m.insert("s-a", "r-outer");
        m.insert("s-b", "r-outer");
        m.insert("s-mid", "r-outer");
        m.insert("r-mid", "s-mid");
        m.insert("s-leaf", "r-mid");
        // r-root has no entry ⇒ root.
        MapResolver(m)
    }

    #[test]
    fn sibling_lca_is_containing_region() {
        let idx = map_idx();
        assert_eq!(
            lca_inclusive(&"s-a".to_string(), &"s-b".to_string(), &idx),
            "r-outer"
        );
    }

    #[test]
    fn nested_lca_is_deepest_common_ancestor() {
        let idx = map_idx();
        // s-leaf is under s-mid > r-outer; s-a is directly in r-outer.
        assert_eq!(
            lca_inclusive(&"s-leaf".to_string(), &"s-a".to_string(), &idx),
            "r-outer"
        );
    }

    #[test]
    fn external_self_transition_lifts_to_parent() {
        let idx = map_idx();
        let t = trans("s-leaf", "s-leaf", TransitionKind::External);
        // parent(s-leaf) == r-mid (the containing region).
        assert_eq!(effective_lca(&t, &idx, |s| s.to_string()), "r-mid");
    }

    #[test]
    fn local_self_transition_stays_at_self() {
        let idx = map_idx();
        let t = trans("s-leaf", "s-leaf", TransitionKind::Local);
        // Not external ⇒ base lca_inclusive(s-leaf, s-leaf) == s-leaf.
        assert_eq!(effective_lca(&t, &idx, |s| s.to_string()), "s-leaf");
    }

    #[test]
    fn cross_parallel_lca_is_the_parallel_state() {
        // r-root → s-par (parallel) → {r-x → s-x, r-y → s-y}
        let mut m = HashMap::new();
        m.insert("s-par", "r-root");
        m.insert("r-x", "s-par");
        m.insert("r-y", "s-par");
        m.insert("s-x", "r-x");
        m.insert("s-y", "r-y");
        let idx = MapResolver(m);
        // Leaves in different parallel regions: LCA is the parallel state.
        assert_eq!(
            lca_inclusive(&"s-x".to_string(), &"s-y".to_string(), &idx),
            "s-par"
        );
    }

    // ---- u8 carrier (codegen shape) ----

    #[test]
    fn u8_carrier_sibling_and_self() {
        // idx: 0=ROOT, 1=Op(parent 0), 2=Running(parent 1), 3=Faulted(parent 0)
        let r = ArrResolver(vec![0, 0, 1, 0]);
        // Running & Faulted: chains [2,1,0] ∩ [3,0] ⇒ 0 (ROOT).
        assert_eq!(lca_inclusive(&2u8, &3u8, &r), 0);
        // Running self external ⇒ parent(2) == 1 (Op).
        let t = trans("s-running", "s-running", TransitionKind::External);
        assert_eq!(
            effective_lca(&t, &r, |id| if id == "s-running" { 2 } else { 0 }),
            1
        );
        // Running self local ⇒ base lca(2,2) == 2.
        let tl = trans("s-running", "s-running", TransitionKind::Local);
        assert_eq!(
            effective_lca(&tl, &r, |id| if id == "s-running" { 2 } else { 0 }),
            2
        );
    }
}
