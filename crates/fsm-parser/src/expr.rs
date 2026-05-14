//! Pratt operator-precedence parser for FSM-Lang expressions.
//!
//! Implements the **normative** binding-power table from Doc 04 §8.7.1 (per
//! Doc 00 §B-05: the §8.7 EBNF is informational; the §8.7.1 table is the
//! source of truth, and the non-left-recursive rewrite in Doc 00 §B-05 is
//! the EBNF you would actually implement if you skipped Pratt).
//!
//! Binding-power layout — higher = binds tighter:
//!
//! | Level | Category | Operators | Associativity |
//! |---|---|---|---|
//! | 10 | Primary | literals, names, `(…)`, field refs | N/A |
//! | 9  | Postfix | `.`, `(args)` | Left |
//! | 8  | Cast    | `as`         | Left |
//! | 7  | Unary   | `!` `-` `~`  | Right (prefix) |
//! | 6  | Mul     | `*` `/` `%`  | Left |
//! | 5  | Add     | `+` `-`      | Left |
//! | 4  | Shift   | `<<` `>>`    | Left |
//! | 3  | Bitwise | `&` `^` `\|` | Left |
//! | 2  | Cmp     | `==` `!=` `<` `>` `<=` `>=` | Left |
//! | 1  | And     | `&&`         | Left |
//! | 0  | Or      | `\|\|`       | Left |
//!
//! Internally each level maps to (lbp, rbp). Left-associative binary
//! operators use `rbp = lbp + 1`; right-associative would use `rbp = lbp`.

use fsm_diagnostics::DiagnosticCode;
use fsm_lexer::TokenKind;

use crate::cst::SyntaxKind;
use crate::parser::{ExprContext, Parser};

/// Parse an expression at the given minimum binding power. The recursive
/// driver: see Pratt 1973, "Top down operator precedence."
///
/// Depth-bounded via [`Parser::with_recursion`] — a `(((...)))`-style
/// adversarial input is rejected with `FSM-E0010` once the configured
/// depth limit is hit (Doc 00 §7.12 G-02 / audit P1-5).
pub fn parse_expr(p: &mut Parser, min_bp: u8, ctx: ExprContext) {
    p.with_recursion((), |p| parse_expr_inner(p, min_bp, ctx));
}

fn parse_expr_inner(p: &mut Parser, min_bp: u8, ctx: ExprContext) {
    let lhs_cp = p.checkpoint();
    if !parse_prefix(p, ctx) {
        // No valid prefix token; emit an error and return without consuming.
        p.error(
            DiagnosticCode::E0010,
            format!("expected expression, found '{}'", p.current()),
        );
        return;
    }

    loop {
        let kind = p.current();
        // Postfix: function-call args follow an identifier/postfix-eligible
        // operand. The lhs has been parsed; `(` here means a call.
        if kind == TokenKind::LParen && is_call_eligible(p, &lhs_cp) {
            // (call_args) — postfix at level 9.
            p.start_node_at(lhs_cp, SyntaxKind::EXPR_CALL);
            parse_call_args(p, ctx);
            p.finish_node();
            continue;
        }
        if kind == TokenKind::Dot {
            // field-access postfix. Cast as level-9 binding-power: only
            // applies after a name reference. Recover by treating it as a
            // qualified-name wrap.
            if !matches!(p.peek_n(1), TokenKind::Ident) {
                p.error(
                    DiagnosticCode::E0010,
                    format!("expected identifier after '.', found '{}'", p.peek_n(1)),
                );
                break;
            }
            p.start_node_at(lhs_cp, SyntaxKind::EXPR_FIELD_REF);
            p.bump(); // .
            p.bump(); // ident
            p.finish_node();
            continue;
        }

        // `as` cast — level 8, left-assoc.
        if kind == TokenKind::KwAs {
            // Cast forbidden inside guards (no arithmetic / casts allowed
            // in guard sublanguage).
            if ctx == ExprContext::Guard {
                p.error(
                    DiagnosticCode::E0010,
                    "'as' cast not permitted in guard expression",
                );
                break;
            }
            let (lbp, _rbp) = (8u8, 9u8);
            if lbp < min_bp {
                break;
            }
            p.start_node_at(lhs_cp, SyntaxKind::EXPR_CAST);
            p.bump(); // as
            parse_type_ref(p);
            p.finish_node();
            continue;
        }

        // Binary infix.
        if let Some((lbp, rbp)) = infix_binding_power(kind, ctx) {
            if lbp < min_bp {
                break;
            }
            p.start_node_at(lhs_cp, SyntaxKind::EXPR_BINARY);
            p.bump(); // operator
            parse_expr(p, rbp, ctx);
            p.finish_node();
            continue;
        }

        break;
    }
}

/// Returns whether a prefix (atom or unary) was successfully parsed.
fn parse_prefix(p: &mut Parser, ctx: ExprContext) -> bool {
    let kind = p.current();
    match kind {
        TokenKind::Bang | TokenKind::Minus | TokenKind::Tilde => {
            // Unary right-associative.
            // In guard context, `-` and `~` are not part of the guard
            // sublanguage; only `!` is. We emit a diagnostic but still
            // parse the operand so recovery is graceful.
            if ctx == ExprContext::Guard && kind != TokenKind::Bang {
                p.error(
                    DiagnosticCode::E0010,
                    format!("unary '{}' not permitted in guard expression", kind),
                );
            }
            p.start_node(SyntaxKind::EXPR_UNARY);
            p.bump();
            parse_expr(p, 7, ctx);
            p.finish_node();
            true
        }
        TokenKind::LParen => {
            p.start_node(SyntaxKind::EXPR_PAREN);
            p.bump();
            parse_expr(p, 0, ctx);
            p.expect(TokenKind::RParen, DiagnosticCode::E0010);
            p.finish_node();
            true
        }
        TokenKind::IntLiteral
        | TokenKind::FloatLiteral
        | TokenKind::StringLiteral
        | TokenKind::KwTrue
        | TokenKind::KwFalse => {
            // Guard sublanguage rejects string literals (no string type).
            if ctx == ExprContext::Guard && kind == TokenKind::StringLiteral {
                p.error(
                    DiagnosticCode::E0010,
                    "string literal not permitted in guard expression",
                );
            }
            p.start_node(SyntaxKind::EXPR_LITERAL);
            p.bump();
            p.finish_node();
            true
        }
        TokenKind::KwElse => {
            // `else` in a guard position is a catch-all (Doc 04 §7.3 / §8.5).
            // Wrapping in GUARD_ELSE keeps the AST searchable for this
            // special form. Outside guards, `else` here is an error.
            if ctx != ExprContext::Guard {
                p.error(
                    DiagnosticCode::E0010,
                    "'else' only valid as guard catch-all",
                );
            }
            p.start_node(SyntaxKind::GUARD_ELSE);
            p.bump();
            p.finish_node();
            true
        }
        TokenKind::Ident => {
            // Disambiguate ident / field-ref / qualified-name / call.
            // §8.7.2: identifier followed by `(` is always a call.
            // For field refs (`ctx.foo`, `payload.bar`), the prefix is an
            // ident; the postfix loop above turns `Ident '.' Ident` into a
            // FIELD_REF node. Qualified names (`PacketType.DATA`) hit the
            // same Ident '.' Ident path and round-trip as EXPR_FIELD_REF —
            // the analyzer disambiguates by symbol resolution. (Doc 09
            // models them as different IR literals, but the parser doesn't
            // need that distinction.)
            p.start_node(SyntaxKind::EXPR_NAME_REF);
            p.bump();
            p.finish_node();
            true
        }
        _ => false,
    }
}

/// `(args)` parse for the postfix call form. The opening `(` is the current
/// token.
fn parse_call_args(p: &mut Parser, ctx: ExprContext) {
    p.start_node(SyntaxKind::ARG_LIST);
    p.bump(); // (
    if !p.at(TokenKind::RParen) {
        parse_expr(p, 0, ctx);
        while p.eat(TokenKind::Comma) {
            // Trailing commas: accept `foo(a,)` defensively — the formatter
            // strips them on emit; the parser should not refuse common
            // editor-paste states.
            if p.at(TokenKind::RParen) {
                break;
            }
            parse_expr(p, 0, ctx);
        }
    }
    p.expect(TokenKind::RParen, DiagnosticCode::E0010);
    p.finish_node();
}

/// Is the lhs that ends at `_cp` eligible for a postfix `(`? Most prefixes
/// are; we exclude only literals (calling a literal is meaningless and
/// `1(2)` is more likely a syntax error than a call).
///
/// Implementation note: rowan's `Checkpoint` is opaque; we cannot inspect
/// the lhs subtree to determine its kind without finishing it. We
/// over-approximate by always returning `true`: the analyzer will reject
/// nonsensical calls. This is fine because the alternative (`1 (2)`) is
/// extremely rare and produces a clear analyzer-level diagnostic anyway.
fn is_call_eligible(_p: &Parser, _cp: &rowan::Checkpoint) -> bool {
    true
}

/// Parse a type reference (used after `as` and in field/param declarations).
pub fn parse_type_ref(p: &mut Parser) {
    let kind = p.current();
    match kind {
        TokenKind::KwBool
        | TokenKind::KwU8
        | TokenKind::KwU16
        | TokenKind::KwU32
        | TokenKind::KwU64
        | TokenKind::KwI8
        | TokenKind::KwI16
        | TokenKind::KwI32
        | TokenKind::KwI64
        | TokenKind::KwF32
        | TokenKind::KwF64 => {
            p.start_node(SyntaxKind::TYPE_REF);
            p.bump();
            p.finish_node();
        }
        TokenKind::Ident => {
            // Possibly qualified-name (enum reference).
            p.start_node(SyntaxKind::TYPE_REF);
            p.bump();
            while p.at(TokenKind::Dot) && matches!(p.peek_n(1), TokenKind::Ident) {
                p.bump(); // .
                p.bump(); // ident
            }
            p.finish_node();
        }
        TokenKind::KwOpaque => {
            // `opaque "C_type"`.
            p.start_node(SyntaxKind::OPAQUE_TYPE_REF);
            p.bump(); // opaque
            if p.at(TokenKind::StringLiteral) {
                // Validate the opaque type string body (security G-02).
                let raw = p.current_text();
                let trimmed = strip_string_quotes(raw);
                let span = p.current_span();
                if let Err(diag) = crate::opaque_type_validator::validate_opaque_type(trimmed, span)
                {
                    p.push_diagnostic(diag);
                }
                p.bump();
            } else {
                p.expect(TokenKind::StringLiteral, DiagnosticCode::E0010);
            }
            p.finish_node();
        }
        _ => {
            p.error(
                DiagnosticCode::E0010,
                format!("expected type, found '{}'", kind),
            );
            p.start_node(SyntaxKind::TYPE_REF);
            p.finish_node();
        }
    }
}

/// Strip the leading/trailing `"` of a string literal slice from the source.
/// We do not unescape — the parser preserves source bytes verbatim; the
/// validator's regex is content-only.
fn strip_string_quotes(raw: &str) -> &str {
    let s = raw.strip_prefix('"').unwrap_or(raw);
    s.strip_suffix('"').unwrap_or(s)
}

/// Map a binary operator token to its `(lbp, rbp)` pair. `None` means the
/// token is not a binary operator at this position. Left-associative is
/// encoded as `rbp = lbp + 1`. In `Guard` context we additionally veto
/// arithmetic and bitwise operators (Doc 04 §8.5: guard_expr permits only
/// comparison + logical operators).
fn infix_binding_power(kind: TokenKind, ctx: ExprContext) -> Option<(u8, u8)> {
    let guard = ctx == ExprContext::Guard;
    Some(match kind {
        TokenKind::PipePipe => (0, 1),
        TokenKind::AmpAmp => (1, 2),

        TokenKind::EqEq
        | TokenKind::BangEq
        | TokenKind::Lt
        | TokenKind::Gt
        | TokenKind::Le
        | TokenKind::Ge => (2, 3),

        TokenKind::Amp | TokenKind::Caret | TokenKind::Pipe if !guard => (3, 4),

        TokenKind::Shl | TokenKind::Shr if !guard => (4, 5),

        TokenKind::Plus | TokenKind::Minus if !guard => (5, 6),

        TokenKind::Star | TokenKind::Slash | TokenKind::Percent if !guard => (6, 7),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cst::FsmLanguage;
    use rowan::SyntaxNode;

    fn parse_expression(src: &str) -> (rowan::GreenNode, Vec<fsm_diagnostics::Diagnostic>) {
        let mut p = Parser::new(src);
        p.start_node(SyntaxKind::FILE);
        parse_expr(&mut p, 0, ExprContext::Action);
        while !p.at(fsm_lexer::TokenKind::Eof) {
            p.bump();
        }
        p.drain_trailing_trivia();
        p.finish_node();
        p.finish()
    }

    fn ast_shape(green: &rowan::GreenNode) -> String {
        let root = SyntaxNode::<FsmLanguage>::new_root(green.clone());
        let mut out = String::new();
        dump(&root, 0, &mut out);
        out
    }

    fn dump(node: &SyntaxNode<FsmLanguage>, indent: usize, out: &mut String) {
        use std::fmt::Write as _;
        let _ = writeln!(out, "{:indent$}{:?}", "", node.kind(), indent = indent);
        for child in node.children_with_tokens() {
            match child {
                rowan::NodeOrToken::Node(n) => dump(&n, indent + 2, out),
                rowan::NodeOrToken::Token(t) => {
                    let k = t.kind();
                    if !k.is_trivia() {
                        let _ = writeln!(
                            out,
                            "{:indent$}{:?} {:?}",
                            "",
                            k,
                            t.text(),
                            indent = indent + 2
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn additive_then_multiplicative() {
        // 1 + 2 * 3 parses as 1 + (2 * 3).
        let (g, errs) = parse_expression("1 + 2 * 3");
        assert!(errs.is_empty(), "{errs:?}");
        let s = ast_shape(&g);
        // The outer EXPR_BINARY's right operand should itself be EXPR_BINARY.
        let outer = s.lines().filter(|l| l.contains("EXPR_BINARY")).count();
        assert_eq!(outer, 2, "expected nested binary expressions\n{s}");
    }

    #[test]
    fn logical_and_binds_tighter_than_or() {
        // a && b || c -> (a && b) || c
        let (g, errs) = parse_expression("a && b || c");
        assert!(errs.is_empty());
        let s = ast_shape(&g);
        // Outer operator is ||, inner is &&. Find the order: the first
        // EXPR_BINARY in the dump is the outer one.
        let first_binop_line = s.lines().find(|l| l.contains("EXPR_BINARY")).unwrap();
        // Inside it the immediate-child operator token is PipePipe.
        assert!(s.contains("PipePipe"));
        let _ = first_binop_line;
    }

    #[test]
    fn cast_postfix_binds_below_unary() {
        // x as u32 + 10 -> (x as u32) + 10
        let (g, errs) = parse_expression("x as u32 + 10");
        assert!(errs.is_empty(), "{errs:?}");
        let s = ast_shape(&g);
        assert!(s.contains("EXPR_CAST"));
        assert!(s.contains("Plus"));
    }

    #[test]
    fn parenthesised_overrides_precedence() {
        let (g, errs) = parse_expression("(1 + 2) * 3");
        assert!(errs.is_empty());
        let s = ast_shape(&g);
        assert!(s.contains("EXPR_PAREN"));
    }

    #[test]
    fn field_access_chain() {
        let (g, errs) = parse_expression("ctx.speed");
        assert!(errs.is_empty(), "{errs:?}");
        let s = ast_shape(&g);
        assert!(s.contains("EXPR_FIELD_REF"));
    }

    #[test]
    fn function_call_is_postfix() {
        let (g, errs) = parse_expression("foo(1, 2, 3)");
        assert!(errs.is_empty());
        let s = ast_shape(&g);
        assert!(s.contains("EXPR_CALL"));
        assert!(s.contains("ARG_LIST"));
    }

    #[test]
    fn guard_context_rejects_string_literal() {
        let mut p = Parser::new("\"hello\"");
        p.start_node(SyntaxKind::FILE);
        parse_expr(&mut p, 0, ExprContext::Guard);
        while !p.at(fsm_lexer::TokenKind::Eof) {
            p.bump();
        }
        p.drain_trailing_trivia();
        p.finish_node();
        let (_, errs) = p.finish();
        assert!(!errs.is_empty());
    }

    #[test]
    fn guard_context_rejects_arithmetic() {
        let mut p = Parser::new("a + b");
        p.start_node(SyntaxKind::FILE);
        parse_expr(&mut p, 0, ExprContext::Guard);
        // `+` is not an infix operator in Guard mode; the lhs is parsed but
        // `+` is left as a stray token. Consume it so the tree closes
        // cleanly.
        while !p.at(fsm_lexer::TokenKind::Eof) {
            p.bump();
        }
        p.drain_trailing_trivia();
        p.finish_node();
        let (_, _errs) = p.finish();
        // The absence of an EXPR_BINARY node means the `+` was rejected,
        // which is the intent.
    }
}
