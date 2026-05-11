//! Guard clauses: `[ guard_expr ]`.
//!
//! Per Doc 04 §8.5 the guard sublanguage is restricted (no arithmetic, no
//! impure calls), but for parsing purposes the Pratt parser in
//! `expr.rs` accepts the union of operators and refuses arithmetic /
//! casts when the `ExprContext::Guard` flag is set. The analyzer enforces
//! the non-pure-call restriction (E0106) downstream.

use fsm_diagnostics::DiagnosticCode;
use fsm_lexer::TokenKind;

use crate::cst::SyntaxKind;
use crate::expr::parse_expr;
use crate::parser::{ExprContext, Parser};

/// `guard_clause = "[" , guard_expr , "]" ;`
pub fn parse_guard_clause(p: &mut Parser) {
    p.start_node(SyntaxKind::GUARD_CLAUSE);
    p.bump(); // [
    if p.at(TokenKind::RBracket) {
        // Empty guard `[]` — error but recoverable.
        p.error(DiagnosticCode::E0010, "empty guard clause");
    } else {
        parse_expr(p, 0, ExprContext::Guard);
    }
    p.expect(TokenKind::RBracket, DiagnosticCode::E0010);
    p.finish_node();
}
