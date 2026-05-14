//! Shared IR builders for integration tests. Kept terse — every helper
//! produces a minimal valid `fsm_ir::Ir` with one machine so tests focus on
//! semantics rather than the boilerplate of constructing dozens of
//! `SourceLocation` shells.

#![allow(dead_code)]

use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    CompositeState, ContextSchema, EventObject, FinalState, HistoryKind, HistoryObject,
    InitialPseudo, Ir, MachineObject, ParallelState, QueueConfig, RegionObject, SimpleState,
    StateNode, TimerKind, TimerObject, TransitionKind, TransitionObject, Trigger,
};

pub fn loc() -> SourceLocation {
    SourceLocation::new("test.fsm", Span::new(0, 1), 1, 1)
}

pub fn ir_one_machine(m: MachineObject) -> Ir {
    Ir {
        ir_version: fsm_ir::CURRENT_IR_VERSION.to_owned(),
        source_hash: "sha256:test".into(),
        source_files: vec!["test.fsm".into()],
        machines: vec![m],
        diagnostics: vec![],
    }
}

pub fn machine(name: &str, root: RegionObject) -> MachineObject {
    MachineObject {
        id: format!("m-{}", name.to_lowercase()),
        stable_id: name.to_string(),
        name: name.to_string(),
        context: ContextSchema::default(),
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

pub fn region(id: &str, initial_pseudo: &str, states: Vec<StateNode>) -> RegionObject {
    RegionObject {
        id: id.to_string(),
        stable_id: None,
        name: id.to_string(),
        initial: initial_pseudo.to_string(),
        states,
        priority: 0,
        loc: loc(),
    }
}

pub fn initial(id: &str, target: &str) -> StateNode {
    StateNode::Initial(InitialPseudo {
        id: id.to_string(),
        target: target.to_string(),
        loc: loc(),
    })
}

pub fn simple(id: &str) -> SimpleState {
    SimpleState {
        id: id.to_string(),
        stable_id: id.to_string(),
        name: id.to_string(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    }
}

pub fn final_state(id: &str) -> StateNode {
    StateNode::Final(FinalState {
        id: id.to_string(),
        stable_id: id.to_string(),
        name: String::new(),
        loc: loc(),
    })
}

pub fn composite(id: &str, regions: Vec<RegionObject>) -> CompositeState {
    CompositeState {
        id: id.to_string(),
        stable_id: id.to_string(),
        name: id.to_string(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        regions,
        history: None,
        loc: loc(),
    }
}

pub fn parallel(id: &str, regions: Vec<RegionObject>) -> ParallelState {
    ParallelState {
        id: id.to_string(),
        stable_id: id.to_string(),
        name: id.to_string(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        regions,
        loc: loc(),
    }
}

pub fn transition(
    id: &str,
    source: &str,
    target: &str,
    event_id: &str,
    kind: TransitionKind,
) -> TransitionObject {
    #[allow(deprecated)]
    TransitionObject {
        id: id.to_string(),
        stable_id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        trigger: Some(Trigger::Event {
            event_id: event_id.to_string(),
            payload_binding: None,
        }),
        guard: None,
        actions: vec![],
        priority: 100,
        kind,
        internal: matches!(kind, TransitionKind::Internal),
        loc: loc(),
    }
}

pub fn timer_transition(
    id: &str,
    source: &str,
    target: &str,
    duration_ms: u32,
    periodic: bool,
) -> TransitionObject {
    #[allow(deprecated)]
    TransitionObject {
        id: id.to_string(),
        stable_id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        trigger: Some(if periodic {
            Trigger::Every {
                period_ms: duration_ms,
            }
        } else {
            Trigger::After { duration_ms }
        }),
        guard: None,
        actions: vec![],
        priority: 100,
        kind: TransitionKind::External,
        internal: false,
        loc: loc(),
    }
}

pub fn timer(
    id: &str,
    owner_state: &str,
    target: &str,
    duration_ms: u32,
    periodic: bool,
) -> TimerObject {
    TimerObject {
        id: id.to_string(),
        stable_id: id.to_string(),
        kind: if periodic {
            TimerKind::Every
        } else {
            TimerKind::After
        },
        duration_ms,
        owner_state_id: owner_state.to_string(),
        target: Some(target.to_string()),
        actions: vec![],
        loc: loc(),
    }
}

pub fn completion_transition(id: &str, source: &str, target: &str) -> TransitionObject {
    #[allow(deprecated)]
    TransitionObject {
        id: id.to_string(),
        stable_id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        trigger: Some(Trigger::Completion {
            from: source.to_string(),
        }),
        guard: None,
        actions: vec![],
        priority: 100,
        kind: TransitionKind::Completion,
        internal: false,
        loc: loc(),
    }
}

pub fn event(id: &str, name: &str) -> EventObject {
    EventObject {
        id: id.to_string(),
        stable_id: format!("M:event:{name}"),
        name: name.to_string(),
        payload: vec![],
        loc: loc(),
    }
}

pub fn history(id: &str, kind: HistoryKind, default_target: &str) -> HistoryObject {
    HistoryObject {
        id: id.to_string(),
        stable_id: id.to_string(),
        history_kind: kind,
        default_target: default_target.to_string(),
        loc: loc(),
    }
}
