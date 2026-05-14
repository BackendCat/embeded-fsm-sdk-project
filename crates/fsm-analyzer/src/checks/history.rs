//! History pseudo-state checks — Doc 00 §7.10 (B-14).
//!
//! Every `shallow_history` / `deep_history` declaration MUST carry a
//! `default -> Target` clause. Missing default emits `FSM-E0111` so codegen
//! can rely on the IR's `default_target: String` (not `Option<String>`).
//!
//! `FSM-E0109` (target points to unknown state) is emitted from
//! [`super::name_resolution`].

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
use fsm_parser::cst::SyntaxKind;

use crate::symbol_table::SymbolTable;
use crate::util::span_of;

/// Run history-default checks. Only emits FSM-E0111; FSM-E0109 lives in
/// [`super::name_resolution`].
pub fn check(file: &ast::File, _st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for m in file.machines() {
        for descendant in m.syntax().descendants() {
            match descendant.kind() {
                SyntaxKind::SHALLOW_HISTORY_DECL => {
                    if let Some(h) = ast::ShallowHistoryDecl::cast(descendant.clone()) {
                        if h.default().is_none() {
                            out.push(
                                Diagnostic::new(DiagnosticCode::E0111, span_of(&descendant))
                                    .with_message(
                                        "shallow_history pseudo-state requires a `default ->` declaration",
                                    ),
                            );
                        }
                    }
                }
                SyntaxKind::DEEP_HISTORY_DECL => {
                    if let Some(h) = ast::DeepHistoryDecl::cast(descendant.clone()) {
                        if h.default().is_none() {
                            out.push(
                                Diagnostic::new(DiagnosticCode::E0111, span_of(&descendant))
                                    .with_message(
                                    "deep_history pseudo-state requires a `default ->` declaration",
                                ),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
