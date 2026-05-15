//! Transition declarations, per Doc 04 §8.
//!
//! All four forms start with `on EVENT`:
//!
//! - **External** `on E [g] -> T : a` (TRANSITION_DECL).
//! - **Internal** `on E [g] : a` (no `->`; INTERNAL_DECL).
//! - **Local** `on E [g] ~> T : a` (LOCAL_DECL).
//!
//! The form is determined after parsing the optional guard + priority and
//! peeking at `->` / `~>` / `:`. We use the `Parser::checkpoint` mechanism
//! so the wrapping syntax-kind node can be picked retroactively.
//!
//! v1.1-W4: an optional `likely` / `rare` BRANCH_HINT child may precede the
//! `on` keyword (parsed by the state-item dispatcher, which passes its
//! pre-hint checkpoint so the wrapping transition node encloses the hint).

use fsm_diagnostics::DiagnosticCode;
use fsm_lexer::TokenKind;

use crate::cst::SyntaxKind;
use crate::parser::Parser;

use super::guard::parse_guard_clause;
use super::state::parse_priority_clause;
use super::stmt::parse_action_block;

/// Entry point: called when the current token is `on`. Consumes one
/// transition. `outer_cp` lets the caller (the branch-hint dispatcher)
/// supply a checkpoint taken *before* a preceding `BRANCH_HINT` node so the
/// resulting transition node encloses the hint as its first child; when
/// `None` the checkpoint is taken here (the ordinary, hint-free path —
/// byte-identical CST to pre-W4).
pub(crate) fn parse_transition_from_on(p: &mut Parser) {
    parse_transition_from_on_at(p, None);
}

pub(crate) fn parse_transition_from_on_at(p: &mut Parser, outer_cp: Option<rowan::Checkpoint>) {
    let cp = outer_cp.unwrap_or_else(|| p.checkpoint());
    p.bump(); // on
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);

    if p.at(TokenKind::LBracket) {
        parse_guard_clause(p);
    }
    if p.at(TokenKind::KwPriority) {
        parse_priority_clause(p);
    }

    match p.current() {
        TokenKind::Arrow => {
            p.start_node_at(cp, SyntaxKind::TRANSITION_DECL);
            p.bump(); // ->
            p.expect(TokenKind::Ident, DiagnosticCode::E0010);
            if p.eat(TokenKind::Colon) {
                parse_action_block(p);
            }
            p.finish_node();
        }
        TokenKind::HistoryArrow => {
            p.start_node_at(cp, SyntaxKind::LOCAL_DECL);
            p.bump(); // ~>
            p.expect(TokenKind::Ident, DiagnosticCode::E0010);
            if p.eat(TokenKind::Colon) {
                parse_action_block(p);
            }
            p.finish_node();
        }
        TokenKind::Colon => {
            // Internal transition (no `->`).
            p.start_node_at(cp, SyntaxKind::INTERNAL_DECL);
            p.bump(); // :
            parse_action_block(p);
            p.finish_node();
        }
        _ => {
            p.start_node_at(cp, SyntaxKind::TRANSITION_DECL);
            p.error(
                DiagnosticCode::E0010,
                format!(
                    "expected '->', '~>' or ':' after transition trigger, found '{}'",
                    p.current()
                ),
            );
            p.finish_node();
        }
    }
}
