//! Defer-related checks — Doc 04 §9.4 / Doc 00 §G-08 / Audit P0-5.
//!
//! - `FSM-E0310` if a state declares both `defer E` and an explicit
//!   `on E -> ...` (or `internal on E:`) for the same event.
//! - `FSM-E0903` on every `defer EVENT` declaration. Per audit P0-5
//!   option-b: v1.0 codegen has no real defer queue (the prior runtime
//!   path silently dropped deferred events, contradicting Doc 02 G1
//!   "no undefined behaviour"). Until v1.1 ships a working queue, we
//!   reject `defer` at analysis time so users get a clear error pointing
//!   at the v1.1 roadmap rather than silently broken machines. The
//!   simulator's defer support (Doc 08 §10) remains intact for internal
//!   tooling; this gate is codegen-facing. See Doc 00 §6 (D-15 to be
//!   added) and `docs/AUDIT_2026_05_14.md` §P0-5.

use std::collections::HashSet;

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};

use crate::symbol_table::SymbolTable;
use crate::util::span_of;

/// Run defer-related checks.
pub fn check(file: &ast::File, _st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for machine in file.machines() {
        // E0310 — defer-vs-transition conflict per state.
        for state in walk_states(&machine) {
            check_state_defer_conflicts(&state, out);
        }
        // E0903 — defer is not yet supported in v1.0 (audit P0-5
        // option-b). Emit one diagnostic per `defer EVENT` declaration so
        // users see every offending site, not just the first machine. The
        // older "too many event types for defer bitmask" trigger is
        // subsumed: with zero supported defers, the >256-event capacity
        // check is moot.
        for state in walk_states(&machine) {
            for d in state.defers() {
                let event = d.event().unwrap_or_else(|| "<unknown>".to_string());
                out.push(
                    Diagnostic::new(DiagnosticCode::E0903, span_of(d.syntax())).with_message(
                        format!(
                            "`defer {event}` is not yet supported in v1.0; \
                             remove the `defer` clause or wait for v1.1 \
                             (see docs/AUDIT_2026_05_14.md §P0-5)",
                        ),
                    ),
                );
            }
        }
    }
}

fn check_state_defer_conflicts(state: &ast::StateDecl, out: &mut Vec<Diagnostic>) {
    let mut deferred: HashSet<String> = HashSet::new();
    for d in state.defers() {
        if let Some(name) = d.event() {
            deferred.insert(name);
        }
    }
    if deferred.is_empty() {
        return;
    }
    // Walk transitions directly on this state for E0310.
    for t in state.transitions() {
        if let Some(name) = t.trigger() {
            if deferred.contains(&name) {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0310, span_of(t.syntax())).with_message(
                        format!(
                        "event '{name}' is deferred AND has an explicit transition in this state",
                    ),
                    ),
                );
            }
        }
    }
    for t in state.internal_transitions() {
        if let Some(name) = t.trigger() {
            if deferred.contains(&name) {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0310, span_of(t.syntax())).with_message(
                        format!(
                        "event '{name}' is deferred AND has an internal transition in this state",
                    ),
                    ),
                );
            }
        }
    }
    for t in state.local_transitions() {
        if let Some(name) = t.trigger() {
            if deferred.contains(&name) {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0310, span_of(t.syntax())).with_message(
                        format!(
                            "event '{name}' is deferred AND has a local transition in this state",
                        ),
                    ),
                );
            }
        }
    }
}

/// Yield every `StateDecl` reachable from a machine (top-level + nested +
/// region states).
fn walk_states(m: &ast::MachineDecl) -> Vec<ast::StateDecl> {
    let mut out = Vec::new();
    for s in m.states() {
        out.push(s.clone());
        collect_nested(&s, &mut out);
    }
    for r in m.regions() {
        for s in r.states() {
            out.push(s.clone());
            collect_nested(&s, &mut out);
        }
    }
    out
}

fn collect_nested(s: &ast::StateDecl, out: &mut Vec<ast::StateDecl>) {
    for nested in s.nested_states() {
        out.push(nested.clone());
        collect_nested(&nested, out);
    }
    for r in s.regions() {
        for child in r.states() {
            out.push(child.clone());
            collect_nested(&child, out);
        }
    }
}
