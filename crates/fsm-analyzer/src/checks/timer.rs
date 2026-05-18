//! Timer-duration checks — Doc 00 §7.9 (B-13).
//!
//! - `FSM-E0410` emitted when an `after` / `every` / `every_internal` resolves
//!   to a duration of `0` ms (or a negative literal).
//! - `FSM-W0601` emitted for durations exceeding 24 hours (86_400_000 ms).
//! - `FSM-E0411` emitted when an `after` / `every` / `every_internal`
//!   duration is **not** a compile-time constant — a runtime/context-variable
//!   expression (`after ctx.deadline_ms ms`) or any non-const-foldable form.
//!   Runtime-variable timer durations are explicitly post-v1.0 (Doc 02 §6 /
//!   Doc 08 §13.5 pt 5 / Doc 15.1 Note). Before this check the lowerer
//!   **silently dropped** such a timer (Finding F-1's sibling defect, audit
//!   §1.1 / §2.3) — a clean `fsm check` over a model that lost an edge. This
//!   is the hard rejecting diagnostic, mirroring the `defer`→`FSM-E0903`
//!   deferred-construct precedent (Doc 02 §5.3 / §9.4): a deferred construct
//!   is *rejected loudly*, never miscompiled silently.
//!
//! Duration extraction handles exactly the const-foldable forms via the
//! single shared [`crate::util::eval_const_expr_value`] resolver (the F-1
//! single-source-of-truth — the lowerer's `duration_ms` fold calls the SAME
//! function, so the check and the lowerer can never diverge again):
//! 1. integer literal `after 100 ms`.
//! 2. unary `after -1 ms`.
//! 3. parenthesised `after (100) ms`.
//! 4. `const` reference `after MY_CONST ms` (Doc 02 §6 / Doc 04 §12 — the
//!    *mandated* idiom; resolved against the file consts table).
//! Anything else (`ctx.`/`payload.` ref, unresolved name, non-const
//! arithmetic) folds to `None` ⇒ `FSM-E0411` (no longer a silent skip).

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
// **W0 / Doc 29 §3.4 (R-2 leave-and-explain).** Retains `fsm_parser::cst`
// for the document-pre-order `m.syntax().descendants()` dispatch over
// AFTER/EVERY/EVERY_INTERNAL_DECL — the unsorted diagnostic vector (neither
// `run_all` nor the CLI sorts) makes traversal order byte-load-bearing for
// the W0 §4.2 gate, and the typed AST has no whole-subtree-preorder
// iterator. A typed accessor layer would relocate — not eliminate — this
// walk and add a large parser surface in a 0-new-API-intended wave; folding
// it risks the P0-1 byte-identity regression class for zero behaviour gain
// (Doc 00 §11.44/§11.49, the DRIFT-2 `LineIndex` precedent).
//
// **F-1 (audit §1.1 / §6 items 1-2):** the prior private R-3 const-fold
// (`resolve_const_expr`/`resolve_expr_value`/`file_consts`) is DELETED. It
// was THE divergent second resolver: it resolved `EXPR_NAME_REF` against
// file consts while the lowerer's `eval_i64` did not, so a `const`-ref
// timer passed this bounds check but was silently dropped by the lowerer.
// Both sites now call the ONE shared `util::eval_const_expr_value` /
// `util::file_const_table` — unified by construction, the only fix that
// cannot re-create the asymmetry latently.
use fsm_parser::cst::SyntaxKind;

use crate::symbol_table::SymbolTable;
use crate::util::{eval_const_expr_value, file_const_table, span_of};

const TWENTY_FOUR_HOURS_MS: i64 = 86_400_000;

/// Run timer duration checks across the AST.
pub fn check(file: &ast::File, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    // The ONE shared file-consts table (F-1 single source of truth — the
    // lowerer builds the same table for its duration fold).
    let consts = file_const_table(file);

    for m in file.machines() {
        for d in m.syntax().descendants() {
            match d.kind() {
                SyntaxKind::AFTER_DECL
                | SyntaxKind::EVERY_DECL
                | SyntaxKind::EVERY_INTERNAL_DECL => {
                    check_timer(&d, &consts, st, out);
                }
                _ => {}
            }
        }
    }
}

fn check_timer(
    timer_node: &fsm_parser::cst::SyntaxNode,
    consts: &[(String, i64)],
    _st: &SymbolTable,
    out: &mut Vec<Diagnostic>,
) {
    let const_expr = timer_node
        .children()
        .find(|c| c.kind() == SyntaxKind::CONST_EXPR);
    let Some(const_expr) = const_expr else { return };
    let span = span_of(&const_expr);

    // `CONST_EXPR` wraps exactly one expression child. Fold it via the SAME
    // resolver the lowerer's `duration_ms` uses (F-1 unification).
    let folded = const_expr
        .children()
        .next()
        .and_then(|expr| eval_const_expr_value(&expr, consts));

    let Some(value) = folded else {
        // §2.3 rejecting diagnostic. The duration is NOT a compile-time
        // constant — a `ctx.`/`payload.` ref, an unresolved name, or
        // non-const arithmetic. Pre-F-1 the lowerer SILENTLY dropped this
        // whole timer (clean `fsm check`, model lost the edge — audit
        // §1.1's runtime-variable case). It is now a hard error: a
        // deferred construct is rejected loudly, never miscompiled
        // silently (the `defer`→`FSM-E0903` deferred-construct precedent,
        // Doc 02 §5.3 / §9.4).
        out.push(
            Diagnostic::new(DiagnosticCode::E0411, span).with_message(
                "timer duration must be a compile-time constant; \
                 runtime-variable timer durations are post-v1.0 (deferred)"
                    .to_string(),
            ),
        );
        return;
    };

    if value <= 0 {
        out.push(
            Diagnostic::new(DiagnosticCode::E0410, span).with_message(format!(
                "timer duration must be greater than zero (got {value})"
            )),
        );
    } else if value > TWENTY_FOUR_HOURS_MS {
        out.push(
            Diagnostic::new(DiagnosticCode::W0601, span).with_message(format!(
                "timer duration {value} ms exceeds 24 hours — verify units"
            )),
        );
    }
}
