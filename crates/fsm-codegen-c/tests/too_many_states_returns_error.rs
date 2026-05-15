//! Audit P1-8 (2026-05-14) — codegen-c must NOT panic on >255 states.
//!
//! ## Test classification (v1.1-W0 / SUBAGENT_CONVENTIONS §5.4, PD-2)
//!
//! The `.contains()` here are on the **`EmitError` Display string**
//! (`msg.contains("255")`, `msg.contains("state")`,
//! `!msg.contains("panicked")`), NOT on generated C. They are the correct
//! tool: this asserts a *user-facing error-message API contract* (the
//! exact wording `fsm generate` surfaces to the terminal, replacing the
//! pre-fix panic backtrace). The actual *behaviour* — `emit()` returns
//! `Err(TooManyStates)` / `Err(UnknownStateId)` instead of aborting, and
//! succeeds under the 255-state cap — is asserted via real
//! `match emit(...)` on the typed error, which IS behavioural. No
//! symbol-presence-of-C-text proxy exists in this file; not converted.
//!
//! Pre-fix, `state_index::IndexBuilder::push` called
//! `u8::try_from(...).expect("…")` and aborted the process when a machine
//! exceeded the `M_StateId_t` u8 cap. `fsm generate` would print a stderr
//! backtrace and exit via SIGABRT.
//!
//! Post-fix: `build_state_index` returns
//! `Result<StateIndex, EmitError::TooManyStates>` and `emit` propagates
//! the error through to the CLI, which prints a one-line diagnostic and
//! exits with code 2.
//!
//! Same audit also covered `state_index::must_lookup` (the panic site at
//! L113) — that case is exercised indirectly via the `pre_flight_validate`
//! path now folded into `emit`; here we focus on the overflow case
//! because constructing a 256+ state IR is straightforward.

use fsm_codegen_c::{emit, CodegenConfig};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    EventObject, InitialPseudo, Ir, MachineObject, OverflowPolicy, QueueConfig, RegionObject,
    SimpleState, StateNode, TransitionKind, TransitionObject, Trigger,
};

fn loc() -> SourceLocation {
    SourceLocation::new("toomany.fsm", Span::new(0, 1), 1, 1)
}

/// Build an IR with `n` simple states + the mandatory Initial pseudo-state.
/// The synthetic root sentinel occupies index 0, so n = 256 already
/// requires more than 256 records (root + initial + 256 simple = 258),
/// guaranteed to overflow u8.
fn ir_with_n_simple_states(n: usize) -> Ir {
    let mut states: Vec<StateNode> = Vec::with_capacity(n + 1);
    states.push(StateNode::Initial(InitialPseudo {
        id: "ps-init".into(),
        target: "s-0".into(),
        loc: loc(),
    }));
    for i in 0..n {
        states.push(StateNode::Simple(SimpleState {
            id: format!("s-{}", i),
            stable_id: format!("M:state:S{}", i),
            name: format!("S{}", i),
            entry: vec![],
            exit: vec![],
            transitions: vec![],
            timers: vec![],
            defers: vec![],
            loc: loc(),
        }));
    }
    let root = RegionObject {
        id: "r-root".into(),
        stable_id: None,
        name: "__root".into(),
        initial: "ps-init".into(),
        states,
        priority: 0,
        loc: loc(),
    };
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "".into(),
        source_files: vec!["toomany.fsm".into()],
        machines: vec![MachineObject {
            id: "m-toomany".into(),
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
            queue: QueueConfig {
                capacity: 4,
                overflow_policy: OverflowPolicy::Assert,
                loc: loc(),
            },
            targets: vec![],
            loc: loc(),
        }],
        diagnostics: vec![],
    }
}

#[test]
fn emit_returns_too_many_states_for_256_plus_state_machine() {
    // 256 simple states + 1 initial pseudo + 1 root sentinel = 258 records,
    // well over u8::MAX = 255.
    let ir = ir_with_n_simple_states(256);
    let result = emit(&ir, &CodegenConfig::default());
    match result {
        Err(fsm_codegen_c::EmitError::TooManyStates) => { /* expected */ }
        Err(other) => panic!(
            "expected EmitError::TooManyStates, got: {:?} ({})",
            other, other
        ),
        Ok(_) => panic!("expected EmitError::TooManyStates, got Ok"),
    }
}

#[test]
fn emit_error_message_is_human_readable() {
    let ir = ir_with_n_simple_states(300);
    let err = match emit(&ir, &CodegenConfig::default()) {
        Err(e) => e,
        Ok(_) => panic!("expected error"),
    };
    let msg = err.to_string();
    // The exact wording is part of the user-facing contract — `fsm generate`
    // surfaces this to the terminal. Audit P1-8: must not be the previous
    // panic backtrace verbiage.
    assert!(
        msg.contains("255"),
        "error must explain the 255-state limit; got: {}",
        msg
    );
    assert!(
        msg.contains("state"),
        "error must mention `state(s)`; got: {}",
        msg
    );
    assert!(
        !msg.contains("panicked"),
        "error must NOT mention `panicked` (pre-fix wording);\n{}",
        msg
    );
}

#[test]
fn emit_under_255_states_still_succeeds() {
    // Sanity: just under the limit must still emit OK. 250 simple states
    // + root + initial = 252 records. (u8::MAX is 255, so 252 fits.)
    let ir = ir_with_n_simple_states(250);
    let result = emit(&ir, &CodegenConfig::default());
    assert!(
        result.is_ok(),
        "250-state machine must emit successfully; got: {:?}",
        result.err()
    );
}

#[test]
fn emit_does_not_panic_on_unknown_state_id_reference() {
    // Build a minimal IR where a transition references a state id that
    // doesn't exist in the index. Pre-fix this hit
    // `state_index::must_lookup` → `panic!`. Post-fix the pre-flight
    // validate surfaces it as `EmitError::UnknownStateId`.
    #[allow(deprecated)]
    let bad_t = TransitionObject {
        id: "t-bad".into(),
        stable_id: "M:transition:t-bad".into(),
        source: "s-real".into(),
        target: "s-DOES-NOT-EXIST".into(), // <-- unindexed
        trigger: Some(Trigger::Event {
            event_id: "e-go".into(),
            payload_binding: None,
        }),
        guard: None,
        actions: vec![],
        priority: 0,
        kind: TransitionKind::External,
        internal: false,
        loc: loc(),
    };
    let real = SimpleState {
        id: "s-real".into(),
        stable_id: "M:state:Real".into(),
        name: "Real".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![bad_t],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    };
    let root = RegionObject {
        id: "r-root".into(),
        stable_id: None,
        name: "__root".into(),
        initial: "ps-init".into(),
        states: vec![
            StateNode::Initial(InitialPseudo {
                id: "ps-init".into(),
                target: "s-real".into(),
                loc: loc(),
            }),
            StateNode::Simple(real),
        ],
        priority: 0,
        loc: loc(),
    };
    let ir = Ir {
        ir_version: "1.0.0".into(),
        source_hash: "".into(),
        source_files: vec!["bad.fsm".into()],
        machines: vec![MachineObject {
            id: "m-bad".into(),
            stable_id: "M".into(),
            name: "M".into(),
            context: Default::default(),
            events: vec![EventObject {
                id: "e-go".into(),
                stable_id: "M:event:GO".into(),
                name: "GO".into(),
                payload: vec![],
                loc: loc(),
            }],
            externs: vec![],
            root,
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            queue: QueueConfig {
                capacity: 4,
                overflow_policy: OverflowPolicy::Assert,
                loc: loc(),
            },
            targets: vec![],
            loc: loc(),
        }],
        diagnostics: vec![],
    };
    match emit(&ir, &CodegenConfig::default()) {
        Err(fsm_codegen_c::EmitError::UnknownStateId(id)) => {
            assert!(
                id.contains("s-DOES-NOT-EXIST"),
                "unknown id must surface in the error: {}",
                id
            );
        }
        Err(other) => panic!(
            "expected EmitError::UnknownStateId, got: {:?} ({})",
            other, other
        ),
        Ok(_) => panic!("expected EmitError::UnknownStateId, got Ok"),
    }
}
