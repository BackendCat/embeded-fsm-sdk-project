//! Defer-related checks — Doc 04 §9.4 / Doc 08 §10 / Doc 00 §G-08.
//!
//! - `FSM-E0310` if a state declares both `defer E` and an explicit
//!   `on E -> ...` (or `internal on E:`) for the same event. This is a
//!   genuine semantic conflict (Doc 10 §7): the user asked to both hold
//!   and consume the same event in the same state. It stays a hard error.
//!
//! v1.1 (2026-05-15): `FSM-E0903` ("`defer EVENT` not supported in v1.0",
//! audit P0-5 option-b) is **retired**. Real per-state defer-buffer
//! runtime now ships in both codegen-c and the simulator per Doc 08 §10,
//! so analysis no longer rejects `defer`. The defer set is lowered into
//! the IR (`StateNode::*::defers`) by `lower::lower_defers`; codegen and
//! the simulator consume it directly. The retired E0903 wire form still
//! parses in `// fsm-lint:disable` annotations via
//! [`fsm_diagnostics::deprecated::DeprecatedCode`] (Doc 10 §14). Supersedes
//! docs/00 §11.7.

use std::collections::HashSet;

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};

use crate::symbol_table::SymbolTable;
use crate::util::{span_of, walk_all_states};

/// Run defer-related checks.
pub fn check(file: &ast::File, _st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for machine in file.machines() {
        // E0310 — defer-vs-transition conflict per state. This is the only
        // defer diagnostic in v1.1: declaring `defer E` AND a transition on
        // `E` in the same state is contradictory (UML 2.5.1 §14.2.3.9.1:
        // an enabled transition wins over deferral, so the `defer` would be
        // dead — flag it rather than silently ignore one of the two).
        for state in walk_all_states(&machine) {
            check_state_defer_conflicts(&state, out);
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

// Walker logic moved to `crate::util::walk_all_states` (R2.1 dedup,
// 2026-05-15).
