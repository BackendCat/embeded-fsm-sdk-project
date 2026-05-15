//! `fsm-simulator` — pure-Rust interpreter implementing FSM Studio semantics.
//!
//! Per Doc 20 §8 + Doc 00 §7.2 this v1.0 crate ships the **interpreter only**.
//! The WebSocket JSON-RPC server described in Doc 13 is deferred to v1.1+.
//! No `tokio`, no `tower-lsp`, no `tungstenite` — the dependency list is
//! intentionally minimal so the simulator runs inside the conformance test
//! runner and the `fsm simulate` CLI without dragging in async machinery.
//!
//! ## Design
//!
//! - [`interpreter::Interpreter`] is the single public driver. It owns an
//!   `Arc<MachineIndex>` per machine and a mutable [`runtime::RuntimeState`].
//! - The RTC step (Doc 08 §3.1) lives in [`interpreter`]. Helpers for LCA
//!   (B-09), exit/entry order (Doc 08 §6 / §7), and completion (B-08) are in
//!   [`runtime`].
//! - Trace records ([`trace::StepRecord`]) match Doc 13 §11 shape verbatim
//!   so the conformance suite consumes them without translation.
//!
//! ## Equivalence gate
//!
//! For any IR `I` and event sequence `E`, the simulator output (the list of
//! `StepRecord`s) must agree with the C99 emit of the same IR executed
//! against the same input. The `tests/codegen_equivalence_smoke.rs` test is
//! a placeholder for when codegen-c gains the instrumented hooks needed to
//! observe its run-time behaviour (TODO post-v1.0).

#![forbid(unsafe_code)]

pub use fsm_ir::{Ir, MachineObject, StateNode};

pub mod eval;
pub mod interpreter;
pub mod runtime;
pub mod trace;

pub use interpreter::{InitOptions, Interpreter, StepError};
pub use runtime::{
    EventKind, EventQueue, InterpreterSnapshot, MachineIndex, QueuedEvent, RuntimeState, Timer,
    TimerFire, TimerSet, Value,
};
pub use trace::{
    execute_trace, parse_trace_yaml, write_trace_yaml, EventReceivedRecord, ExecError, InitTrace,
    StepKind, StepRecord, SubmachineRecord, TraceCommand, TraceFile, TraceParseError, TraceResult,
    TransitionTakenRecord,
};
