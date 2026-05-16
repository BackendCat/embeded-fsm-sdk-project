//! Timer-duration checks — Doc 00 §7.9 (B-13).
//!
//! - `FSM-E0410` emitted when an `after` / `every` / `every_internal` resolves
//!   to a duration of `0` ms (or a negative literal).
//! - `FSM-W0601` emitted for durations exceeding 24 hours (86_400_000 ms).
//!
//! Duration extraction handles three forms:
//! 1. integer literal `after 100 ms` — direct parse.
//! 2. negative literal `after -1 ms` — unary EXPR_UNARY containing IntLiteral.
//! 3. constant reference `after MY_CONST ms` — looked up against the file
//!    consts table; non-resolvable refs are silently skipped (the
//!    reference-resolution pass complains separately).

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
// **W0 / Doc 29 §3.4 (R-2 + R-3 leave-and-explain, generalized to this
// check).** Retains `fsm_parser::cst` for: (R-2) the document-pre-order
// `m.syntax().descendants()` dispatch over AFTER/EVERY/EVERY_INTERNAL_DECL
// — the unsorted diagnostic vector (neither `run_all` nor the CLI sorts)
// makes traversal order byte-load-bearing for the W0 §4.2 gate, and the
// typed AST has no whole-subtree-preorder iterator; (R-3) `resolve_const_expr`
// / `resolve_expr_value`'s const-fold over the deliberately-shallow
// expression CST (`EXPR_LITERAL`/`EXPR_UNARY`/`EXPR_PAREN`/`EXPR_NAME_REF`,
// resolving file consts the parser cannot see). A typed accessor layer
// would relocate — not eliminate — these walks and add a large parser
// surface in a 0-new-API-intended wave. Folding either worsens clarity /
// risks the P0-1 byte-identity regression class for zero behaviour gain —
// Doc 00 §11.44/§11.49, the DRIFT-2 `LineIndex` precedent. (The typed
// timer `duration()`/`action_block()` accessors W0 added are consumed by
// the *lowerer*; this *check* does its own const-fold so it stays.)
use fsm_parser::cst::{SyntaxKind, SyntaxNode};

use crate::symbol_table::SymbolTable;
use crate::util::{parse_int_literal_i64, span_of};

const TWENTY_FOUR_HOURS_MS: i64 = 86_400_000;

/// Run timer duration checks across the AST.
pub fn check(file: &ast::File, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    // Collect file-level const literal values for resolution.
    let consts = file_consts(file);

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
    timer_node: &SyntaxNode,
    consts: &[(String, i64)],
    _st: &SymbolTable,
    out: &mut Vec<Diagnostic>,
) {
    let const_expr = timer_node
        .children()
        .find(|c| c.kind() == SyntaxKind::CONST_EXPR);
    let Some(const_expr) = const_expr else { return };
    let Some(value) = resolve_const_expr(&const_expr, consts) else {
        return;
    };
    let span = span_of(&const_expr);
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

/// Resolve a const-expression node to an `i64` value if it is a literal or a
/// reference to a previously declared `const`. Returns `None` for forms the
/// analyzer cannot fold at compile time (e.g. unresolved name refs, complex
/// arithmetic).
fn resolve_const_expr(const_expr: &SyntaxNode, consts: &[(String, i64)]) -> Option<i64> {
    // The CONST_EXPR node wraps a single expression child.
    let expr = const_expr.children().next()?;
    resolve_expr_value(&expr, consts)
}

fn resolve_expr_value(expr: &SyntaxNode, consts: &[(String, i64)]) -> Option<i64> {
    match expr.kind() {
        SyntaxKind::EXPR_LITERAL => {
            // Find IntLiteral / FloatLiteral text and parse.
            let tok = expr
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| matches!(t.kind(), SyntaxKind::IntLiteral | SyntaxKind::FloatLiteral))?;
            parse_int_literal_i64(tok.text())
        }
        SyntaxKind::EXPR_UNARY => {
            // Unary minus / plus around a literal.
            let op_tok = expr
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| matches!(t.kind(), SyntaxKind::Minus | SyntaxKind::Plus))?;
            let inner = expr.children().next()?;
            let v = resolve_expr_value(&inner, consts)?;
            match op_tok.kind() {
                SyntaxKind::Minus => Some(-v),
                SyntaxKind::Plus => Some(v),
                _ => Some(v),
            }
        }
        SyntaxKind::EXPR_PAREN => {
            let inner = expr.children().next()?;
            resolve_expr_value(&inner, consts)
        }
        SyntaxKind::EXPR_NAME_REF => {
            let name = expr
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| t.kind() == SyntaxKind::Ident)?
                .text()
                .to_string();
            consts.iter().find(|(n, _)| n == &name).map(|(_, v)| *v)
        }
        _ => None,
    }
}

/// Walk file-level const declarations and fold their values into `(name,
/// value)` pairs. Skips any const whose value cannot be folded.
fn file_consts(file: &ast::File) -> Vec<(String, i64)> {
    let mut out = Vec::new();
    let raw: Vec<_> = file.consts().collect();
    for c in &raw {
        if let (Some(name), Some(ce)) = (c.name(), c.value()) {
            // c.value() returns ConstExpr already.
            if let Some(v) = resolve_const_expr(ce.syntax(), &out) {
                out.push((name, v));
            }
        }
    }
    out
}
