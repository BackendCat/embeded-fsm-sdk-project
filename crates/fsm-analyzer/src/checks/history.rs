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
// **W0 / Doc 29 §3.4 (R-2 leave-and-explain, generalized to this check).**
// `check` walks `m.syntax().descendants()` in rowan **document pre-order**
// and dispatches SHALLOW/DEEP_HISTORY_DECL. History pseudo-states can nest
// arbitrarily deep under states/regions; the typed AST has per-kind
// iterators but no whole-subtree-preorder iterator, and the emitted
// diagnostic vector is **unsorted** (neither `run_all` nor the CLI's
// `emit_json_aggregate` sorts) — so when ≥2 history decls miss a `default`
// the E0111 *order* is byte-load-bearing for the W0 §4.2 gate. Reproducing
// `descendants()` order via typed accessors would need the typed ordered
// heterogeneous-child iterator Doc 29 §3.4-R-2 rejects as the
// refactor-to-number trap; folding it would risk the P0-1 byte-identity
// regression class for zero behaviour gain — Doc 00 §11.44/§11.49, the
// DRIFT-2 `LineIndex` precedent. Left-and-explained.
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
