//! Machine body — the inside of `machine NAME { … }`.
//!
//! Per Doc 04 §4:
//!
//! ```ebnf
//! machine_item =
//!       context_block
//!     | events_block
//!     | queue_block
//!     | target_block
//!     | state_decl
//!     | region_decl
//!     | choice_decl
//!     | junction_decl
//!     | fork_decl
//!     | join_decl
//!     | initial_decl
//!     | extern_decl ;
//! ```
//!
//! The block keywords (`context`, `events`, `queue`, `target`) are not
//! reserved keywords in the lexer table — Doc 04 §1.5 lists `context`,
//! `target` as keywords but `events` and `queue` are *not* reserved
//! ([cross-checked the §1.5 table]; only `context` has a dedicated
//! `KwContext`). For contextual keywords without a `Kw*` variant, the
//! parser matches by token text.

use fsm_diagnostics::DiagnosticCode;
use fsm_lexer::TokenKind;

use crate::cst::SyntaxKind;
use crate::expr::{parse_expr, parse_type_ref};
use crate::parser::{ExprContext, Parser};
use crate::token_set::TokenSet;

use super::state::parse_state_item;
use super::top_level::{parse_extern_decl, try_parse_stable_id};

/// Sync set used inside the machine body when a `machine_item` parse blows
/// up. Anchors recovery at the next item-starter or the closing brace.
pub const MACHINE_ITEM_STARTS: TokenSet = TokenSet::new(&[
    TokenKind::KwContext,
    TokenKind::KwState,
    TokenKind::KwTarget,
    TokenKind::KwInitial,
    TokenKind::KwExtern,
    TokenKind::KwPure,
    TokenKind::KwRegion,
    TokenKind::KwChoice,
    TokenKind::KwJunction,
    TokenKind::KwFork,
    TokenKind::KwJoin,
    TokenKind::KwShallowHistory,
    TokenKind::KwDeepHistory,
    TokenKind::KwFinal,
    TokenKind::At,
    TokenKind::StableId,
    TokenKind::DocComment,
    TokenKind::RBrace,
    TokenKind::Eof,
]);

/// Parse the inside of a `machine { … }` block. The opening `{` has been
/// consumed; this routine consumes items up to the closing `}`.
pub fn parse_machine_body(p: &mut Parser) {
    while !p.at(TokenKind::RBrace) && !p.at(TokenKind::Eof) {
        let progressed_at = p.current_span().start;

        // Optional stable-ID first.
        let _ = try_parse_stable_id(p);

        match p.current() {
            TokenKind::KwContext => parse_context_block(p),
            TokenKind::KwTarget => parse_target_block(p),
            TokenKind::KwExtern | TokenKind::KwPure => parse_extern_decl(p),
            TokenKind::KwInitial => parse_initial_decl(p),
            TokenKind::KwState | TokenKind::KwExport => super::state::parse_state_decl(p),
            TokenKind::KwRegion => super::state::parse_region_decl(p),
            TokenKind::KwChoice => super::state::parse_choice_decl(p),
            TokenKind::KwJunction => super::state::parse_junction_decl(p),
            TokenKind::KwFork => super::state::parse_fork_decl(p),
            TokenKind::KwJoin => super::state::parse_join_decl(p),
            TokenKind::KwShallowHistory => super::state::parse_shallow_history_decl(p),
            TokenKind::KwDeepHistory => super::state::parse_deep_history_decl(p),
            TokenKind::KwFinal => super::state::parse_final_decl(p),
            TokenKind::Ident => {
                // Contextual blocks: `events { … }`, `queue { … }`.
                match p.current_text() {
                    "events" => parse_events_block(p),
                    "queue" => parse_queue_block(p),
                    "entry_point" => super::state::parse_entry_point_decl(p),
                    "exit_point" => super::state::parse_exit_point_decl(p),
                    _ => {
                        p.error_until(
                            MACHINE_ITEM_STARTS,
                            DiagnosticCode::E0010,
                            format!("unexpected '{}' in machine body", p.current_text()),
                        );
                    }
                }
            }
            _ => {
                p.error_until(
                    MACHINE_ITEM_STARTS,
                    DiagnosticCode::E0010,
                    format!("unexpected '{}' in machine body", p.current()),
                );
            }
        }

        if p.current_span().start == progressed_at && !p.at(TokenKind::Eof) {
            p.err_and_bump(DiagnosticCode::E0010, "unexpected token in machine body");
        }
    }
}

/// `context_block = "context" , "{" , { field_decl } , "}" ;`
fn parse_context_block(p: &mut Parser) {
    p.start_node(SyntaxKind::CONTEXT_BLOCK);
    p.bump(); // context
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        while !p.at(TokenKind::RBrace) && !p.at(TokenKind::Eof) {
            let progressed_at = p.current_span().start;
            if p.at(TokenKind::Ident) {
                parse_field_decl(p);
            } else {
                p.err_and_bump(DiagnosticCode::E0010, "expected field declaration");
            }
            if p.current_span().start == progressed_at && !p.at(TokenKind::Eof) {
                p.err_and_bump(DiagnosticCode::E0010, "unexpected token in context");
            }
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node();
}

/// `field_decl = identifier , ":" , type , [ "=" , const_expr ] ;`
fn parse_field_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::FIELD_DECL);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.expect(TokenKind::Colon, DiagnosticCode::E0010);
    parse_type_ref(p);
    if p.eat(TokenKind::Eq) {
        p.start_node(SyntaxKind::CONST_EXPR);
        parse_expr(p, 0, ExprContext::Action);
        p.finish_node();
    }
    p.finish_node();
}

/// `events_block = "events" , "{" , { event_decl } , "}" ;`
fn parse_events_block(p: &mut Parser) {
    p.start_node(SyntaxKind::EVENTS_BLOCK);
    // `events` is a contextual keyword: lexed as Ident.
    p.bump();
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        while !p.at(TokenKind::RBrace) && !p.at(TokenKind::Eof) {
            let progressed_at = p.current_span().start;

            let _ = try_parse_stable_id(p);
            // event_decl is just an identifier optionally followed by a
            // payload list — no leading keyword, so it starts with Ident.
            if p.at(TokenKind::Ident) {
                parse_event_decl(p);
            } else {
                p.err_and_bump(DiagnosticCode::E0010, "expected event declaration");
            }
            if p.current_span().start == progressed_at && !p.at(TokenKind::Eof) {
                p.err_and_bump(DiagnosticCode::E0010, "unexpected token in events");
            }
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node();
}

/// `event_decl = identifier , [ "(" , payload_field , {"," payload_field} , ")" ] ;`
fn parse_event_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::EVENT_DECL);
    p.bump(); // event identifier
    if p.at(TokenKind::LParen) {
        p.start_node(SyntaxKind::PAYLOAD_LIST);
        p.bump(); // (
        if !p.at(TokenKind::RParen) {
            parse_payload_field(p);
            while p.eat(TokenKind::Comma) {
                if p.at(TokenKind::RParen) {
                    break;
                }
                parse_payload_field(p);
            }
        }
        p.expect(TokenKind::RParen, DiagnosticCode::E0010);
        p.finish_node();
    }
    p.finish_node();
}

fn parse_payload_field(p: &mut Parser) {
    p.start_node(SyntaxKind::PAYLOAD_FIELD);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.expect(TokenKind::Colon, DiagnosticCode::E0010);
    parse_type_ref(p);
    p.finish_node();
}

/// `queue_block = "queue" , "{" , { config_entry } , "}" ;`
fn parse_queue_block(p: &mut Parser) {
    p.start_node(SyntaxKind::QUEUE_BLOCK);
    p.bump(); // queue (contextual)
    parse_config_body(p);
    p.finish_node();
}

/// `target_block = "target" , identifier , "{" , { config_entry } , "}" ;`
fn parse_target_block(p: &mut Parser) {
    p.start_node(SyntaxKind::TARGET_BLOCK);
    p.bump(); // target
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    parse_config_body(p);
    p.finish_node();
}

fn parse_config_body(p: &mut Parser) {
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        while !p.at(TokenKind::RBrace) && !p.at(TokenKind::Eof) {
            let progressed_at = p.current_span().start;
            if p.at(TokenKind::Ident) {
                parse_config_entry(p);
            } else {
                p.err_and_bump(DiagnosticCode::E0010, "expected config entry");
            }
            if p.current_span().start == progressed_at && !p.at(TokenKind::Eof) {
                p.err_and_bump(DiagnosticCode::E0010, "unexpected token in config block");
            }
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
}

/// `config_entry = identifier , "=" , ( integer | boolean | identifier ) ;`
fn parse_config_entry(p: &mut Parser) {
    p.start_node(SyntaxKind::CONFIG_ENTRY);
    p.bump(); // key ident
    p.expect(TokenKind::Eq, DiagnosticCode::E0010);
    match p.current() {
        TokenKind::IntLiteral | TokenKind::KwTrue | TokenKind::KwFalse | TokenKind::Ident => {
            p.bump();
        }
        _ => {
            p.error(
                DiagnosticCode::E0010,
                format!(
                    "expected integer, boolean or identifier in config entry, found '{}'",
                    p.current()
                ),
            );
        }
    }
    p.finish_node();
}

/// `initial_decl = "initial" , identifier ;`
pub fn parse_initial_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::INITIAL_DECL);
    p.bump(); // initial
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.finish_node();
}

#[allow(dead_code)]
fn _unused_silence(_p: &mut Parser, _: TokenSet) {
    // Suppress unused-import warning on TokenSet for now; remove when the
    // unused symbol gets a real consumer.
    let _ = parse_state_item;
}
