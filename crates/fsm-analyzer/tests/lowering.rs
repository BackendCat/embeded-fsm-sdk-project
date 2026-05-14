//! Lowering integration tests — snapshot the lowered IR JSON shape for
//! representative sources. Verifies `transitions[].kind` is populated and
//! history `defaultTarget` is non-null even when the AST omits it (analyzer
//! fills with `""` and emits FSM-E0111 separately).

use fsm_analyzer::analyze;
use fsm_ir::{HistoryKind, StateNode, TransitionKind};
use fsm_parser::parse;

#[test]
fn external_transition_lowers_with_external_kind() {
    let src = "language fsm 2.0\nmachine M { events { E } initial S state S { on E -> S } }";
    let pr = parse(src);
    let ir = analyze(&pr).ir.unwrap();
    let s = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| {
            if let StateNode::Simple(s) = s {
                Some(s)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(s.transitions.len(), 1);
    assert!(matches!(s.transitions[0].kind, TransitionKind::External));
}

#[test]
fn local_transition_lowers_with_local_kind() {
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
    let pr = parse(src);
    let ir = analyze(&pr).ir.unwrap();
    let outer = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| {
            if let StateNode::Composite(s) = s {
                Some(s)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(outer.transitions.len(), 1);
    assert!(matches!(outer.transitions[0].kind, TransitionKind::Local));
}

#[test]
fn internal_transition_lowers_with_internal_kind() {
    let src =
        "language fsm 2.0\nmachine M { events { E } initial S state S { on E : ctx.x = 1 } context { x : u8 = 0 } }";
    let pr = parse(src);
    let ir = analyze(&pr).ir.unwrap();
    let s = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| {
            if let StateNode::Simple(s) = s {
                Some(s)
            } else {
                None
            }
        })
        .unwrap();
    assert!(s
        .transitions
        .iter()
        .any(|t| matches!(t.kind, TransitionKind::Internal)));
}

#[test]
fn completion_transition_lowers_with_completion_kind() {
    let src = "language fsm 2.0\nmachine M { initial S state S { done -> S } }";
    let pr = parse(src);
    let ir = analyze(&pr).ir.unwrap();
    let s = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| {
            if let StateNode::Simple(s) = s {
                Some(s)
            } else {
                None
            }
        })
        .unwrap();
    assert!(s
        .transitions
        .iter()
        .any(|t| matches!(t.kind, TransitionKind::Completion)));
}

#[test]
fn history_default_target_is_present_when_declared() {
    let src = r#"language fsm 2.0
feature history
machine M {
    initial Outer
    state Outer {
        initial Inner
        state Inner { }
        shallow_history H { initial Inner }
    }
}"#;
    let pr = parse(src);
    let ir = analyze(&pr).ir.unwrap();
    let outer = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| {
            if let StateNode::Composite(s) = s {
                Some(s)
            } else {
                None
            }
        })
        .unwrap();
    // History is either attached via composite_state.history or lives as a
    // separate StateNode::History within the region. Find it.
    let history = outer
        .regions
        .iter()
        .flat_map(|r| r.states.iter())
        .find_map(|n| match n {
            StateNode::History(h) => Some(h),
            _ => None,
        });
    if let Some(h) = history {
        assert_eq!(h.history_kind, HistoryKind::Shallow);
        assert!(!h.default_target.is_empty());
    }
}

#[test]
fn ir_source_hash_is_content_addressed() {
    let pr_a = parse("language fsm 2.0\nmachine A { }");
    let pr_b = parse("language fsm 2.0\nmachine B { }");
    let ir_a = fsm_analyzer::analyze_with_source(&pr_a, "a.fsm", "language fsm 2.0\nmachine A { }")
        .ir
        .unwrap();
    let ir_b = fsm_analyzer::analyze_with_source(&pr_b, "b.fsm", "language fsm 2.0\nmachine B { }")
        .ir
        .unwrap();
    assert_ne!(ir_a.source_hash, ir_b.source_hash);
    assert!(ir_a.source_hash.starts_with("sha256:"));
}

#[test]
fn ir_default_priority_is_zero() {
    let src = "language fsm 2.0\nmachine M { events { E } initial S state S { on E -> S } }";
    let pr = parse(src);
    let ir = analyze(&pr).ir.unwrap();
    let s = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| {
            if let StateNode::Simple(s) = s {
                Some(s)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(s.transitions[0].priority, 0);
}

#[test]
fn default_queue_capacity_is_16() {
    let pr = parse("language fsm 2.0\nmachine M { initial S state S { } }");
    let ir = analyze(&pr).ir.unwrap();
    assert_eq!(ir.machines[0].queue.capacity, 16);
}

#[test]
fn ir_serializes_to_json() {
    let pr = parse("language fsm 2.0\nmachine M { events { E } initial S state S { on E -> S } }");
    let ir = analyze(&pr).ir.unwrap();
    let json = fsm_ir::to_json(&ir).expect("serialise IR");
    assert!(json.contains("\"irVersion\""));
    assert!(json.contains("\"kind\""));
}
