//! Honest `FSM-E0400` / `FSM-W0602` emission (Doc 30 §1.3 catalog-drift
//! close, §5.2 gate "FSM-E0400 / FSM-W0602 honestly closed").
//!
//! Doc 30 §1.3 records that `FSM-E0400` (unreachable state) and
//! `FSM-W0602` (no-incoming-transitions, not initial) are
//! catalog-*reserved* but had **no emission site anywhere** — a ROADMAP
//! drift. This module is the real emission site, and it is *honest* about
//! what each code is allowed to claim:
//!
//! - **`FSM-E0400` (Error — the strong claim).** A state the
//!   interpreter-driven explorer *proved* can never appear in any reachable
//!   configuration. Only emitted when the search was **exhaustive**
//!   (`stop_reason == Exhausted`). On a bound-truncated search we MUST NOT
//!   claim `E0400` — "this state is unreachable" would itself be a
//!   false-proven (the dual of the cardinal sin). Doc 10's catalog entry
//!   hedges exactly this: *"Error if the compiler can prove statically"* —
//!   the exhaustive explorer is that proof; a truncated one is not.
//! - **`FSM-W0602` (Warning — the structural subset).** A declared
//!   concrete state with literally **zero incoming transition edges** in
//!   the IR and which is not an initial target. This is a purely
//!   *structural* observation (a shape fact, like the rest of this crate's
//!   non-semantic IR reads) and is sound to emit regardless of whether the
//!   dynamic search was exhaustive — a state nothing points at and that no
//!   region starts in cannot be entered. It is the weaker, always-safe
//!   companion to `E0400`.
//!
//! A state can legitimately get both (no incoming edge ⇒ structurally
//! W0602; the exhaustive explorer also proves E0400). De-duplication
//! policy is the CLI's concern (it currently surfaces both, most-severe
//! first); this module just produces the faithful set.

use std::collections::BTreeSet;

use fsm_diagnostics::{Diagnostic, DiagnosticCode, Span};
use fsm_ir::{MachineObject, StateNode};

use crate::engine::StopReason;
use crate::reachability::ReachabilityReport;

/// Build the reachability diagnostics for one machine from the explorer's
/// reachability fact + the structural IR.
///
/// `exhaustive` gates `FSM-E0400`: pass `stats.stop_reason ==
/// Exhausted`. When the search was bound-truncated, **no `E0400` is
/// emitted** (only the structural `W0602`s) — the honest-unreachability
/// guard, the dual of the never-false-proven invariant.
///
/// Output is sorted by code then state id (deterministic — the project's
/// reproducibility discipline).
pub fn reachability_diagnostics(
    machine: &MachineObject,
    report: &ReachabilityReport,
    stop_reason: StopReason,
) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    let exhaustive = matches!(stop_reason, StopReason::Exhausted);

    // FSM-E0400 — only when the search was exhaustive (the strong,
    // proof-backed claim).
    if exhaustive {
        for sid in &report.unreachable {
            if let Some((name, span)) = concrete_state_loc(machine, sid) {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0400, span).with_message(format!(
                        "unreachable state '{name}' — no execution path from the \
                         initial configuration ever enters it (proven by exhaustive \
                         bounded exploration)"
                    )),
                );
            }
        }
    }

    // FSM-W0602 — structural: declared concrete state with no incoming
    // transition edge anywhere and not an initial target. Always sound.
    let targeted = transition_target_ids(machine);
    let initials = initial_target_ids(machine);
    for (sid, name, span) in concrete_states(machine) {
        if !targeted.contains(&sid) && !initials.contains(&sid) {
            out.push(
                Diagnostic::new(DiagnosticCode::W0602, span).with_message(format!(
                    "unreachable state '{name}' — no incoming transitions and not an \
                     initial state"
                )),
            );
        }
    }

    // Deterministic order: (code string, state span start).
    out.sort_by(|a, b| (a.code.to_string(), a.span.start).cmp(&(b.code.to_string(), b.span.start)));
    out
}

/// `(name, span)` for a concrete (simple/final) state id, or `None` if the
/// id is not a concrete state. Structural IR read.
fn concrete_state_loc(machine: &MachineObject, id: &str) -> Option<(String, Span)> {
    concrete_states(machine)
        .into_iter()
        .find(|(sid, _, _)| sid == id)
        .map(|(_, name, span)| (name, span))
}

/// All concrete leaf states as `(id, name, span)`. Structural IR read —
/// no semantics.
fn concrete_states(machine: &MachineObject) -> Vec<(String, String, Span)> {
    fn walk(states: &[StateNode], acc: &mut Vec<(String, String, Span)>) {
        for s in states {
            match s {
                StateNode::Simple(s) => {
                    acc.push((s.id.clone(), s.name.clone(), s.loc.span));
                }
                StateNode::Final(f) => {
                    let name = if f.name.is_empty() {
                        f.id.clone()
                    } else {
                        f.name.clone()
                    };
                    acc.push((f.id.clone(), name, f.loc.span));
                }
                StateNode::Composite(c) => {
                    for r in &c.regions {
                        walk(&r.states, acc);
                    }
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
                        walk(&r.states, acc);
                    }
                }
                _ => {}
            }
        }
    }
    let mut acc = Vec::new();
    walk(&machine.root.states, &mut acc);
    acc
}

/// Set of every state id that is the `target` of some transition anywhere.
/// Structural IR read.
fn transition_target_ids(machine: &MachineObject) -> BTreeSet<String> {
    fn walk(states: &[StateNode], acc: &mut BTreeSet<String>) {
        for s in states {
            let transitions = match s {
                StateNode::Simple(s) => &s.transitions,
                StateNode::Composite(c) => {
                    for r in &c.regions {
                        walk(&r.states, acc);
                    }
                    &c.transitions
                }
                StateNode::Parallel(p) => {
                    for r in &p.regions {
                        walk(&r.states, acc);
                    }
                    &p.transitions
                }
                _ => continue,
            };
            for t in transitions {
                acc.insert(t.target.clone());
            }
        }
    }
    let mut acc = BTreeSet::new();
    walk(&machine.root.states, &mut acc);
    acc
}

/// Set of every state id named as a region's `initial` target. Structural
/// IR read.
fn initial_target_ids(machine: &MachineObject) -> BTreeSet<String> {
    fn walk_region(r: &fsm_ir::RegionObject, acc: &mut BTreeSet<String>) {
        // The region's `initial` field is a pseudo-state id; the actual
        // entered state is that Initial node's `target`. Resolve it.
        for s in &r.states {
            if let StateNode::Initial(i) = s {
                if i.id == r.initial {
                    acc.insert(i.target.clone());
                }
            }
            match s {
                StateNode::Composite(c) => {
                    for cr in &c.regions {
                        walk_region(cr, acc);
                    }
                }
                StateNode::Parallel(p) => {
                    for pr in &p.regions {
                        walk_region(pr, acc);
                    }
                }
                _ => {}
            }
        }
    }
    let mut acc = BTreeSet::new();
    walk_region(&machine.root, &mut acc);
    acc
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

    fn reachable_of(ir: &fsm_ir::Ir) -> ReachabilityReport {
        crate::verify(ir, crate::VerifyOptions::default())
            .expect("verify")
            .reachability
    }

    #[test]
    fn island_emits_e0400_and_w0602_when_exhaustive() {
        let ir = ir_of(
            r#"language fsm 2.0
machine M {
    events { GO BACK }
    initial Idle
    state Idle { on GO -> Active }
    state Active { on BACK -> Idle }
    state Island { on GO -> Idle }
}"#,
        );
        let m = &ir.machines[0];
        let report = reachable_of(&ir);
        let diags = reachability_diagnostics(m, &report, StopReason::Exhausted);

        // Island has no incoming edge AND is dynamically unreachable ⇒ both
        // codes, for Island only.
        assert!(
            diags
                .iter()
                .any(|d| d.code == DiagnosticCode::E0400 && d.message.contains("Island")),
            "exhaustive search must emit FSM-E0400 for Island; got {diags:?}"
        );
        assert!(
            diags
                .iter()
                .any(|d| d.code == DiagnosticCode::W0602 && d.message.contains("Island")),
            "Island has no incoming transitions ⇒ FSM-W0602; got {diags:?}"
        );
        // No false positive on the reachable states.
        assert!(
            !diags
                .iter()
                .any(|d| d.message.contains("Idle") || d.message.contains("Active")),
            "reachable states must not be flagged; got {diags:?}"
        );
    }

    #[test]
    fn no_e0400_when_search_was_truncated() {
        // The honest-unreachability guard: a bound-truncated search must
        // NOT emit E0400 (claiming a state unreachable from an incomplete
        // search is a false-proven, the dual of the cardinal sin). The
        // structural W0602 may still fire (it is sound regardless).
        let ir = ir_of(
            r#"language fsm 2.0
machine M {
    events { GO BACK }
    initial Idle
    state Idle { on GO -> Active }
    state Active { on BACK -> Idle }
    state Island { on GO -> Idle }
}"#,
        );
        let m = &ir.machines[0];
        let report = reachable_of(&ir);

        let diags = reachability_diagnostics(m, &report, StopReason::MaxStatesHit);
        assert!(
            !diags.iter().any(|d| d.code == DiagnosticCode::E0400),
            "a truncated search must NOT emit FSM-E0400 (false-proven guard); got {diags:?}"
        );
        // W0602 (structural, no-incoming) is still sound to emit.
        assert!(
            diags
                .iter()
                .any(|d| d.code == DiagnosticCode::W0602 && d.message.contains("Island")),
            "structural W0602 holds regardless of search exhaustiveness; got {diags:?}"
        );
    }

    #[test]
    fn sound_machine_emits_no_reachability_diagnostics() {
        let ir = ir_of(
            r#"language fsm 2.0
machine M {
    events { GO BACK }
    initial Idle
    state Idle { on GO -> Active }
    state Active { on BACK -> Idle }
}"#,
        );
        let m = &ir.machines[0];
        let report = reachable_of(&ir);
        let diags = reachability_diagnostics(m, &report, StopReason::Exhausted);
        assert!(
            diags.is_empty(),
            "a fully-reachable machine must produce no E0400/W0602; got {diags:?}"
        );
    }
}
