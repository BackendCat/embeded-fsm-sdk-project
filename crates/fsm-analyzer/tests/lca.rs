//! `effective_lca` integration tests.
//!
//! Each case constructs an IR via `analyze` and walks the machine to
//! recover the resulting transition kind and effective-LCA.

use fsm_analyzer::{analyze, lca::effective_lca, lca::LcaIndex};
use fsm_ir::{StateNode, TransitionKind};
use fsm_parser::parse;

fn machine_index(src: &str) -> (fsm_ir::Ir, LcaIndex) {
    let pr = parse(src);
    let ir = analyze(&pr).ir.unwrap();
    let idx = LcaIndex::build(&ir.machines[0]);
    (ir, idx)
}

#[test]
fn external_self_transition_lifts_lca_to_parent() {
    let src = "language fsm 2.0\nmachine M { events { E } initial S state S { on E -> S } }";
    let (ir, idx) = machine_index(src);
    let m = &ir.machines[0];
    let s = m
        .root
        .states
        .iter()
        .find_map(|n| {
            if let StateNode::Simple(s) = n {
                Some(s)
            } else {
                None
            }
        })
        .unwrap();
    let t = &s.transitions[0];
    assert!(matches!(t.kind, TransitionKind::External));
    let lca = effective_lca(t, &idx);
    // External self-transition: effective LCA is the parent region, not S.
    assert_ne!(lca, s.id);
}

#[test]
fn local_self_transition_keeps_lca_at_state() {
    // local self-transitions require a proper-descendant target.
    let src = r#"language fsm 2.0
machine M {
    events { E }
    initial Outer
    state Outer {
        initial Inner
        state Inner { }
        on E ~> Inner
    }
}"#;
    let (ir, idx) = machine_index(src);
    let m = &ir.machines[0];
    let outer = m
        .root
        .states
        .iter()
        .find_map(|n| {
            if let StateNode::Composite(s) = n {
                Some(s)
            } else {
                None
            }
        })
        .unwrap();
    let t = &outer.transitions[0];
    assert!(matches!(t.kind, TransitionKind::Local));
    let lca = effective_lca(t, &idx);
    // local: lca_inclusive(source, descendant_target) = source.
    assert_eq!(lca, outer.id);
}

#[test]
fn sibling_transition_lca_is_their_region() {
    let src =
        "language fsm 2.0\nmachine M { events { E } initial A state A { on E -> B } state B { } }";
    let (ir, idx) = machine_index(src);
    let m = &ir.machines[0];
    let a = m
        .root
        .states
        .iter()
        .find_map(|n| {
            if let StateNode::Simple(s) = n {
                if s.name == "A" {
                    Some(s)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap();
    let t = &a.transitions[0];
    let lca = effective_lca(t, &idx);
    // Both A and B sit directly under root region.
    assert_eq!(lca, m.root.id);
}
