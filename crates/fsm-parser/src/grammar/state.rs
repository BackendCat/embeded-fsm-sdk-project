//! State declarations and pseudo-state forms.
//!
//! Per Doc 04 §5–§7:
//!
//! - `state_decl` — simple or composite state, body holds `state_item`s.
//! - `region_decl` — orthogonal region inside a composite state.
//! - `shallow_history_decl`, `deep_history_decl`, `choice_decl`,
//!   `junction_decl`, `fork_decl`, `join_decl`, `final_decl`,
//!   `entry_point_decl`, `exit_point_decl`.
//!
//! State items are dispatched in `parse_state_item` below.

use fsm_diagnostics::DiagnosticCode;
use fsm_lexer::TokenKind;

use crate::cst::SyntaxKind;
use crate::expr::parse_expr;
use crate::parser::{ExprContext, Parser};
use crate::token_set::TokenSet;

use super::guard::parse_guard_clause;
use super::stmt::parse_action_block;
use super::top_level::try_parse_stable_id;
use super::transition::parse_transition_from_on;

/// What may legally start a `state_item`. Used as the sync set when a body
/// rule explodes.
pub const STATE_ITEM_STARTS: TokenSet = TokenSet::new(&[
    TokenKind::KwOn,
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
    TokenKind::KwAfter,
    TokenKind::KwEvery,
    TokenKind::KwDefer,
    TokenKind::KwDone,
    TokenKind::At,
    TokenKind::StableId,
    TokenKind::DocComment,
    TokenKind::RBrace,
    TokenKind::Eof,
]);

/// `state_decl = [ "export" ] , "state" , identifier , "{" , { state_item } , "}" ;`
///
/// Depth-bounded — nested composite states drive recursion through
/// `parse_state_item -> parse_state_decl`. See [`Parser::with_recursion`]
/// (Doc 00 §7.12 G-02 / audit P1-5).
pub fn parse_state_decl(p: &mut Parser) {
    p.with_recursion((), |p| parse_state_decl_inner(p));
}

fn parse_state_decl_inner(p: &mut Parser) {
    p.start_node(SyntaxKind::STATE_DECL);
    let _ = p.eat(TokenKind::KwExport);
    p.expect(TokenKind::KwState, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    // Optional submachine reference: `state ID is SubName { ... }`
    // (Doc 04 §15). The `is SubName` binds the state's behaviour to a
    // named submachine instance; the body may still carry `done ->`
    // (fires on sub-instance completion) plus entry/exit. Modelled as an
    // optional SUBMACHINE_REF child of STATE_DECL — see kinds.rs.
    if p.at(TokenKind::KwIs) {
        parse_submachine_ref(p);
    }
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        while !p.at(TokenKind::RBrace) && !p.at(TokenKind::Eof) {
            let progressed_at = p.current_span().start;
            parse_state_item(p);
            if p.current_span().start == progressed_at && !p.at(TokenKind::Eof) {
                p.err_and_bump(DiagnosticCode::E0010, "unexpected token in state body");
            }
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node();
}

/// `submachine_ref = "is" , identifier ;` — the optional binding inside
/// `state ID is SubName { ... }` (Doc 04 §15). Wrapped in a SUBMACHINE_REF
/// node so the analyzer (W2b) can pick it up as a typed child of STATE_DECL
/// instead of token-scanning. A missing name (`state X is { }`) emits a
/// clear FSM-E0010 and the parser still finds the body; trailing junk
/// (`state X is Y Z {}`) is consumed up to the next `{` / state-item /
/// brace so recovery does not spin.
fn parse_submachine_ref(p: &mut Parser) {
    p.start_node(SyntaxKind::SUBMACHINE_REF);
    p.bump(); // is
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    // Anything other than `{` (or a body terminator) after `is SubName`
    // is junk — resync to the brace so the state body still parses.
    if !p.at(TokenKind::LBrace) && !p.at_any_of(SUBMACHINE_REF_RECOVER) {
        p.error_until(
            SUBMACHINE_REF_RECOVER,
            DiagnosticCode::E0010,
            "unexpected token after submachine reference name",
        );
    }
    p.finish_node();
}

/// Resync anchors after a malformed `is SubName …` — the state body brace,
/// any state-item starter, or a closing brace / EOF.
const SUBMACHINE_REF_RECOVER: TokenSet = TokenSet::new(&[
    TokenKind::LBrace,
    TokenKind::RBrace,
    TokenKind::KwState,
    TokenKind::KwOn,
    TokenKind::KwDone,
    TokenKind::Eof,
]);

/// Dispatch one `state_item`.
pub fn parse_state_item(p: &mut Parser) {
    // Optional stable-ID prefix.
    let _ = try_parse_stable_id(p);

    match p.current() {
        TokenKind::KwOn => parse_transition_from_on(p),
        TokenKind::KwDone => parse_completion_decl(p),
        TokenKind::KwAfter => parse_after_decl(p),
        TokenKind::KwEvery => parse_every_decl(p),
        TokenKind::KwDefer => parse_defer_decl(p),
        TokenKind::KwState | TokenKind::KwExport => parse_state_decl(p),
        TokenKind::KwRegion => parse_region_decl(p),
        TokenKind::KwChoice => parse_choice_decl(p),
        TokenKind::KwJunction => parse_junction_decl(p),
        TokenKind::KwFork => parse_fork_decl(p),
        TokenKind::KwJoin => parse_join_decl(p),
        TokenKind::KwShallowHistory => parse_shallow_history_decl(p),
        TokenKind::KwDeepHistory => parse_deep_history_decl(p),
        TokenKind::KwInitial => super::machine::parse_initial_decl(p),
        TokenKind::KwFinal => parse_final_decl(p),
        TokenKind::Ident => match p.current_text() {
            "entry" => parse_entry_decl(p),
            "exit" => parse_exit_decl(p),
            "entry_point" => parse_entry_point_decl(p),
            "exit_point" => parse_exit_point_decl(p),
            // v1.1-W4: optional `likely` / `rare` transition-prefix hint.
            // Contextual (the lexer emits these as `Ident`): only a hint
            // when the next significant token actually starts a transition
            // (`on` / `after` / `every` / `done`). Otherwise `likely`/`rare`
            // is an ordinary identifier used elsewhere — fall through to the
            // generic "unexpected ident" recovery exactly as before this
            // wave (back-compat: a state body never legally begins a
            // *non-transition* item with a bare ident, so the only behaviour
            // change is the new hint form).
            "likely" | "rare" if next_starts_transition(p) => parse_hinted_transition(p),
            _ => {
                p.error_until(
                    STATE_ITEM_STARTS,
                    DiagnosticCode::E0010,
                    format!("unexpected '{}' in state body", p.current_text()),
                );
            }
        },
        _ => {
            p.error_until(
                STATE_ITEM_STARTS,
                DiagnosticCode::E0010,
                format!("unexpected '{}' in state body", p.current()),
            );
        }
    }
}

// ─── v1.1-W4 branch hints (likely / rare) ────────────────────────────────

/// `true` when the token after the current `likely`/`rare` ident begins a
/// transition (`on` / `after` / `every` / `done`). Used to keep
/// `likely`/`rare` contextual — only a hint in transition-prefix position,
/// otherwise an ordinary identifier (back-compat). `peek_n` skips trivia, so
/// `likely   on TICK …` (any whitespace/comment between) resolves correctly.
fn next_starts_transition(p: &Parser) -> bool {
    matches!(
        p.peek_n(1),
        TokenKind::KwOn | TokenKind::KwAfter | TokenKind::KwEvery | TokenKind::KwDone
    )
}

/// Parse a `likely` / `rare` prefixed transition. The hint is wrapped in a
/// `BRANCH_HINT` node which becomes the FIRST child of the transition node:
/// we take the checkpoint *before* emitting BRANCH_HINT and hand it to the
/// underlying transition parser's `start_node_at`, so `likely on E -> T`
/// produces `TRANSITION_DECL [ BRANCH_HINT [likely] , E , T ]`. This mirrors
/// how SUBMACHINE_REF / GUARD_CLAUSE are optional children of their owning
/// node (the analyzer reads it as a typed child, never token-scans).
///
/// At most one hint prefix is admitted (the grammar has no production for a
/// second), so `likely rare on …` parses the first as the hint and the
/// second `rare` as the (then-unexpected) trigger position — a clean
/// FSM-E0010, never a both-hints IR.
fn parse_hinted_transition(p: &mut Parser) {
    let cp = p.checkpoint();
    // The hint node wraps exactly the `likely` / `rare` ident token.
    p.start_node(SyntaxKind::BRANCH_HINT);
    p.bump(); // `likely` | `rare` (ident)
    p.finish_node();

    match p.current() {
        TokenKind::KwOn => {
            super::transition::parse_transition_from_on_at(p, Some(cp));
        }
        TokenKind::KwAfter => parse_after_decl_at(p, Some(cp)),
        TokenKind::KwEvery => parse_every_decl_at(p, Some(cp)),
        TokenKind::KwDone => parse_completion_decl_at(p, Some(cp)),
        _ => {
            // Unreachable in practice: the caller gated on
            // `next_starts_transition`. Defensive recovery keeps the parser
            // total — wrap whatever follows so the hint token is not
            // orphaned and recovery does not spin.
            p.start_node_at(cp, SyntaxKind::TRANSITION_DECL);
            p.error(
                DiagnosticCode::E0010,
                format!(
                    "expected a transition after '{}' hint, found '{}'",
                    "likely/rare",
                    p.current()
                ),
            );
            p.finish_node();
        }
    }
}

// ─── entry / exit ────────────────────────────────────────────────────────

fn parse_entry_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::ENTRY_DECL);
    p.bump(); // `entry` (ident)
    p.expect(TokenKind::Colon, DiagnosticCode::E0010);
    parse_action_block(p);
    p.finish_node();
}

fn parse_exit_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::EXIT_DECL);
    p.bump(); // `exit` (ident)
    p.expect(TokenKind::Colon, DiagnosticCode::E0010);
    parse_action_block(p);
    p.finish_node();
}

// ─── final / entry_point / exit_point ───────────────────────────────────

pub fn parse_final_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::FINAL_DECL);
    p.bump(); // final
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.finish_node();
}

/// `entry_point_decl = "entry_point" , identifier , "->" , identifier ;`
pub fn parse_entry_point_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::ENTRY_POINT_DECL);
    p.bump(); // `entry_point` (contextual)
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.expect(TokenKind::Arrow, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.finish_node();
}

/// `exit_point_decl = "exit_point" , identifier ;`
pub fn parse_exit_point_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::EXIT_POINT_DECL);
    p.bump(); // `exit_point` (contextual)
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.finish_node();
}

// ─── regions ─────────────────────────────────────────────────────────────

/// `region_decl = "region" , identifier , "{" , { region_item } , "}" ;`
///
/// Depth-bounded — regions live inside composite states and may contain
/// further nested states; the recursion mirrors `parse_state_decl`.
pub fn parse_region_decl(p: &mut Parser) {
    p.with_recursion((), |p| parse_region_decl_inner(p));
}

fn parse_region_decl_inner(p: &mut Parser) {
    p.start_node(SyntaxKind::REGION_DECL);
    p.bump(); // region
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        while !p.at(TokenKind::RBrace) && !p.at(TokenKind::Eof) {
            let progressed_at = p.current_span().start;
            parse_state_item(p); // region_item ⊆ state_item — analyzer flags
                                 // illegal forms; parser accepts the union.
            if p.current_span().start == progressed_at && !p.at(TokenKind::Eof) {
                p.err_and_bump(DiagnosticCode::E0010, "unexpected token in region body");
            }
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node();
}

// ─── history ─────────────────────────────────────────────────────────────

/// `shallow_history_decl = "shallow_history" , identifier , "{" , initial_decl , "}" ;`
pub fn parse_shallow_history_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::SHALLOW_HISTORY_DECL);
    p.bump(); // shallow_history
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        // Per Doc 00 §7.10 (B-14), the parser still ACCEPTS history without
        // a default — the analyzer raises E0111 with a span-accurate error.
        if p.at(TokenKind::KwInitial) {
            super::machine::parse_initial_decl(p);
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node();
}

pub fn parse_deep_history_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::DEEP_HISTORY_DECL);
    p.bump(); // deep_history
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        if p.at(TokenKind::KwInitial) {
            super::machine::parse_initial_decl(p);
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node();
}

// ─── choice / junction ───────────────────────────────────────────────────

/// `choice_decl = "choice" , identifier , "{" , choice_branch+ , "}" ;`
///
/// `choice_branch = "[" , guard_expr , "]" , "->" , identifier ,
///                  [ ":" , action_list ] ;`
pub fn parse_choice_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::CHOICE_DECL);
    p.bump(); // choice
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        while p.at(TokenKind::LBracket) {
            parse_choice_branch(p);
        }
        if !p.at(TokenKind::RBrace) && !p.at(TokenKind::Eof) {
            p.error_until(
                CHOICE_RECOVER,
                DiagnosticCode::E0010,
                "expected choice branch or '}'",
            );
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node();
}

const CHOICE_RECOVER: TokenSet =
    TokenSet::new(&[TokenKind::RBrace, TokenKind::LBracket, TokenKind::Eof]);

fn parse_choice_branch(p: &mut Parser) {
    p.start_node(SyntaxKind::CHOICE_BRANCH);
    // Guard clause `[expr]`.
    if p.at(TokenKind::LBracket) {
        p.start_node(SyntaxKind::GUARD_CLAUSE);
        p.bump(); // [
        parse_expr(p, 0, ExprContext::Guard);
        p.expect(TokenKind::RBracket, DiagnosticCode::E0010);
        p.finish_node();
    }
    p.expect(TokenKind::Arrow, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.eat(TokenKind::Colon) {
        parse_action_block(p);
    }
    p.finish_node();
}

pub fn parse_junction_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::JUNCTION_DECL);
    p.bump(); // junction
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        while p.at(TokenKind::LBracket) {
            p.start_node(SyntaxKind::JUNCTION_BRANCH);
            parse_choice_branch_inner(p);
            p.finish_node();
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node();
}

fn parse_choice_branch_inner(p: &mut Parser) {
    if p.at(TokenKind::LBracket) {
        p.start_node(SyntaxKind::GUARD_CLAUSE);
        p.bump();
        parse_expr(p, 0, ExprContext::Guard);
        p.expect(TokenKind::RBracket, DiagnosticCode::E0010);
        p.finish_node();
    }
    p.expect(TokenKind::Arrow, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.eat(TokenKind::Colon) {
        parse_action_block(p);
    }
}

// ─── fork / join ─────────────────────────────────────────────────────────

/// `fork_decl = "fork" , identifier , "->" , "{" , identifier { "," identifier } , "}" ;`
pub fn parse_fork_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::FORK_DECL);
    p.bump(); // fork
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.expect(TokenKind::Arrow, DiagnosticCode::E0010);
    p.start_node(SyntaxKind::FORK_TARGETS);
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        if p.at(TokenKind::Ident) {
            p.bump();
            while p.eat(TokenKind::Comma) {
                if p.at(TokenKind::RBrace) {
                    break;
                }
                p.expect(TokenKind::Ident, DiagnosticCode::E0010);
            }
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node(); // FORK_TARGETS
    p.finish_node(); // FORK_DECL
}

/// `join_decl = "join" , identifier , "{" , identifier {"," identifier} , "}" ,
///              "->" , identifier ;`
pub fn parse_join_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::JOIN_DECL);
    p.bump(); // join
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.start_node(SyntaxKind::JOIN_SOURCES);
    if p.expect(TokenKind::LBrace, DiagnosticCode::E0010) {
        if p.at(TokenKind::Ident) {
            p.bump();
            while p.eat(TokenKind::Comma) {
                if p.at(TokenKind::RBrace) {
                    break;
                }
                p.expect(TokenKind::Ident, DiagnosticCode::E0010);
            }
        }
        p.expect(TokenKind::RBrace, DiagnosticCode::E0010);
    }
    p.finish_node(); // JOIN_SOURCES
    p.expect(TokenKind::Arrow, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.finish_node();
}

// ─── completion / after / every / defer ──────────────────────────────────

/// `completion_decl = "done" , [ guard_clause ] , [ priority_clause ] ,
///                    "->" , identifier , [ ":" , action_list ] ;`
///
/// Per Doc 00 §B-07, guards on completion transitions ARE permitted.
fn parse_completion_decl(p: &mut Parser) {
    parse_completion_decl_at(p, None);
}

/// As [`parse_completion_decl`]; `outer_cp` (v1.1-W4) lets a preceding
/// `BRANCH_HINT` node be enclosed as the COMPLETION_DECL's first child.
/// `None` ⇒ checkpoint here ⇒ byte-identical CST to pre-W4.
fn parse_completion_decl_at(p: &mut Parser, outer_cp: Option<rowan::Checkpoint>) {
    let cp = outer_cp.unwrap_or_else(|| p.checkpoint());
    p.bump(); // done
    if p.at(TokenKind::LBracket) {
        parse_guard_clause(p);
    }
    if p.at(TokenKind::KwPriority) {
        parse_priority_clause(p);
    }
    p.start_node_at(cp, SyntaxKind::COMPLETION_DECL);
    p.expect(TokenKind::Arrow, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.eat(TokenKind::Colon) {
        parse_action_block(p);
    }
    p.finish_node();
}

/// `after_decl = "after" , const_expr , "ms" , "->" , identifier , [ ":" , action_list ] ;`
fn parse_after_decl(p: &mut Parser) {
    parse_after_decl_at(p, None);
}

/// As [`parse_after_decl`]; `outer_cp` (v1.1-W4) encloses a preceding
/// `BRANCH_HINT` as the AFTER_DECL's first child. `None` ⇒ pre-W4 shape.
fn parse_after_decl_at(p: &mut Parser, outer_cp: Option<rowan::Checkpoint>) {
    let cp = outer_cp.unwrap_or_else(|| p.checkpoint());
    p.bump(); // after
    p.start_node(SyntaxKind::CONST_EXPR);
    parse_expr(p, 0, ExprContext::Action);
    p.finish_node();
    p.expect(TokenKind::KwMs, DiagnosticCode::E0010);
    p.start_node_at(cp, SyntaxKind::AFTER_DECL);
    p.expect(TokenKind::Arrow, DiagnosticCode::E0010);
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    if p.eat(TokenKind::Colon) {
        parse_action_block(p);
    }
    p.finish_node();
}

/// `every_decl = "every" , const_expr , "ms" , "->" , identifier , [ ":" , action_list ] ;`
/// `every_internal_decl = "every" , const_expr , "ms" , ":" , action_list ;`
///
/// Distinguish by looking at what comes after the `ms` keyword.
fn parse_every_decl(p: &mut Parser) {
    parse_every_decl_at(p, None);
}

/// As [`parse_every_decl`]; `outer_cp` (v1.1-W4) encloses a preceding
/// `BRANCH_HINT` as the EVERY_DECL / EVERY_INTERNAL_DECL first child.
/// `None` ⇒ checkpoint taken here ⇒ byte-identical pre-W4 CST.
fn parse_every_decl_at(p: &mut Parser, outer_cp: Option<rowan::Checkpoint>) {
    let cp = outer_cp.unwrap_or_else(|| p.checkpoint());
    p.bump(); // every
    p.start_node(SyntaxKind::CONST_EXPR);
    parse_expr(p, 0, ExprContext::Action);
    p.finish_node();
    p.expect(TokenKind::KwMs, DiagnosticCode::E0010);

    // Disambiguate: `:` means internal; `->` means transition.
    if p.at(TokenKind::Colon) {
        p.start_node_at(cp, SyntaxKind::EVERY_INTERNAL_DECL);
        p.bump(); // :
        parse_action_block(p);
        p.finish_node();
    } else {
        p.start_node_at(cp, SyntaxKind::EVERY_DECL);
        p.expect(TokenKind::Arrow, DiagnosticCode::E0010);
        p.expect(TokenKind::Ident, DiagnosticCode::E0010);
        if p.eat(TokenKind::Colon) {
            parse_action_block(p);
        }
        p.finish_node();
    }
}

/// `defer_decl = "defer" , identifier ;`
fn parse_defer_decl(p: &mut Parser) {
    p.start_node(SyntaxKind::DEFER_DECL);
    p.bump(); // defer
    p.expect(TokenKind::Ident, DiagnosticCode::E0010);
    p.finish_node();
}

// ─── shared helpers ──────────────────────────────────────────────────────

pub(super) fn parse_priority_clause(p: &mut Parser) {
    p.start_node(SyntaxKind::PRIORITY_CLAUSE);
    p.bump(); // priority
    p.expect(TokenKind::IntLiteral, DiagnosticCode::E0010);
    p.finish_node();
}
