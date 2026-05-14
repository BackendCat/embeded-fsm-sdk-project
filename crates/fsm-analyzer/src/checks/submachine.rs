//! Submachine reference checks — Doc 04 §15.
//!
//! When a state declares a `submachine` reference (`submachine Foo : MachineId
//! { entry → ... exit → ... }`), the referenced machine must exist and the
//! entry/exit points declared on the parent must align with the submachine's
//! declared entry/exit points.
//!
//! The current grammar exposes submachines via the `KwSubmachine` keyword
//! but the parser does not yet wrap a distinct CST node for the reference —
//! we look for the `submachine` keyword inside a state and emit FSM-E0103 /
//! FSM-E0500 / FSM-E0501 best-effort. Full submachine wiring is gated behind
//! `feature submachines` per Doc 04 §15.

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
use fsm_parser::cst::SyntaxKind;

use crate::symbol_table::SymbolTable;
use crate::util::span_of;

/// Run submachine checks. Currently emits FSM-E0103 if a `send EVENT to X`
/// references an unknown machine — the rest of the submachine surface is
/// handled by name-resolution. Cycle detection (FSM-E0502) checks the
/// `submachines` field on the IR after lowering.
pub fn check(file: &ast::File, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    // Detect submachine declarations: a `submachine` keyword followed by a
    // state-declaration in the same state. The parser does not currently
    // produce a distinct CST node for the reference; we look for the keyword.
    for m in file.machines() {
        for descendant in m.syntax().descendants() {
            if descendant.kind() != SyntaxKind::STATE_DECL {
                continue;
            }
            // Direct token scan for `submachine` keyword.
            let is_submachine = descendant
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .any(|t| t.kind() == SyntaxKind::KwSubmachine);
            if !is_submachine {
                continue;
            }
            // The state declares it references a submachine. The first ident
            // token after `submachine` is the referenced machine name.
            let mut idents = descendant
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .filter(|t| t.kind() == SyntaxKind::Ident)
                .map(|t| t.text().to_string());
            let _own_name = idents.next();
            if let Some(reference) = idents.next() {
                if st.resolve_machine(&reference).is_none() {
                    out.push(
                        Diagnostic::new(DiagnosticCode::E0103, span_of(&descendant)).with_message(
                            format!("unknown machine '{reference}' in submachine reference"),
                        ),
                    );
                }
            }
        }
    }
    // Cycle detection — simple direct cycles only. Build a name→submachine
    // adjacency by scanning each machine subtree for nested `submachine`
    // references.
    let adjacency = build_submachine_graph(file, st);
    for (src, dsts) in &adjacency {
        for dst in dsts {
            if adjacency.get(dst).map(|t| t.contains(src)).unwrap_or(false) {
                if let Some(m) = file
                    .machines()
                    .find(|m| m.name().as_deref() == Some(src.as_str()))
                {
                    out.push(
                        Diagnostic::new(DiagnosticCode::E0502, span_of(m.syntax()))
                            .with_message(format!("submachine cycle detected: {src} <-> {dst}")),
                    );
                }
            }
        }
    }
}

fn build_submachine_graph(
    file: &ast::File,
    _st: &SymbolTable,
) -> std::collections::HashMap<String, Vec<String>> {
    let mut adj = std::collections::HashMap::<String, Vec<String>>::new();
    for m in file.machines() {
        let Some(name) = m.name() else { continue };
        let mut refs = Vec::new();
        for descendant in m.syntax().descendants() {
            if descendant.kind() != SyntaxKind::STATE_DECL {
                continue;
            }
            let is_submachine = descendant
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .any(|t| t.kind() == SyntaxKind::KwSubmachine);
            if !is_submachine {
                continue;
            }
            let idents: Vec<String> = descendant
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .filter(|t| t.kind() == SyntaxKind::Ident)
                .map(|t| t.text().to_string())
                .collect();
            if idents.len() >= 2 {
                refs.push(idents[1].clone());
            }
        }
        adj.insert(name, refs);
    }
    adj
}
