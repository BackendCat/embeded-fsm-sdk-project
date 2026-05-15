//! Lowering integration tests — snapshot the lowered IR JSON shape for
//! representative sources. Verifies `transitions[].kind` is populated and
//! history `defaultTarget` is non-null even when the AST omits it (analyzer
//! fills with `""` and emits FSM-E0111 separately).

use fsm_analyzer::analyze;
use fsm_ir::{HistoryKind, StateNode, TimerKind, TransitionKind, Trigger};
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

/// Regression anchor for W7-FU-2 (Doc 00 §11.27): a transition with **no
/// `priority` clause** lowers to [`fsm_ir::DEFAULT_TRANSITION_PRIORITY`]
/// (100), the value Doc 04 §8.6 and Doc 09 §6 specify.
///
/// This test previously asserted `0` (`ir_default_priority_is_zero`) — it
/// was pinning the lowering *bug* (`lower/state.rs` `.unwrap_or(0)`), not a
/// spec-correct expectation. Under min-wins selection (Doc 08 §4.2) the
/// default must be LOW priority so an explicit small number can float a
/// specific transition above the unprioritized herd; a `0` default made an
/// unprioritized transition out-prioritise every explicitly-deprioritized
/// one. Asserting `100` here makes this the FAILS-on-old / PASSES-on-new
/// regression proof (§5.1) and the §5.4 unit pin.
#[test]
fn transition_without_priority_clause_lowers_to_spec_default_100() {
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
    assert_eq!(
        s.transitions[0].priority,
        fsm_ir::DEFAULT_TRANSITION_PRIORITY,
        "an unprioritized transition must lower to the Doc 04 §8.6 / Doc 09 \
         §6 default of 100, not 0"
    );
    assert_eq!(s.transitions[0].priority, 100);
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

#[test]
fn after_decl_lowers_to_timer_plus_after_triggered_transition() {
    // P0-4: `after N ms -> X` must produce BOTH a TimerObject and a
    // matching TransitionObject whose trigger is `Trigger::After { timer_id
    // }`. Pre-fix, only the timer was produced and the transition vanished
    // into a `trigger: None` slot indistinguishable from completion.
    let src = "language fsm 2.0\nfeature timers\nmachine M { events { E } initial A state A { after 100 ms -> B } state B {} }";
    let pr = parse(src);
    let ir = analyze(&pr).ir.unwrap();
    let s = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| match s {
            StateNode::Simple(s) if s.name == "A" => Some(s),
            _ => None,
        })
        .expect("state A");
    assert_eq!(s.timers.len(), 1, "state A should own one timer");
    let timer = &s.timers[0];
    assert!(matches!(timer.kind, TimerKind::After));
    assert_eq!(timer.duration_ms, 100);

    let after_t = s
        .transitions
        .iter()
        .find(|t| matches!(t.trigger, Some(Trigger::After { .. })))
        .expect("after-triggered transition");
    if let Some(Trigger::After {
        duration_ms,
        timer_id,
    }) = &after_t.trigger
    {
        assert_eq!(*duration_ms, 100);
        assert_eq!(
            timer_id, &timer.id,
            "timer_id on Trigger::After must reference the owning TimerObject"
        );
    } else {
        unreachable!()
    }
}

#[test]
fn every_decl_lowers_to_timer_plus_every_triggered_transition() {
    let src = "language fsm 2.0\nfeature timers\nmachine M { events { E } initial A state A { every 50 ms -> A } }";
    let pr = parse(src);
    let ir = analyze(&pr).ir.unwrap();
    let s = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| match s {
            StateNode::Simple(s) if s.name == "A" => Some(s),
            _ => None,
        })
        .expect("state A");
    assert_eq!(s.timers.len(), 1);
    let timer = &s.timers[0];
    assert!(matches!(timer.kind, TimerKind::Every));
    let every_t = s
        .transitions
        .iter()
        .find(|t| matches!(t.trigger, Some(Trigger::Every { .. })))
        .expect("every-triggered transition");
    if let Some(Trigger::Every {
        period_ms,
        timer_id,
    }) = &every_t.trigger
    {
        assert_eq!(*period_ms, 50);
        assert_eq!(timer_id, &timer.id);
    } else {
        unreachable!()
    }
}

#[test]
fn every_internal_decl_lowers_to_internal_kind_transition() {
    // `every N ms : action_block` (no target) — analyzer must still emit a
    // transition so the action runs on each fire, but with TransitionKind::
    // Internal so no exit/entry sequence executes.
    let src = "language fsm 2.0\nfeature timers\nextern tick()\nmachine M { initial A state A { every 25 ms : tick() } }";
    let pr = parse(src);
    let ir = analyze(&pr).ir.unwrap();
    let s = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| match s {
            StateNode::Simple(s) if s.name == "A" => Some(s),
            _ => None,
        })
        .expect("state A");
    assert_eq!(s.timers.len(), 1);
    assert!(matches!(s.timers[0].kind, TimerKind::EveryInternal));
    let internal_t = s
        .transitions
        .iter()
        .find(|t| matches!(t.trigger, Some(Trigger::Every { .. })))
        .expect("every-internal transition");
    assert!(matches!(internal_t.kind, TransitionKind::Internal));
}
