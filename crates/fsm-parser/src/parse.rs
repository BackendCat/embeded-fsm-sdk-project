//! Public entry point — `parse(src)` and `parse_with_tokens(tokens, src)`.
//!
//! Both return a [`ParseResult`] holding the green tree and the
//! accumulated diagnostics. Callers typically project to either the CST
//! ([`ParseResult::syntax`]) or the typed AST root ([`ParseResult::ast`]).

use fsm_diagnostics::Diagnostic;
use fsm_lexer::Token;
use rowan::GreenNode;

use crate::ast;
use crate::cst::SyntaxNode;
use crate::grammar;
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

/// Parse `src` into a CST + diagnostics.
pub fn parse(src: &str) -> ParseResult {
    let mut p = Parser::new(src);
    grammar::parse_file(&mut p);
    let (green, errors) = p.finish();
    ParseResult { green, errors }
}

/// Same as [`parse`] but takes a pre-built token vector. Used by the LSP
/// incremental-reparse pipeline (Doc 20 §4.5) so it doesn't re-lex
/// untouched ranges.
pub fn parse_with_tokens(src: &str, tokens: Vec<Token>) -> ParseResult {
    let mut p = Parser::from_tokens(src, tokens);
    grammar::parse_file(&mut p);
    let (green, errors) = p.finish();
    ParseResult { green, errors }
}
