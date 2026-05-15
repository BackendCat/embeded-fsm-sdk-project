//! AD-1 equivalence proof (2026-05-15) — the shared `ParentResolver` LCA
//! algorithm must yield byte-identical results to the three pre-refactor
//! hand-rolled impls (analyzer `String` parent-map, simulator `String`
//! rich index, codegen `u8` parent-array) on the five canonical
//! topologies: sibling, nested, external-self, local-self, cross-parallel.
//!
//! This file re-derives the *expected* answers from first principles (the
//! UML 2.5.1 walk-up-the-parent-pointers rule + B-09 self-transition
//! lift), independent of any single old implementation, so it is a true
//! oracle rather than a snapshot of one impl's behaviour. The per-crate
//! adoption tests additionally pin the real carriers.

use std::collections::HashMap;

use fsm_ir::lca::{ancestors, effective_lca, lca_inclusive, ParentResolver};
use fsm_ir::{SourceLocation, Span, TransitionKind, TransitionObject, Trigger};

/// `String`-keyed resolver mirroring the analyzer / simulator carrier
/// shape: region IDs appear in the parent chain; a missing entry is the
/// root (the upward walk stops on `None`).
struct MapResolver(HashMap<String, String>);

impl ParentResolver for MapResolver {
    type Id = String;
    fn parent(&self, id: &String) -> Option<String> {
        self.0.get(id).cloned()
    }
}

/// `u8`-array resolver mirroring the codegen carrier shape: index 0 is the
/// `ROOT_SENTINEL`, which is its own parent; termination is via `is_root`,
/// NOT via `None`. Regions are *not* present in this chain — a state's
/// parent is the enclosing composite/parallel state index.
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
        loc: SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1),
    }
}

/// String-carrier topology (analyzer/simulator shape):
///
/// ```text
/// r-root
///  └ s-outer            (composite)
///     └ r-outer
///        ├ s-a          (simple sibling)
///        ├ s-b          (simple sibling)
///        └ s-mid        (composite)
///           └ r-mid
///              └ s-leaf (simple, deeply nested)
/// ```
fn string_topology() -> MapResolver {
    let pairs = [
        ("s-outer", "r-root"),
        ("r-outer", "s-outer"),
        ("s-a", "r-outer"),
        ("s-b", "r-outer"),
        ("s-mid", "r-outer"),
        ("r-mid", "s-mid"),
        ("s-leaf", "r-mid"),
    ];
    MapResolver(
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    )
}

#[test]
fn string_sibling_lca_is_their_region() {
    let r = string_topology();
    assert_eq!(
        lca_inclusive(&"s-a".to_string(), &"s-b".to_string(), &r),
        "r-outer"
    );
}

#[test]
fn string_nested_lca_is_deepest_common_ancestor() {
    let r = string_topology();
    assert_eq!(
        lca_inclusive(&"s-leaf".to_string(), &"s-a".to_string(), &r),
        "r-outer"
    );
    // Ancestor chain order is leaf→root, self at index 0.
    assert_eq!(
        ancestors(&"s-leaf".to_string(), &r),
        vec!["s-leaf", "r-mid", "s-mid", "r-outer", "s-outer", "r-root"]
    );
}

#[test]
fn string_external_self_lifts_to_containing_region() {
    let r = string_topology();
    let t = trans("s-leaf", "s-leaf", TransitionKind::External);
    // B-09: external `S -> S` ⇒ effective LCA = parent(S) = r-mid.
    assert_eq!(effective_lca(&t, &r, |s| s.to_string()), "r-mid");
}

#[test]
fn string_local_self_stays_at_self() {
    let r = string_topology();
    let t = trans("s-leaf", "s-leaf", TransitionKind::Local);
    // Local self-transition: base lca_inclusive(S,S) == S (no lift).
    assert_eq!(effective_lca(&t, &r, |s| s.to_string()), "s-leaf");
}

#[test]
fn string_cross_parallel_lca_is_the_parallel_state() {
    // r-root → s-par (parallel) → {r-x → s-x, r-y → s-y}
    let pairs = [
        ("s-par", "r-root"),
        ("r-x", "s-par"),
        ("r-y", "s-par"),
        ("s-x", "r-x"),
        ("s-y", "r-y"),
    ];
    let r = MapResolver(
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    );
    assert_eq!(
        lca_inclusive(&"s-x".to_string(), &"s-y".to_string(), &r),
        "s-par"
    );
}

/// `u8`-carrier topology (codegen shape — regions collapsed):
///
/// ```text
/// 0 ROOT
/// 1 Op       (composite, parent 0)
/// 2 Running  (simple,    parent 1)
/// 3 Faulted  (simple,    parent 0)
/// 4 Par      (parallel,  parent 0)
/// 5 X        (simple,    parent 4)
/// 6 Y        (simple,    parent 4)
/// ```
fn u8_topology() -> ArrResolver {
    ArrResolver(vec![0, 0, 1, 0, 0, 4, 4])
}

#[test]
fn u8_sibling_lca_is_root() {
    let r = u8_topology();
    // Running(2) & Faulted(3): chains [2,1,0] ∩ [3,0] ⇒ 0.
    assert_eq!(lca_inclusive(&2u8, &3u8, &r), 0);
}

#[test]
fn u8_nested_lca() {
    let r = u8_topology();
    assert_eq!(ancestors(&2u8, &r), vec![2, 1, 0]);
    // Running(2) & Op(1): 1 is an ancestor of 2 ⇒ LCA == 1.
    assert_eq!(lca_inclusive(&2u8, &1u8, &r), 1);
}

#[test]
fn u8_external_self_lifts_to_parent_index() {
    let r = u8_topology();
    let t = trans("s-running", "s-running", TransitionKind::External);
    // parent(2) == 1 (Op) — region is skipped in the codegen carrier.
    assert_eq!(
        effective_lca(&t, &r, |id| if id == "s-running" { 2 } else { 0 }),
        1
    );
}

#[test]
fn u8_local_self_stays_at_self_index() {
    let r = u8_topology();
    let t = trans("s-running", "s-running", TransitionKind::Local);
    assert_eq!(
        effective_lca(&t, &r, |id| if id == "s-running" { 2 } else { 0 }),
        2
    );
}

#[test]
fn u8_cross_parallel_lca_is_the_parallel_index() {
    let r = u8_topology();
    // X(5) & Y(6): chains [5,4,0] ∩ [6,4,0] ⇒ 4 (Par).
    assert_eq!(lca_inclusive(&5u8, &6u8, &r), 4);
}

#[test]
fn malformed_no_common_ancestor_falls_back_to_a() {
    // Two disjoint trees — no shared ancestor. Every prior impl returned
    // `a` itself rather than panicking; preserve that.
    let pairs = [("s-a", "r-1"), ("s-b", "r-2")];
    let r = MapResolver(
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    );
    assert_eq!(
        lca_inclusive(&"s-a".to_string(), &"s-b".to_string(), &r),
        "s-a"
    );
}
