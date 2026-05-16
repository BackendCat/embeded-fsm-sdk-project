//! `fsm-verify` — the FSM Studio verification core.
//!
//! The v1.4 "verification core" epic (W1 keystone spine + W2
//! composite/parallel/history/timer/submachine coverage — Doc 30
//! §4.2-W1/-W2). This crate is the **single source of verification
//! truth**: a bounded explicit-state explorer that *drives the shipped
//! [`fsm_simulator::Interpreter`] as the transition oracle* and reports
//! reachability + deadlock properties over a model.
//!
//! ## The keystone (Doc 30 §4.1 — non-negotiable, an epic-level invariant)
//!
//! The explorer **NEVER re-implements transition selection, LCA, completion,
//! guard evaluation, or parallel semantics.** Those live exclusively in
//! [`fsm_simulator::interpreter`] (the single spec-conformant RTC engine,
//! Doc 08). The explorer:
//!
//! 1. builds one [`Interpreter`](fsm_simulator::Interpreter) over the IR,
//! 2. `init`s it to the initial configuration,
//! 3. for every frontier configuration: `restore`s the interpreter to that
//!    configuration's snapshot, then for **each declared event** calls
//!    `dispatch` and reads the resulting `snapshot` as the successor,
//! 4. backtracks purely by `restore`.
//!
//! Re-implementing any of that here would be a *second semantics* — the
//! exact P0-1 / MV5-1 "fabricate a parallel implementation" anti-pattern
//! this project keeps re-learning. The only IR data this crate reads
//! directly is **structural and semantics-free**: the declared event list
//! and which state nodes are `final` (so a legitimate terminal config is
//! not mistaken for a deadlock). "Which transition fires" is *always* the
//! interpreter's answer.
//!
//! ## Memory bound (Doc 30 §3.2 / §4.1 sub-foot-gun)
//!
//! The visited set is keyed by a **stable digest** of
//! [`InterpreterSnapshot`](fsm_simulator::InterpreterSnapshot), never the
//! retained full snapshot. The snapshot is all-`Vec`/`BTreeMap`/scalar, so
//! its canonical `serde_json` form is byte-stable across runs (the same
//! Doc 13 §11 wire-format contract `fsm-simulator`'s trace layer relies
//! on). Full snapshots are materialised only for the live frontier and the
//! witness path.
//!
//! ## Honest bound (Doc 30 §3.2 / R1 — the cardinal verification sin)
//!
//! Exploration is bounded by construction (`max_states` visited
//! configurations, `max_steps` explored edges). On hitting a bound the
//! verdict is [`Verdict::Inconclusive`] — **never** a false
//! [`Verdict::ProvenNoDeadlock`]. A false "proven" for a verifier is the
//! symbol-presence-equivalent: the one thing a safety-claiming tool must
//! never do.

#![forbid(unsafe_code)]

mod deadlock;
mod diagnostics;
mod digest;
mod engine;
mod reachability;

pub use deadlock::DeadlockReport;
pub use diagnostics::reachability_diagnostics;
pub use engine::{
    verify, ExplorationStats, StopReason, Verdict, VerifyError, VerifyOptions, VerifyOutcome,
    DEFAULT_MAX_STATES, DEFAULT_MAX_STEPS,
};
pub use reachability::ReachabilityReport;
