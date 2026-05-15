//! Shared IR builders for fsm-codegen-c integration tests.
//!
//! Each `tests/*.rs` integration test target gets its own crate, so this
//! module is included via `#[path = "common/mod.rs"]` from each test file.

// each integration target uses a subset
#![allow(dead_code)]
// Same idiom-based reason as the `dead_code` allow: `tests/common/mod.rs` is
// recompiled per test binary, so the workspace `unreachable_pub` lint (Doc 00
// §11.4x) flags shared helpers as unreachable per-binary though they are a
// real cross-test API. One module attribute, not per-item noise.
#![allow(unreachable_pub)]

use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    BoolLit, CompositeState, ContextField, ContextSchema, EventObject, ExternObject, FinalState,
    InitialPseudo, IntLit, Ir, Literal, MachineObject, OverflowPolicy, ParallelState, Param,
    QueueConfig, RegionObject, SimpleState, StateNode, TransitionKind, TransitionObject, Trigger,
    Type,
};

pub fn loc() -> SourceLocation {
    SourceLocation::new("motor.fsm", Span::new(0, 1), 1, 1)
}

#[allow(deprecated)]
pub fn motor_transition(
    id: &str,
    source: &str,
    target: &str,
    trigger_event_id: Option<&str>,
    priority: u16,
    kind: TransitionKind,
) -> TransitionObject {
    TransitionObject {
        id: id.into(),
        stable_id: format!("Motor:transition:{}", id),
        source: source.into(),
        target: target.into(),
        trigger: trigger_event_id.map(|e| Trigger::Event {
            event_id: e.into(),
            payload_binding: None,
        }),
        guard: None,
        actions: vec![],
        priority,
        kind,
        internal: false,
        hint: None,
        loc: loc(),
    }
}

pub fn motor_event(id: &str, name: &str, payload: Vec<Param>) -> EventObject {
    EventObject {
        id: id.into(),
        stable_id: format!("Motor:event:{}", name),
        name: name.into(),
        payload,
        loc: loc(),
    }
}

pub fn motor_extern_action(id: &str, name: &str) -> ExternObject {
    ExternObject {
        id: id.into(),
        stable_id: format!("Motor:extern:{}", name),
        name: name.into(),
        pure: false,
        params: vec![],
        return_type: None,
        loc: loc(),
    }
}

/// Three-state Motor: Idle → Running → Faulted → Idle. Documents typical
/// codegen surface: events, externs, transitions, an initial pseudo, a
/// context field.
pub fn motor_ir() -> Ir {
    let make_simple = |id: &str, name: &str, transitions: Vec<TransitionObject>| {
        StateNode::Simple(SimpleState {
            id: id.into(),
            stable_id: format!("Motor:state:{}", name),
            name: name.into(),
            entry: vec![],
            exit: vec![],
            transitions,
            timers: vec![],
            defers: vec![],
            loc: loc(),
        })
    };

    let idle_to_running = motor_transition(
        "t-idle-running",
        "s-idle",
        "s-running",
        Some("e-start"),
        100,
        TransitionKind::External,
    );
    let running_to_faulted = motor_transition(
        "t-running-faulted",
        "s-running",
        "s-faulted",
        Some("e-fault"),
        100,
        TransitionKind::External,
    );
    let running_to_idle = motor_transition(
        "t-running-idle",
        "s-running",
        "s-idle",
        Some("e-stop"),
        100,
        TransitionKind::External,
    );
    let faulted_to_idle = motor_transition(
        "t-faulted-idle",
        "s-faulted",
        "s-idle",
        Some("e-reset"),
        100,
        TransitionKind::External,
    );

    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "".into(),
        source_files: vec!["motor.fsm".into()],
        machines: vec![MachineObject {
            id: "m-motor".into(),
            stable_id: "Motor".into(),
            name: "Motor".into(),
            context: ContextSchema {
                fields: vec![
                    ContextField {
                        id: "f-speed".into(),
                        name: "speed".into(),
                        ty: Type::Primitive { name: "u16".into() },
                        default: Some(Literal::Int(IntLit {
                            value: 0,
                            loc: None,
                        })),
                        loc: loc(),
                    },
                    ContextField {
                        id: "f-running".into(),
                        name: "running".into(),
                        ty: Type::Primitive {
                            name: "bool".into(),
                        },
                        default: Some(Literal::Bool(BoolLit {
                            value: false,
                            loc: None,
                        })),
                        loc: loc(),
                    },
                ],
            },
            events: vec![
                motor_event("e-start", "START", vec![]),
                motor_event("e-stop", "STOP", vec![]),
                motor_event("e-fault", "FAULT", vec![]),
                motor_event("e-reset", "RESET", vec![]),
            ],
            externs: vec![
                motor_extern_action("ext-start", "startMotor"),
                motor_extern_action("ext-stop", "stopMotor"),
            ],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-initial-0".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-initial-0".into(),
                        target: "s-idle".into(),
                        loc: loc(),
                    }),
                    make_simple("s-idle", "Idle", vec![idle_to_running]),
                    make_simple(
                        "s-running",
                        "Running",
                        vec![running_to_faulted, running_to_idle],
                    ),
                    make_simple("s-faulted", "Faulted", vec![faulted_to_idle]),
                ],
                priority: 0,
                loc: loc(),
            },
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            queue: QueueConfig {
                capacity: 8,
                overflow_policy: OverflowPolicy::Assert,
                loc: loc(),
            },
            targets: vec![],
            loc: loc(),
        }],
        diagnostics: vec![],
    }
}

/// Composite Motor: Operational composite holds a Running leaf; FAULT on
/// the composite parent transitions to Error. Validates B-10 leaf-to-root
/// dispatch — the FAULT event arriving in Running must walk up to
/// Operational to find the transition.
pub fn hierarchical_motor_ir() -> Ir {
    let mut ir = motor_ir();
    let machine = &mut ir.machines[0];
    // NOTE (v1.1-W0 / PD-2): do NOT re-push `e-fault` here. `motor_ir()`
    // already declares the FAULT event (see its `events` vec). The previous
    // extra `machine.events.push(motor_event("e-fault", "FAULT", vec![]))`
    // produced an IR with a *duplicate* event id, which codegen faithfully
    // lowered to a duplicate C enumerator (`MOTOR_EVENT_FAULT = 2` and
    // `= 4`) — uncompilable C. The defect stayed invisible for the life of
    // this fixture because the only B-10 tests were symbol-presence
    // `.contains()` checks that never invoked gcc (exactly the P0-1-class
    // masking PD-2 pays down). The new gcc-compile-RUN test below would not
    // even compile with the duplicate, so it is removed here.

    let running = StateNode::Simple(SimpleState {
        id: "s-op-running".into(),
        stable_id: "Motor:state:Op.Running".into(),
        name: "Op.Running".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    });
    let op_fault_to_error = motor_transition(
        "t-op-fault-error",
        "s-op",
        "s-error",
        Some("e-fault"),
        100,
        TransitionKind::External,
    );
    let operational = StateNode::Composite(CompositeState {
        id: "s-op".into(),
        stable_id: "Motor:state:Operational".into(),
        name: "Operational".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![op_fault_to_error],
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
                    target: "s-op-running".into(),
                    loc: loc(),
                }),
                running,
            ],
            priority: 0,
            loc: loc(),
        }],
        history: None,
        loc: loc(),
    });
    let error = StateNode::Simple(SimpleState {
        id: "s-error".into(),
        stable_id: "Motor:state:Error".into(),
        name: "Error".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    });

    machine.root = RegionObject {
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
            operational,
            error,
        ],
        priority: 0,
        loc: loc(),
    };
    ir
}

/// Parallel Motor with two regions, each ending in Final. Validates B-08
/// parallel completion logic.
pub fn parallel_motor_ir() -> Ir {
    let mut ir = motor_ir();
    let machine = &mut ir.machines[0];
    machine
        .events
        .push(motor_event("e-region-a-done", "A_DONE", vec![]));
    machine
        .events
        .push(motor_event("e-region-b-done", "B_DONE", vec![]));

    let region_a_init = StateNode::Initial(InitialPseudo {
        id: "ps-a-init".into(),
        target: "s-a-active".into(),
        loc: loc(),
    });
    let region_a_done = StateNode::Final(FinalState {
        id: "s-a-final".into(),
        stable_id: "Motor:state:A.Final".into(),
        name: "AFinal".into(),
        loc: loc(),
    });
    let region_a_active = StateNode::Simple(SimpleState {
        id: "s-a-active".into(),
        stable_id: "Motor:state:A.Active".into(),
        name: "A_Active".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![motor_transition(
            "t-a-done",
            "s-a-active",
            "s-a-final",
            Some("e-region-a-done"),
            100,
            TransitionKind::External,
        )],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    });
    let region_b_init = StateNode::Initial(InitialPseudo {
        id: "ps-b-init".into(),
        target: "s-b-active".into(),
        loc: loc(),
    });
    let region_b_done = StateNode::Final(FinalState {
        id: "s-b-final".into(),
        stable_id: "Motor:state:B.Final".into(),
        name: "BFinal".into(),
        loc: loc(),
    });
    let region_b_active = StateNode::Simple(SimpleState {
        id: "s-b-active".into(),
        stable_id: "Motor:state:B.Active".into(),
        name: "B_Active".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![motor_transition(
            "t-b-done",
            "s-b-active",
            "s-b-final",
            Some("e-region-b-done"),
            100,
            TransitionKind::External,
        )],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    });

    let parallel = StateNode::Parallel(ParallelState {
        id: "s-monitor".into(),
        stable_id: "Motor:state:Monitor".into(),
        name: "Monitor".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        regions: vec![
            RegionObject {
                id: "r-a".into(),
                stable_id: None,
                name: "RegionA".into(),
                initial: "ps-a-init".into(),
                states: vec![region_a_init, region_a_active, region_a_done],
                priority: 0,
                loc: loc(),
            },
            RegionObject {
                id: "r-b".into(),
                stable_id: None,
                name: "RegionB".into(),
                initial: "ps-b-init".into(),
                states: vec![region_b_init, region_b_active, region_b_done],
                priority: 1,
                loc: loc(),
            },
        ],
        loc: loc(),
    });

    machine.root.states.push(parallel);
    ir
}
