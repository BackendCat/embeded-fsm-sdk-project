//! The deadlock definition — **derived from Doc 08, cross-checked against
//! the spec, NOT from intuition** (Doc 30 §4.1 sub-foot-gun, R3).
//!
//! ## Doc 08 derivation (the normative chain)
//!
//! - Doc 08 §1: a **basic state** is `simple`, `final`, or a parallel
//!   region with no active substate. A **configuration** C is the set of
//!   active basic states.
//! - Doc 08 §1: a transition T is **enabled** iff its source ∈ C, its
//!   trigger matches the current event (or is a completion), and its guard
//!   is true.
//! - Doc 08 §3.1: `RTC_step` — `if select_transitions = ∅: return` (the
//!   event is *discarded*; the configuration is unchanged).
//! - Doc 08 §9.1 / §9.2 / §14: completion events are generated on entering
//!   a `final` (or all-regions-final parallel) state and processed at the
//!   **front** of the internal queue, *immediately*, within the same
//!   `M_dispatch` drain (`M_dispatch` loops while the internal queue is
//!   non-empty). Therefore any configuration the interpreter *returns* (it
//!   drains to quiescence on `init` / `dispatch`) has **no pending
//!   completion** by construction — completion-driven progress has already
//!   been taken.
//!
//! ## The definition (W2 — composite/parallel/history/timer/submachine)
//!
//! A reachable configuration C is **deadlocked** iff *all* hold:
//!
//! 1. C is **not a final configuration** — at least one active leaf is not
//!    a `final` state. A configuration whose every active leaf is `final`
//!    is a *legitimate terminal* (Doc 08 §9.1's all-regions-done /
//!    composite-final rules) — including an **all-regions-final parallel**
//!    (every region's leaf is `final`) — the machine has *completed*, and
//!    that is **NOT** a deadlock.
//! 2. **No exploration edge makes progress.** The W2 explorer drives two
//!    edge families through the interpreter (the §4.1 keystone): (a) for
//!    *every* declared event, `dispatch` leaves the full configuration
//!    **unchanged** (Doc 08 §3.1 — no enabled transition ⇒ discard ⇒
//!    config unchanged); **and** (b) the **timer-fire edge** — if any
//!    timer is armed anywhere (parent or, recursively, a nested
//!    sub-instance), `advance_clock` to the soonest expiry also leaves the
//!    configuration unchanged. We never decide "is this transition/timer
//!    enabled?" ourselves; we ask the interpreter by driving it. Hence a
//!    state armed-waiting on a timer that *fires* is **NOT** a deadlock
//!    (edge (b) progresses it — the Doc 08 §13 / Doc 30 §4.1 foot-gun
//!    guard, now W2-real, not W1-deferred); only a config where *even the
//!    timer-fire* changes nothing (e.g. an `every_internal` heartbeat that
//!    runs an action but never transitions) is stuck.
//! 3. **No completion is pending** — guaranteed by construction: the
//!    interpreter only ever hands us drained-to-quiescence configurations
//!    (§9.2/§14 above), and **submachine-completion** (a sub-instance
//!    reaching its `final` driving the parent's `done ->`) is taken
//!    *inside* the interpreter's `dispatch`/`advance_clock` RTC drain — so
//!    a config whose only progress is a submachine-completion is **NOT** a
//!    deadlock (the event/timer edge that triggered the drain already set
//!    progress, or `init` drained it). No extra check is needed here; this
//!    clause is the explicit invariant.
//!
//! ### Doc 08 derivation of the W2 extensions (cited, not intuited)
//!
//! - **Timer (Doc 08 §13).** A timer fires when its owning runtime's
//!   `virtual_clock_ms` reaches `expiry_ms`. So "no declared event ⇒
//!   deadlock" is **wrong** the moment timers exist (it false-positives
//!   every timer-wait). The correct rule is "no progress by **any** edge",
//!   and the timer-fire edge is driven by the real
//!   `Interpreter::advance_clock` — never a re-implemented clock/firing
//!   rule. (`digest` additionally normalises the *non-observable* absolute
//!   clock origin to a relative timer phase — Doc 08 §13 — so cyclic
//!   timers / heartbeats terminate and a genuine timer-deadlock is
//!   *detected* rather than wrongly reported `Inconclusive`; the
//!   bisimulation proof is in `digest`.)
//! - **Parallel join (Doc 08 §9.1).** An all-regions-final parallel is the
//!   completion *terminal*, caught by clause 1 (`is_final_configuration`
//!   = every active leaf is `final`); it must NOT be flagged a deadlock. A
//!   parallel where one region can never reach its `final` so the join
//!   never fires *and* no other transition leaves the composite IS a
//!   genuine deadlock — correctly caught by clause 2 (no edge progresses).
//! - **Submachine (Doc 08 §12).** Sub-instance advancement (delegation,
//!   §12.1) and sub-completion driving the parent `done ->` (§12.3) are
//!   the interpreter's, taken inside `dispatch`; the explorer only observes
//!   the resulting snapshot (whose nested sub-instance state is now
//!   captured losslessly — the W2-P0 fix). No `done` is re-implemented.

use fsm_ir::{MachineObject, StateNode};

/// A detected deadlock: the reachable configuration with no possible
/// progress, plus enough context to surface a counterexample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeadlockReport {
    /// Active leaf-state IDs of the deadlocked configuration (the
    /// interpreter's `current_states()` for that config).
    pub config: Vec<String>,
}

/// True iff `state_id` resolves to a `final` state in `machine`.
///
/// **Structural IR read only** — this inspects the IR's `StateNode::Final`
/// discriminator, which is data, not behaviour. It does *not* re-implement
/// any execution semantics; "is the machine complete?" is a structural
/// question about the active configuration, answered against the IR shape.
fn is_final_state(machine: &MachineObject, state_id: &str) -> bool {
    fn walk(states: &[StateNode], target: &str) -> bool {
        for s in states {
            match s {
                StateNode::Final(f) if f.id == target => return true,
                StateNode::Composite(c) => {
                    for r in &c.regions {
                        if walk(&r.states, target) {
                            return true;
                        }
                    }
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
                        if walk(&r.states, target) {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }
    walk(&machine.root.states, state_id)
}

/// Is `config` (the interpreter's active leaf set) a *final configuration*
/// — i.e. has the machine completed?
///
/// Per Doc 08 §1 + §9.1: a configuration is terminal when its active
/// leaves are `final` states. For a **flat machine** (W1 scope) there is
/// exactly one active leaf and it is final iff the machine has reached its
/// top-level `final`. We require the config to be non-empty and *every*
/// active leaf to be a `final` state — this is the conservative,
/// spec-faithful predicate that also generalises to W2's hierarchical /
/// parallel finals (all regions final) without change.
pub(crate) fn is_final_configuration(machine: &MachineObject, config: &[String]) -> bool {
    !config.is_empty() && config.iter().all(|s| is_final_state(machine, s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ir_of(src: &str) -> fsm_ir::Ir {
        let pr = fsm_parser::parse(src);
        let res = fsm_analyzer::analyze(&pr);
        assert!(
            !res.diagnostics
                .iter()
                .any(|d| d.severity == fsm_diagnostics::Severity::Error),
            "fixture must analyze clean; got {:?}",
            res.diagnostics
        );
        res.ir.expect("analyzer produced IR")
    }

    #[test]
    fn final_state_is_detected_structurally() {
        let ir = ir_of(
            r#"language fsm 2.0
machine M {
    events { GO }
    initial A
    state A { on GO -> Done }
    final Done
}"#,
        );
        let m = &ir.machines[0];
        // Find the IDs the lowerer assigned.
        let mut a_id = None;
        let mut done_id = None;
        for s in &m.root.states {
            match s {
                StateNode::Simple(s) if s.name == "A" => a_id = Some(s.id.clone()),
                StateNode::Final(f) if f.name == "Done" => done_id = Some(f.id.clone()),
                _ => {}
            }
        }
        let a_id = a_id.expect("state A lowered");
        let done_id = done_id.expect("final Done lowered");

        assert!(is_final_state(m, &done_id), "Done must be a final state");
        assert!(!is_final_state(m, &a_id), "A must NOT be a final state");
        assert!(
            is_final_configuration(m, &[done_id]),
            "config {{Done}} is a terminal configuration, NOT a deadlock"
        );
        assert!(
            !is_final_configuration(m, &[a_id]),
            "config {{A}} is not terminal"
        );
        assert!(
            !is_final_configuration(m, &[]),
            "empty config is not a final configuration"
        );
    }
}
