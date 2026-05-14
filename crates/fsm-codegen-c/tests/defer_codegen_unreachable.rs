//! Audit P0-5 option-b — defensive assert: if a `defer EVENT` IR ever
//! reaches codegen (analyzer regression), codegen panics in debug builds
//! rather than emit silently-broken C.
//!
//! The analyzer is the primary gate (see
//! `fsm-analyzer/tests/defer_rejected_in_v1_0.rs`); this test is the
//! belt-and-braces guard for the codegen entry. It bypasses the analyzer
//! by constructing the IR programmatically with `defers: vec![DeferDecl
//! { ... }]`, then asserts `emit()` panics with a clear message.
//!
//! Only meaningful in debug builds — `cargo test` defaults to that, and
//! release-mode test runs are out of scope (the audit doc gates on
//! `cargo test --workspace`).

#![cfg(debug_assertions)]

mod common;

use fsm_codegen_c::{emit, CodegenConfig};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    ContextSchema, DeferDecl, EventObject, InitialPseudo, Ir, MachineObject, OverflowPolicy,
    QueueConfig, RegionObject, SimpleState, StateNode,
};

fn loc() -> SourceLocation {
    SourceLocation::new("defer-bypass.fsm", Span::new(0, 1), 1, 1)
}

#[allow(deprecated)]
fn ir_with_defer() -> Ir {
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: String::new(),
        source_files: vec!["defer-bypass.fsm".into()],
        machines: vec![MachineObject {
            id: "m-x".into(),
            stable_id: "X".into(),
            name: "X".into(),
            context: ContextSchema { fields: vec![] },
            events: vec![EventObject {
                id: "e-data".into(),
                stable_id: "X:event:DATA".into(),
                name: "DATA".into(),
                payload: vec![],
                loc: loc(),
            }],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-initial".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-initial".into(),
                        target: "s-only".into(),
                        loc: loc(),
                    }),
                    StateNode::Simple(SimpleState {
                        id: "s-only".into(),
                        stable_id: "X:state:Only".into(),
                        name: "Only".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![],
                        timers: vec![],
                        defers: vec![DeferDecl {
                            event_id: "e-data".into(),
                            loc: loc(),
                        }],
                        loc: loc(),
                    }),
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

#[test]
#[should_panic(expected = "defer")]
fn codegen_panics_when_defer_bearing_ir_bypasses_analyzer() {
    // The analyzer would normally reject this with FSM-E0903. We bypass
    // it by handing codegen an IR built by hand. Codegen MUST refuse to
    // emit silently-broken C; the debug_assert in `emit::emit` should
    // fire.
    let ir = ir_with_defer();
    let config = CodegenConfig::default();
    let _ = emit(&ir, &config); // expected to panic
}
