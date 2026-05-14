//! Action sublanguage statements — `assign`, `if`, `while`, `for`, `raise`,
//! `send`, `defer`, bare-call.
//!
//! Per Doc 04 §8.7 / §9. Action blocks are introduced by `:` in transition
//! / entry / exit / completion / timer contexts; the syntax is
//!
//! ```ebnf
//! action_list = statement , { ";" , statement } , [ ";" ] ;
//! ```
//!
//! A trailing `;` is optional. Action lists do not need their own braces —
//! they are flat semicolon-separated sequences terminated by the next
//! state_item start or `}`.

use fsm_diagnostics::DiagnosticCode;
use fsm_lexer::TokenKind;

use crate::cst::SyntaxKind;
use crate::expr::parse_expr;
use crate::parser::{ExprContext, Parser};
use crate::token_set::TokenSet;

/// Tokens that may end an action block from outside (without explicit `;`).
/// Used by the action loop to know when to stop scanning. These overlap
/// with `state_item` starts because `entry: a; on EV ...` lets the next
/// state item terminate the action block.
const ACTION_BLOCK_TERMINATORS: TokenSet = TokenSet::new(&[
    TokenKind::RBrace,
    TokenKind::KwOn,
    TokenKind::KwDone,
    TokenKind::KwAfter,
    TokenKind::KwEvery,
    TokenKind::KwDefer,
    TokenKind::KwState,
    TokenKind::KwExport,
    TokenKind::KwRegion,
    TokenKind::KwChoice,
    TokenKind::KwJunction,
    TokenKind::KwFork,
    TokenKind::KwJoin,
    TokenKind::KwInitial,
    TokenKind::KwShallowHistory,
    TokenKind::KwDeepHistory,
    TokenKind::KwFinal,
    TokenKind::At,
    TokenKind::StableId,
    TokenKind::DocComment,
    TokenKind::Eof,
]);

/// Parse an `action_list` introduced by `:`.
///
/// Depth-bounded — `if`/`while`/`for` bodies nest action blocks
/// recursively (Doc 00 §7.12 G-02 / audit P1-5).
pub fn parse_action_block(p: &mut Parser) {
    p.with_recursion((), |p| parse_action_block_inner(p));
}

fn parse_action_block_inner(p: &mut Parser) {
    p.start_node(SyntaxKind::ACTION_BLOCK);

    // The first statement is mandatory (the EBNF requires `statement` then
    // zero-or-more `; statement`). If we land on a terminator immediately,
    // emit a diagnostic but recover.
    if at_action_terminator(p) {
        p.error(DiagnosticCode::E0010, "expected statement after ':'");
        p.finish_node();
        return;
    }
    parse_statement(p);

    // Subsequent `; statement` repetitions.
    while p.at(TokenKind::Semicolon) {
        // Trailing semicolon before a terminator is permitted.
        let cp = p.checkpoint();
        p.bump(); // ;
        if at_action_terminator(p) {
            // It was the optional trailing `;` — done.
            let _ = cp; // checkpoint unused but cheap.
            break;
        }
        parse_statement(p);
    }

    p.finish_node();
}

/// `true` if the current token signals the end of an action block — used to
/// stop the statement loop without requiring an explicit closing token.
fn at_action_terminator(p: &Parser) -> bool {
    p.at_any_of(ACTION_BLOCK_TERMINATORS) || p.at(TokenKind::Eof)
}

/// Dispatch one statement. Statements end at `;` or an action-block
/// terminator; we do NOT consume the trailing `;` inside the statement
/// (the outer loop handles it).
fn parse_statement(p: &mut Parser) {
    // `if`, `while`, `for` are NOT reserved keywords in Doc 04 §1.5 — they
    // come in as `Ident`s with specific text. Check those first.
    if p.at(TokenKind::Ident) {
        match p.current_text() {
            "if" => return parse_if(p),
            "while" => return parse_while(p),
            "for" => return parse_for(p),
            _ => return parse_assign_or_call(p),
        }
    }
    match p.current() {
        TokenKind::KwRaise => parse_raise(p),
        TokenKind::KwSend => parse_send(p),
        TokenKind::KwDefer => parse_defer_stmt(p),
        _ => {
            p.error(
                DiagnosticCode::E0010,
                format!("expected statement, found '{}'", p.current()),
            );
            if !p.at(TokenKind::Eof) && !p.at(TokenKind::Semicolon) {
                p.err_and_bump(DiagnosticCode::E0010, "skipping unexpected token");
            }
        }
    }
}

/// Decide assign vs. call by looking ahead. Assignment has `IDENT = …` or
/// `field_ref = …`; call has `IDENT (` or bare `IDENT;`. The decision is
/// purely local: if we see `=` (after possibly walking through a `.IDENT`
/// chain) we go to assign; otherwise call.
fn parse_assign_or_call(p: &mut Parser) {
    // Walk past field-ref dots to find the first non-dot/ident token.
    let mut i = 0;
    // First token is the identifier.
    if !matches!(p.peek_n(i), TokenKind::Ident) {
        // Defensive: should not happen.
        return parse_call_stmt(p);
    }
    i += 1;
    while matches!(p.peek_n(i), TokenKind::Dot) && matches!(p.peek_n(i + 1), TokenKind::Ident) {
        i += 2;
    }
    if matches!(p.peek_n(i), TokenKind::Eq) {
        parse_assign(p);
    } else {
        parse_call_stmt(p);
    }
}

fn parse_assign(p: &mut Parser) {
    p.start_node(SyntaxKind::STMT_ASSIGN);
    // LHS = ctx.foo / payload.foo / bare-ident.
    // We use the Pratt parser to consume the field-ref / name; the
    // analyzer enforces that the lhs is a valid l-value (Doc 10 E0205).
    parse_expr(p, 0, ExprContext::Action);
    if !p.expect(TokenKind::Eq, DiagnosticCode::E0010) {
        p.finish_node();
        return;
    }
    parse_expr(p, 0, ExprContext::Action);
    p.finish_node();
}

fn parse_call_stmt(p: &mut Parser) {
    p.start_node(SyntaxKind::STMT_CALL);
    parse_expr(p, 0, ExprContext::Action);
    p.finish_node();
}

fn parse_raise(p: &mut Parser) {
    p.start_node(SyntaxKind::STMT_RAISE);
    p.bump(); // raise
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.at(TokenKind::LParen) {
        parse_paren_arg_list(p);
    }
    p.finish_node();
}

fn parse_send(p: &mut Parser) {
    p.start_node(SyntaxKind::STMT_SEND);
    p.bump(); // send
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.at(TokenKind::LParen) {
        parse_paren_arg_list(p);
    }
    p.expect(TokenKind::KwTo, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.finish_node();
}

fn parse_defer_stmt(p: &mut Parser) {
    p.start_node(SyntaxKind::STMT_DEFER);
    p.bump(); // defer
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.finish_node();
}

fn parse_if(p: &mut Parser) {
    p.start_node(SyntaxKind::STMT_IF);
    p.bump(); // `if` (contextual)
    p.expect(TokenKind::LParen, DiagnosticCode::E0010);
    parse_expr(p, 0, ExprContext::Action);
    p.expect(TokenKind::RParen, DiagnosticCode::E0010);
    parse_block_action(p);

    // Optional else.
    if p.at(TokenKind::KwElse) {
        p.start_node(SyntaxKind::STMT_ELSE);
        p.bump(); // else
        if p.at(TokenKind::Ident) && p.current_text() == "if" {
            parse_if(p);
        } else {
            parse_block_action(p);
        }
        p.finish_node();
    }
    p.finish_node();
}

fn parse_while(p: &mut Parser) {
    p.start_node(SyntaxKind::STMT_WHILE);
    p.bump(); // while (contextual)
    p.expect(TokenKind::LParen, DiagnosticCode::E0010);
    parse_expr(p, 0, ExprContext::Action);
    p.expect(TokenKind::RParen, DiagnosticCode::E0010);
    parse_block_action(p);
    p.finish_node();
}

fn parse_for(p: &mut Parser) {
    p.start_node(SyntaxKind::STMT_FOR);
    p.bump(); // for (contextual)
    p.expect(TokenKind::LParen, DiagnosticCode::E0010);
    // init assign
    parse_assign(p);
    p.expect(TokenKind::Semicolon, DiagnosticCode::E0010);
    // condition
    parse_expr(p, 0, ExprContext::Action);
    p.expect(TokenKind::Semicolon, DiagnosticCode::E0010);
    // step assign
    parse_assign(p);
    p.expect(TokenKind::RParen, DiagnosticCode::E0010);
    parse_block_action(p);
    p.finish_node();
}

/// Parse a brace-delimited inner action list — used by `if`/`while`/`for`.
///
/// Depth-bounded — `if` bodies may contain further nested `if`s with
/// their own brace blocks, so the recursion goes through here.
fn parse_block_action(p: &mut Parser) {
    p.with_recursion((), |p| parse_block_action_inner(p));
}

fn parse_block_action_inner(p: &mut Parser) {
    if !p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        return;
    }
    // Inside braces: action_list separated by `;`. Wrap in ACTION_BLOCK so
    // the AST shape stays uniform.
    p.start_node(SyntaxKind::ACTION_BLOCK);
    if !p.at(TokenKind::RBrace) {
        parse_statement(p);
        while p.at(TokenKind::Semicolon) {
            p.bump(); // ;
            if p.at(TokenKind::RBrace) {
                break;
            }
            parse_statement(p);
        }
    }
    p.finish_node();
    p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
}

fn parse_paren_arg_list(p: &mut Parser) {
    p.start_node(SyntaxKind::ARG_LIST);
    p.bump(); // (
    if !p.at(TokenKind::RParen) {
        parse_expr(p, 0, ExprContext::Action);
        while p.eat(TokenKind::Comma) {
            if p.at(TokenKind::RParen) {
                break;
            }
            parse_expr(p, 0, ExprContext::Action);
        }
    }
    p.expect(TokenKind::RParen, DiagnosticCode::E0010);
    p.finish_node();
}
