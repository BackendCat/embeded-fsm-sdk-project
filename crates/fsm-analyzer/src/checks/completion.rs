//! Completion-transition semantics — Doc 00 §7.5 (B-07).
//!
//! Per B-07 the analyzer DOES NOT emit FSM-E0301; completion-transition
//! guards are explicitly allowed. The remaining duty of this module is:
//!
//! - emit `FSM-E0401` when a composite state declares an external
//!   self-transition with `->` (likely the user meant `~>` or `internal on`);
//! - emit `FSM-W0101` (dead transition) when a `done` transition exists with
//!   `[else]` followed by a later unguarded `done -> ...` — the latter is
//!   unreachable.
//!
//! The dead-transition check overlaps with [`super::determinism`]; this file
//! keeps the completion-specific cases together.

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
use fsm_parser::cst::SyntaxKind;

use crate::symbol_table::SymbolTable;
use crate::util::span_of;

/// Run completion-transition checks. Per B-07 we explicitly DO NOT emit
/// FSM-E0301 even if a completion carries a guard.
pub fn check(file: &ast::File, _st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for m in file.machines() {
        for state in walk_states(&m) {
            check_state(&state, out);
        }
    }
}

fn check_state(state: &ast::StateDecl, out: &mut Vec<Diagnostic>) {
    let name = state.name().unwrap_or_default();

    // External-self-transition warning on composite states.
    let has_nested = state.nested_states().next().is_some() || state.regions().next().is_some();
    if has_nested {
        for t in state.transitions() {
            if t.target().as_deref() == Some(name.as_str()) {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0401, span_of(t.syntax())).with_message(
                        format!(
                            "external self-transition on composite state '{name}' — consider `~>` (local) or `internal on`"
                        ),
                    ),
                );
            }
        }
    }

    // Completion-after-else dead detection.
    let mut saw_else = false;
    for c in state.completions() {
        let is_else = c
            .guard()
            .is_some_and(|g| matches!(g.expr(), Some(ast::Expr::GuardElse(_))));
        let is_unguarded = c.guard().is_none();
        if saw_else && (is_unguarded || !is_else) {
            out.push(
                Diagnostic::new(DiagnosticCode::W0101, span_of(c.syntax()))
                    .with_message("completion transition unreachable after `[else]`"),
            );
        }
        if is_else {
            saw_else = true;
        }
    }
}

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
    for child in s.nested_states() {
        out.push(child.clone());
        collect_nested(&child, out);
    }
    for r in s.regions() {
        for nested in r.states() {
            out.push(nested.clone());
            collect_nested(&nested, out);
        }
    }
    // Pseudo-state forms in state.children_with_tokens() are not StateDecls
    // — they live as separate SyntaxKind nodes (FINAL_DECL, etc.) and are
    // exempt from completion checks because they cannot host transitions
    // directly.
    let _ = SyntaxKind::FINAL_DECL;
}
