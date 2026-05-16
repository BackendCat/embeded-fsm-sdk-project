//! Parallel-state region structural checks — Doc 04 §6 / Doc 10 §FSM-E0600.
//!
//! Two invariants:
//! 1. Every `region` block inside a parallel (multi-region) state MUST
//!    declare an `initial` — `FSM-E0600`.
//! 2. A multi-region state must contain ≥ 2 regions — single-region forms
//!    emit `FSM-H0004` (hint) per Doc 10.
//! 3. Single-state regions are flagged with `FSM-W0600`.
//!
//! "Parallel" here is detected structurally: a state node carries one or
//! more `region` children. Composite states with no `region` keyword are
//! plain composites and exempt from these checks.

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};

use crate::symbol_table::SymbolTable;
use crate::util::span_of;

/// Run parallel-state checks.
pub fn check(file: &ast::File, _st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for m in file.machines() {
        // Top-level parallel states attached directly to the machine.
        for region in m.regions() {
            check_region(&region, out);
        }
        // Nested parallels — walk every state and inspect its regions.
        for state in m.states() {
            walk_state(&state, out);
        }
    }
}

fn walk_state(state: &ast::StateDecl, out: &mut Vec<Diagnostic>) {
    let regions: Vec<_> = state.regions().collect();
    if regions.len() == 1 {
        // Single-region "parallel" — informational hint per Doc 10 H0004.
        out.push(
            Diagnostic::new(DiagnosticCode::H0004, span_of(state.syntax())).with_message(
                "parallel state has only one region — consider a plain composite state",
            ),
        );
    }
    for r in &regions {
        check_region(r, out);
    }
    for nested in state.nested_states() {
        walk_state(&nested, out);
    }
}

fn check_region(region: &ast::RegionDecl, out: &mut Vec<Diagnostic>) {
    // The region MUST declare an initial. W0 (Doc 29 §3.3): typed
    // `initials()` iterator replaces the old `children()+kind()` CST
    // predicate — a behaviour-identical existence check (the diagnostic is
    // keyed by the region span; the result is order-independent).
    let has_initial = region.initials().next().is_some();
    if !has_initial {
        out.push(
            Diagnostic::new(DiagnosticCode::E0600, span_of(region.syntax())).with_message(format!(
                "parallel region '{}' has no `initial` declaration",
                region.name().unwrap_or_default()
            )),
        );
    }
    // Region-with-single-state warning W0600.
    let states: Vec<_> = region.states().collect();
    if states.len() == 1 {
        out.push(
            Diagnostic::new(DiagnosticCode::W0600, span_of(region.syntax())).with_message(format!(
                "region '{}' has only one state",
                region.name().unwrap_or_default()
            )),
        );
    }
    // Recurse — nested parallels live inside region states.
    for s in &states {
        walk_state(s, out);
    }
}
