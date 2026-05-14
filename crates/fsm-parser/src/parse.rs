//! Public entry point — `parse(src)` and `parse_with_tokens(tokens, src)`.
//!
//! Both return a [`ParseResult`] holding the green tree and the
//! accumulated diagnostics. Callers typically project to either the CST
//! ([`ParseResult::syntax`]) or the typed AST root ([`ParseResult::ast`]).
//!
//! Per Doc 00 §7.12 G-02 / audit §P1-5, the entry points enforce DoS
//! limits ([`ParseLimits`]) on input size and token count before driving
//! the grammar. Adversarial inputs short-circuit to a single
//! `FSM-E0010` diagnostic + empty file CST, never panic.

use fsm_diagnostics::{Diagnostic, DiagnosticCode, Span};
use fsm_lexer::Token;
use rowan::GreenNode;

use crate::ast;
use crate::cst::{SyntaxKind, SyntaxNode};
use crate::grammar;
use crate::limits::ParseLimits;
use crate::parser::Parser;

/// Output of a parse call. The CST is always present; downstream code
/// chooses whether to drop into the typed AST view.
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// Rowan green tree — preserves trivia and round-trips the source.
    pub green: GreenNode,
    /// Diagnostics emitted while parsing. May be non-empty even on success
    /// (warnings, lexer errors that were recovered from, etc.).
    pub errors: Vec<Diagnostic>,
}

impl ParseResult {
    /// Build a strongly typed [`SyntaxNode`] view of the green tree.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// Build the AST root view. Always succeeds because the root node is
    /// always a `FILE`.
    pub fn ast(&self) -> ast::File {
        use crate::ast::AstNode;
        ast::File::cast(self.syntax()).expect("root node is always FILE")
    }

    /// Reconstruct the source byte-for-byte from the green tree. Used in
    /// tests to confirm the CST round-trip property.
    pub fn reconstructed_text(&self) -> String {
        self.syntax().text().to_string()
    }
}

/// Parse `src` into a CST + diagnostics, using [`ParseLimits::DEFAULT`].
///
/// Adversarial inputs (oversize, or producing too many tokens) short-
/// circuit to a single `FSM-E0010` diagnostic plus an **empty** `FILE`
/// CST. The function never panics and never blocks on attacker-controlled
/// nesting.
pub fn parse(src: &str) -> ParseResult {
    parse_with_limits(src, &ParseLimits::DEFAULT)
}

/// Parse with caller-supplied limits. Used by build drivers that need to
/// raise (or, more commonly, tighten) the defaults — e.g., a Web Playground
/// that wants to reject anything over 64 KiB regardless of the parser's
/// internal default.
pub fn parse_with_limits(src: &str, limits: &ParseLimits) -> ParseResult {
    // Input-size cap — checked before we hand the source to the lexer so
    // we never allocate token vectors for a multi-megabyte adversarial
    // input.
    if src.len() > limits.max_input_bytes {
        return oversize_input_result(src.len(), limits.max_input_bytes);
    }

    let tokens = fsm_lexer::tokenize(src);

    // Token-count cap — defends against pathological tokenizations and
    // future lexer changes that might emit more tokens than expected
    // from a within-spec input.
    if tokens.len() > limits.max_token_count {
        return excess_tokens_result(tokens.len(), limits.max_token_count);
    }

    let mut p = Parser::from_tokens_with_limits(src, tokens, *limits);
    grammar::parse_file(&mut p);
    let (green, errors) = p.finish();
    ParseResult { green, errors }
}

/// Same as [`parse`] but takes a pre-built token vector. Used by the LSP
/// incremental-reparse pipeline (Doc 20 §4.5) so it doesn't re-lex
/// untouched ranges. Applies [`ParseLimits::DEFAULT`] to both the source
/// length and the token count.
pub fn parse_with_tokens(src: &str, tokens: Vec<Token>) -> ParseResult {
    let limits = ParseLimits::DEFAULT;
    if src.len() > limits.max_input_bytes {
        return oversize_input_result(src.len(), limits.max_input_bytes);
    }
    if tokens.len() > limits.max_token_count {
        return excess_tokens_result(tokens.len(), limits.max_token_count);
    }
    let mut p = Parser::from_tokens_with_limits(src, tokens, limits);
    grammar::parse_file(&mut p);
    let (green, errors) = p.finish();
    ParseResult { green, errors }
}

/// Build the canonical "input too big" parse result. A single E0010
/// diagnostic anchored at byte 0, plus an empty `FILE` green tree so
/// callers projecting to `ast()` still see a well-formed root.
fn oversize_input_result(actual: usize, cap: usize) -> ParseResult {
    let mut b = rowan::GreenNodeBuilder::new();
    b.start_node(SyntaxKind::FILE.into_raw());
    b.finish_node();
    let green = b.finish();
    let errors = vec![
        Diagnostic::new(DiagnosticCode::E0010, Span::new(0, 0)).with_message(format!(
            "input exceeds maximum size ({actual} bytes; limit {cap})"
        )),
    ];
    ParseResult { green, errors }
}

/// Build the canonical "too many tokens" parse result. Same shape as
/// [`oversize_input_result`] — empty FILE + single E0010.
fn excess_tokens_result(actual: usize, cap: usize) -> ParseResult {
    let mut b = rowan::GreenNodeBuilder::new();
    b.start_node(SyntaxKind::FILE.into_raw());
    b.finish_node();
    let green = b.finish();
    let errors = vec![
        Diagnostic::new(DiagnosticCode::E0010, Span::new(0, 0)).with_message(format!(
            "input produces too many tokens ({actual}; limit {cap})"
        )),
    ];
    ParseResult { green, errors }
}
