//! Round-trip integration tests for the canonical IR.
//!
//! Each test constructs a representative IR document, serialises it, parses
//! it back, and asserts equality. Together they exercise every wire-format
//! shape described in Doc 09 + Doc 00 §7.4.

#![allow(deprecated)]

use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::*;

fn loc() -> SourceLocation {
    SourceLocation::new("tests/roundtrip.fsm", Span::new(0, 1), 1, 1)
}

fn empty_region(id: &str) -> RegionObject {
    RegionObject {
        id: id.into(),
        stable_id: None,
        name: id.into(),
        initial: "ps-initial-0".into(),
        states: vec![],
        priority: 0,
        loc: loc(),
    }
}

fn empty_machine(name: &str) -> MachineObject {
    MachineObject {
        id: format!("m-{name}"),
        stable_id: name.into(),
        name: name.into(),
        context: ContextSchema::default(),
        events: vec![],
        externs: vec![],
        root: empty_region("r-root"),
        submachines: vec![],
        consts: vec![],
        imports: vec![],
        features: vec![],
        queue: QueueConfig {
            capacity: 16,
            overflow_policy: OverflowPolicy::Assert,
            loc: loc(),
        },
        targets: vec![],
        loc: loc(),
    }
}

fn round_trip(ir: &Ir) {
    let json = to_json(ir).expect("serialise");
    let back = from_json(&json).expect("deserialise");
    assert_eq!(ir, &back);
}

// ---------------------------------------------------------------------------
// 1. Empty IR
// ---------------------------------------------------------------------------

#[test]
fn empty_ir_round_trip() {
    round_trip(&Ir::default());
}

// ---------------------------------------------------------------------------
// 2. Motor — 3 simple states + 1 transition each (the task brief example)
// ---------------------------------------------------------------------------

#[test]
fn motor_three_states_round_trip() {
    let mk_simple = |id: &str, target: &str| {
        StateNode::Simple(SimpleState {
            id: id.into(),
            stable_id: format!("Motor:state:{id}"),
            name: id.into(),
            entry: vec![],
            exit: vec![],
            transitions: vec![TransitionObject {
                id: format!("t-{id}-{target}"),
                stable_id: format!("Motor:transition:{id}-{target}"),
                source: id.into(),
                target: target.into(),
                trigger: Some(Trigger::Event {
                    event_id: "ev-tick".into(),
                    payload_binding: None,
                }),
                guard: None,
                actions: vec![],
                priority: 100,
                kind: TransitionKind::External,
                internal: false,
                loc: loc(),
            }],
            timers: vec![],
            defers: vec![],
            loc: loc(),
        })
    };

    let mut m = empty_machine("Motor");
    m.events.push(EventObject {
        id: "ev-tick".into(),
        stable_id: "Motor:event:TICK".into(),
        name: "TICK".into(),
        payload: vec![],
        loc: loc(),
    });
    m.root.states = vec![
        StateNode::Initial(InitialPseudo {
            id: "ps-initial-0".into(),
            target: "Idle".into(),
            loc: loc(),
        }),
        mk_simple("Idle", "Running"),
        mk_simple("Running", "Faulted"),
        mk_simple("Faulted", "Idle"),
    ];

    let ir = Ir {
        machines: vec![m],
        source_hash: "sha256:motor".into(),
        source_files: vec!["motor.fsm".into()],
        ..Ir::default()
    };
    round_trip(&ir);
}

// ---------------------------------------------------------------------------
// 3. Parallel state
// ---------------------------------------------------------------------------

#[test]
fn parallel_state_round_trip() {
    let region_a = RegionObject {
        id: "r-a".into(),
        stable_id: None,
        name: "A".into(),
        initial: "ps-a-init".into(),
        priority: 0,
        states: vec![StateNode::Initial(InitialPseudo {
            id: "ps-a-init".into(),
            target: "s-a1".into(),
            loc: loc(),
        })],
        loc: loc(),
    };
    let region_b = RegionObject {
        id: "r-b".into(),
        stable_id: None,
        name: "B".into(),
        initial: "ps-b-init".into(),
        priority: 1,
        states: vec![StateNode::Initial(InitialPseudo {
            id: "ps-b-init".into(),
            target: "s-b1".into(),
            loc: loc(),
        })],
        loc: loc(),
    };
    let parallel = StateNode::Parallel(ParallelState {
        id: "s-monitor".into(),
        stable_id: "M:state:Monitor".into(),
        name: "Monitor".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        regions: vec![region_a, region_b],
        loc: loc(),
    });

    let mut m = empty_machine("M");
    m.root.states.push(parallel);
    let ir = Ir {
        machines: vec![m],
        ..Ir::default()
    };
    round_trip(&ir);
}

// ---------------------------------------------------------------------------
// 4. Composite + history
// ---------------------------------------------------------------------------

#[test]
fn composite_with_history_round_trip() {
    let inner_region = RegionObject {
        id: "r-inner".into(),
        stable_id: None,
        name: "Inner".into(),
        initial: "ps-inner-init".into(),
        priority: 0,
        states: vec![StateNode::Initial(InitialPseudo {
            id: "ps-inner-init".into(),
            target: "s-inner-a".into(),
            loc: loc(),
        })],
        loc: loc(),
    };
    let composite = StateNode::Composite(CompositeState {
        id: "s-comp".into(),
        stable_id: "M:state:Operational".into(),
        name: "Operational".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        regions: vec![inner_region],
        history: Some(HistoryObject {
            id: "ps-history-0".into(),
            stable_id: "M:history:H".into(),
            history_kind: HistoryKind::Deep,
            // B-14: never None — must be a real target.
            default_target: "s-inner-a".into(),
            loc: loc(),
        }),
        loc: loc(),
    });

    let mut m = empty_machine("M");
    m.root.states.push(composite);
    let ir = Ir {
        machines: vec![m],
        ..Ir::default()
    };
    round_trip(&ir);
}

// ---------------------------------------------------------------------------
// 5. Submachine reference (cross-machine)
// ---------------------------------------------------------------------------

#[test]
fn submachine_ref_round_trip() {
    let sm_ref = StateNode::Submachine(SubmachineRef {
        id: "s-link".into(),
        stable_id: "Parent:state:Link".into(),
        name: "Link".into(),
        submachine_id: "m-conn".into(),
        entry_points: vec![("Start".into(), "s-conn-start".into())],
        exit_points: vec![("Done".into(), "s-idle".into())],
        transitions: vec![],
        loc: loc(),
    });

    let mut parent = empty_machine("Parent");
    parent.root.states.push(sm_ref);
    // Nested submachine declaration sits in parent.submachines.
    parent.submachines.push(empty_machine("Conn"));

    let ir = Ir {
        machines: vec![parent],
        ..Ir::default()
    };
    round_trip(&ir);
}

// ---------------------------------------------------------------------------
// 6. Imports
// ---------------------------------------------------------------------------

#[test]
fn imports_round_trip() {
    let mut m = empty_machine("M");
    m.imports.push(ImportDecl {
        path: "common/events.fsm".into(),
        alias: Some("Common".into()),
        named_imports: None,
        loc: loc(),
    });
    m.imports.push(ImportDecl {
        path: "shared/types.fsm".into(),
        alias: None,
        named_imports: Some(vec!["PacketType".into(), "ErrorCode".into()]),
        loc: loc(),
    });
    round_trip(&Ir {
        machines: vec![m],
        ..Ir::default()
    });
}

// ---------------------------------------------------------------------------
// 7. Const declarations
// ---------------------------------------------------------------------------

#[test]
fn const_decls_round_trip() {
    let mut m = empty_machine("M");
    m.consts.push(ConstDecl {
        id: "c-max".into(),
        stable_id: "M:const:MAX_RETRIES".into(),
        name: "MAX_RETRIES".into(),
        ty: Type::Primitive { name: "u8".into() },
        value: Literal::Int(IntLit {
            value: 5,
            loc: None,
        }),
        loc: loc(),
    });
    round_trip(&Ir {
        machines: vec![m],
        ..Ir::default()
    });
}

// ---------------------------------------------------------------------------
// 8. Feature flags
// ---------------------------------------------------------------------------

#[test]
fn feature_flags_round_trip() {
    let mut m = empty_machine("M");
    m.features = vec![
        FeatureDecl {
            name: "hsm".into(),
            loc: loc(),
        },
        FeatureDecl {
            name: "parallel".into(),
            loc: loc(),
        },
        FeatureDecl {
            name: "history".into(),
            loc: loc(),
        },
    ];
    round_trip(&Ir {
        machines: vec![m],
        ..Ir::default()
    });
}

// ---------------------------------------------------------------------------
// 9. Cast expression in a guard's extern_call arguments
// ---------------------------------------------------------------------------

#[test]
fn cast_expression_in_action_round_trip() {
    // ctx.speed_u32 = ctx.speed_u16 as u32
    let assign = Statement::Assign {
        target: FieldRef::Ctx {
            field: "speed_u32".into(),
        },
        value: Expr::Cast(CastExpr {
            operand: Box::new(Expr::FieldRef {
                field_ref: FieldRef::Ctx {
                    field: "speed_u16".into(),
                },
            }),
            target_type: Type::Primitive { name: "u32".into() },
            loc: loc(),
        }),
    };
    let mut m = empty_machine("M");
    m.root.states.push(StateNode::Simple(SimpleState {
        id: "s-only".into(),
        stable_id: "M:state:Only".into(),
        name: "Only".into(),
        entry: vec![assign],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    }));
    round_trip(&Ir {
        machines: vec![m],
        ..Ir::default()
    });
}

// ---------------------------------------------------------------------------
// 10. enum_variant literal in a guard's RHS
// ---------------------------------------------------------------------------

#[test]
fn enum_variant_in_guard_round_trip() {
    // [payload.kind == PacketType.HEARTBEAT] on a transition.
    let guard = GuardExpr::FieldCmp {
        lhs: FieldRef::Payload {
            field: "kind".into(),
        },
        op: CmpOp::Eq,
        rhs: GuardOperand::Literal(Literal::EnumVariant(EnumVariantLit {
            enum_name: "PacketType".into(),
            variant_name: "HEARTBEAT".into(),
            loc: None,
        })),
    };
    let t = TransitionObject {
        id: "t-0".into(),
        stable_id: "M:transition:0".into(),
        source: "s-from".into(),
        target: "s-to".into(),
        trigger: Some(Trigger::Event {
            event_id: "ev-packet".into(),
            payload_binding: Some("p".into()),
        }),
        guard: Some(guard),
        actions: vec![],
        priority: 100,
        kind: TransitionKind::External,
        internal: false,
        loc: loc(),
    };
    let mut m = empty_machine("M");
    m.root.states.push(StateNode::Simple(SimpleState {
        id: "s-from".into(),
        stable_id: "M:state:From".into(),
        name: "From".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![t],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    }));
    round_trip(&Ir {
        machines: vec![m],
        ..Ir::default()
    });
}

// ---------------------------------------------------------------------------
// 11. Invalid version rejected
// ---------------------------------------------------------------------------

#[test]
fn invalid_major_version_rejected() {
    // Hand-crafted JSON to keep this test specific to the version-check
    // path, not derived from a Rust-side construct.
    let s = r#"{
        "irVersion": "2.0.0",
        "sourceHash": "",
        "sourceFiles": [],
        "machines": [],
        "diagnostics": []
    }"#;
    assert!(
        matches!(
            from_json(s),
            Err(IrJsonError::IncompatibleMajorVersion { .. })
        ),
        "v2 IR documents must be rejected per Doc 09 §17"
    );
}

// ---------------------------------------------------------------------------
// 12. TransitionKind variants serialise as documented
// ---------------------------------------------------------------------------

#[test]
fn all_transition_kinds_serialize_correctly() {
    for (kind, wire) in [
        (TransitionKind::External, "external"),
        (TransitionKind::Local, "local"),
        (TransitionKind::Internal, "internal"),
        (TransitionKind::Completion, "completion"),
    ] {
        let s = serde_json::to_string(&kind).unwrap();
        assert_eq!(s, format!("\"{wire}\""));
        let back: TransitionKind = serde_json::from_str(&s).unwrap();
        assert_eq!(kind, back);
    }
}
