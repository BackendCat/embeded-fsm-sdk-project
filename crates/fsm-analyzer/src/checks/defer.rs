//! Defer-related checks — Doc 04 §9.4 / Doc 00 §G-08.
//!
//! - `FSM-E0310` if a state declares both `defer E` and an explicit
//!   `on E -> ...` (or `internal on E:`) for the same event.
//! - `FSM-E0903` if the machine uses any `defer` declaration AND has more
//!   than 256 distinct event types (the upper cap from Doc 00 §G-08; the
//!   codegen-c bitmask grows to `uint8_t[ceil(N/8)]` between 33 and 256
//!   events, beyond which there is no fast representation).

use std::collections::HashSet;

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
use fsm_parser::cst::SyntaxKind;

use crate::symbol_table::SymbolTable;
use crate::util::span_of;

const DEFER_EVENT_LIMIT: usize = 256;

/// Run defer-related checks.
pub fn check(file: &ast::File, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for (m_idx, machine) in file.machines().enumerate() {
        // E0310 — defer-vs-transition conflict per state.
        for state in walk_states(&machine) {
            check_state_defer_conflicts(&state, out);
        }
        // E0903 — defer + too many events.
        let event_count = st.machines.get(m_idx).map(|s| s.events.len()).unwrap_or(0);
        if event_count > DEFER_EVENT_LIMIT
            && machine
                .syntax()
                .descendants()
                .any(|d| d.kind() == SyntaxKind::DEFER_DECL || d.kind() == SyntaxKind::STMT_DEFER)
        {
            out.push(
                Diagnostic::new(DiagnosticCode::E0903, span_of(machine.syntax())).with_message(
                    format!(
                        "machine has {event_count} event types — defer bitmask supports up to {DEFER_EVENT_LIMIT}",
                    ),
                ),
            );
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
