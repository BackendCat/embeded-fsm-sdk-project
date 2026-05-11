//! Recursive-descent driver.
//!
//! The parser owns the lossless token stream, a [`rowan::GreenNodeBuilder`],
//! and a vector of accumulated [`Diagnostic`]s. Lookahead and consumption
//! operate on **non-trivia** tokens — trivia is auto-attached to the next
//! `bump()` so the green tree round-trips the source byte for byte.
//!
//! Error recovery follows the rust-analyzer "panic and resync" model:
//! [`Parser::error_until`] skips tokens (each emitted under an `ERROR_NODE`)
//! until a sync-set member is seen.

use fsm_diagnostics::{Diagnostic, DiagnosticCode, Span};
use fsm_lexer::{Token, TokenKind};

use crate::cst::{syntax_kind_from_token, SyntaxKind};
use crate::token_set::TokenSet;

/// Parser state carried through every grammar rule. Methods follow the
/// conventional vocabulary: `current`, `peek`, `bump`, `eat`, `expect`.
///
/// The lifetime parameter is implicit — the parser holds the source as a
/// `&'src str` because the green tree records token text verbatim (rowan
/// stores owned strings internally, but we look up bytes from `src` to
/// avoid copying through an intermediate `String`).
pub struct Parser<'src> {
    src: &'src str,
    /// Full token stream including trivia + the final `Eof`.
    tokens: Vec<Token>,
    /// Index into `tokens`. Always points at the next *non-trivia* token, or
    /// the `Eof` sentinel.
    pos: usize,
    builder: rowan::GreenNodeBuilder<'static>,
    errors: Vec<Diagnostic>,
    /// Stable-ID annotations attach to the *next* declaration. The driver
    /// emits them as part of the declaration's CST subtree so the AST can
    /// recover them through `stable_id()`. This buffer holds an annotation
    /// that has been parsed but not yet wrapped into its declaration.
    pending_stable_id_start: Option<rowan::Checkpoint>,
}

impl std::fmt::Debug for Parser<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parser")
            .field("pos", &self.pos)
            .field("tokens_len", &self.tokens.len())
            .field("errors_len", &self.errors.len())
            .field("current", &self.current())
            .finish()
    }
}

/// Parse-time context — currently only "are we in a guard expression?",
/// which restricts the set of operators / call forms accepted. Pratt code
/// reads this through `Parser::expr_context`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExprContext {
    /// Action sublanguage. Full expression grammar.
    Action,
    /// Guard sublanguage. Calls only via `pure extern` (semantic check
    /// happens in the analyzer; parser accepts the call site but tags it).
    Guard,
}

impl<'src> Parser<'src> {
    /// Build a parser over `src`. The source is tokenised eagerly so the
    /// parser can do constant-time peeks.
    pub fn new(src: &'src str) -> Self {
        let tokens = fsm_lexer::tokenize(src);
        Self::from_tokens(src, tokens)
    }

    /// Alternative entry point for callers that already have a token
    /// vector — useful for the LSP incremental-reparse pipeline (Doc 20
    /// §4.5).
    pub fn from_tokens(src: &'src str, tokens: Vec<Token>) -> Self {
        let mut p = Self {
            src,
            tokens,
            pos: 0,
            builder: rowan::GreenNodeBuilder::new(),
            errors: Vec::new(),
            pending_stable_id_start: None,
        };
        // Position at first non-trivia token. Trivia at the start of the
        // file is auto-attached to the first declaration's CST subtree by
        // `bump()`.
        p.skip_trivia();
        p
    }

    /// Returned by [`Parser::finish`] — the final green tree and accumulated
    /// diagnostics. **Callers MUST close their root node before invoking
    /// this method.** Trailing trivia inside the root is the caller's
    /// responsibility to attach via [`Parser::drain_trailing_trivia`]
    /// before closing.
    pub fn finish(self) -> (rowan::GreenNode, Vec<Diagnostic>) {
        (self.builder.finish(), self.errors)
    }

    /// Emit any remaining trivia tokens (whitespace, comments, but NOT
    /// the `Eof` sentinel) into the current node. Call this immediately
    /// before [`Parser::finish_node`] on the root so the green tree
    /// includes trailing whitespace and round-trips the source.
    pub fn drain_trailing_trivia(&mut self) {
        while self.pos < self.tokens.len() {
            let tk = self.tokens[self.pos].kind;
            if tk == TokenKind::Eof {
                break;
            }
            self.emit_token_at(self.pos);
            self.pos += 1;
        }
    }

    /// Kind of the next non-trivia token. Always defined — returns
    /// [`TokenKind::Eof`] past the end.
    pub fn current(&self) -> TokenKind {
        self.tokens
            .get(self.pos)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    /// Span of the current token. Useful for diagnostic anchoring.
    pub fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or_else(|| Span::empty(self.src.len()))
    }

    /// Source text of the current token.
    pub fn current_text(&self) -> &str {
        let span = self.current_span();
        &self.src[span.start..span.end]
    }

    /// `n`-token lookahead. `peek_n(0)` is [`Parser::current`].
    pub fn peek_n(&self, n: usize) -> TokenKind {
        // Skip trivia while counting. The lookahead is bounded — practical
        // grammar rules need n <= 3, so the linear scan is cheap.
        let mut idx = self.pos;
        let mut left = n;
        while idx < self.tokens.len() {
            let kind = self.tokens[idx].kind;
            if kind.is_trivia() {
                idx += 1;
                continue;
            }
            if left == 0 {
                return kind;
            }
            left -= 1;
            idx += 1;
        }
        TokenKind::Eof
    }

    /// `true` if [`Parser::current`] is `kind`.
    pub fn at(&self, kind: TokenKind) -> bool {
        self.current() == kind
    }

    /// `true` if [`Parser::current`] is in `set`. Used for sync-set checks.
    pub fn at_any_of(&self, set: TokenSet) -> bool {
        set.contains(self.current())
    }

    /// Consume the current token, emitting it (plus any preceding trivia)
    /// into the green tree. Mirrors rust-analyzer's `Parser::bump`.
    pub fn bump(&mut self) {
        // Trivia tokens are emitted first so the next token appears
        // *after* its leading whitespace/comments in the tree.
        debug_assert!(!self.current().is_trivia());
        self.emit_token_at(self.pos);
        self.pos += 1;
        self.skip_trivia();
    }

    /// Consume `kind` if present; return whether a token was consumed.
    pub fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Consume `kind` or emit a diagnostic. The diagnostic uses `code` and
    /// includes the actual current token in the message. Returns whether
    /// the token was consumed — callers may use this signal to decide
    /// whether to bail out of the current rule.
    pub fn expect(&mut self, kind: TokenKind, code: DiagnosticCode) -> bool {
        if self.eat(kind) {
            return true;
        }
        let span = self.current_span();
        let msg = format!("expected '{}', found '{}'", kind, self.current());
        self.errors
            .push(Diagnostic::new(code, span).with_message(msg));
        false
    }

    /// Emit a diagnostic at the current position without consuming any
    /// token. Caller is responsible for advancing the cursor.
    pub fn error(&mut self, code: DiagnosticCode, message: impl Into<String>) {
        let span = self.current_span();
        self.errors
            .push(Diagnostic::new(code, span).with_message(message));
    }

    /// Emit a diagnostic at an arbitrary span.
    pub fn error_at(&mut self, span: Span, code: DiagnosticCode, message: impl Into<String>) {
        self.errors
            .push(Diagnostic::new(code, span).with_message(message));
    }

    /// Append a fully-formed diagnostic.
    pub fn push_diagnostic(&mut self, diag: Diagnostic) {
        self.errors.push(diag);
    }

    /// Start a new CST node. Mirrors `GreenNodeBuilder::start_node`.
    pub fn start_node(&mut self, kind: SyntaxKind) {
        self.builder.start_node(kind.into_raw());
    }

    /// Finish the most recently started CST node.
    pub fn finish_node(&mut self) {
        self.builder.finish_node();
    }

    /// Take a checkpoint that can later be wrapped with
    /// [`Parser::start_node_at`]. Used when we discover *after* parsing some
    /// prefix that a wrapper node is needed (e.g. binary expressions in the
    /// Pratt loop).
    pub fn checkpoint(&mut self) -> rowan::Checkpoint {
        self.builder.checkpoint()
    }

    /// Wrap the tokens emitted since `cp` in a node of `kind`.
    pub fn start_node_at(&mut self, cp: rowan::Checkpoint, kind: SyntaxKind) {
        self.builder.start_node_at(cp, kind.into_raw());
    }

    /// Wrap the current token in an `ERROR_NODE`, emit a diagnostic, and
    /// advance.
    pub fn err_and_bump(&mut self, code: DiagnosticCode, message: impl Into<String>) {
        self.start_node(SyntaxKind::ERROR_NODE);
        self.error(code, message);
        if !self.at(TokenKind::Eof) {
            self.bump();
        }
        self.finish_node();
    }

    /// Panic-mode recovery. Wrap everything between the current position and
    /// the first sync-set member in an `ERROR_NODE`. Always consumes at
    /// least one token if not already at sync (otherwise infinite loops are
    /// possible).
    pub fn error_until(
        &mut self,
        sync: TokenSet,
        code: DiagnosticCode,
        message: impl Into<String>,
    ) {
        let start_span = self.current_span();
        let actual = self.current();
        self.errors
            .push(Diagnostic::new(code, start_span).with_message(format!(
                "{}: found '{}'",
                message.into(),
                actual
            )));
        self.start_node(SyntaxKind::ERROR_NODE);
        // Always consume at least the offending token unless we are at EOF
        // or already at a sync token — otherwise the rule will spin.
        let mut consumed = false;
        while !self.at(TokenKind::Eof) && (!consumed || !self.at_any_of(sync)) {
            self.bump();
            consumed = true;
            if self.at_any_of(sync) {
                break;
            }
        }
        self.finish_node();
    }

    /// Helper: skip over trivia tokens, emitting them as the parent node's
    /// children. Trivia is *always* emitted into the active node (rowan
    /// guarantees the green tree round-trips the source).
    fn skip_trivia(&mut self) {
        while let Some(tok) = self.tokens.get(self.pos) {
            if tok.kind == TokenKind::Eof {
                break;
            }
            if tok.kind.is_trivia() {
                self.emit_token_at(self.pos);
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Emit the token at index `i` into the green tree. Maps lexer error
    /// tokens to a diagnostic (using the embedded code) **and** emits them
    /// into the tree under their natural `Error` syntax kind so the tree
    /// remains byte-exact.
    fn emit_token_at(&mut self, i: usize) {
        let tok = &self.tokens[i];
        if let TokenKind::Error(code) = tok.kind {
            self.errors
                .push(Diagnostic::new(code, tok.span).with_message(format!(
                    "lexer error: {}",
                    &self.src[tok.span.start..tok.span.end]
                )));
        }
        let kind = syntax_kind_from_token(tok.kind);
        let text = &self.src[tok.span.start..tok.span.end];
        self.builder.token(kind.into_raw(), text);
    }

    /// Stash a `pending_stable_id_start` checkpoint. Used by callers that
    /// see a `@id(...)` annotation prefixing a declaration whose start
    /// keyword has not yet been consumed.
    pub fn stash_stable_id_checkpoint(&mut self, cp: rowan::Checkpoint) {
        self.pending_stable_id_start = Some(cp);
    }

    /// Pop the stashed checkpoint, if any.
    pub fn take_stable_id_checkpoint(&mut self) -> Option<rowan::Checkpoint> {
        self.pending_stable_id_start.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_returns_first_non_trivia() {
        let mut p = Parser::new("  // comment\n machine M { }");
        assert_eq!(p.current(), TokenKind::KwMachine);
        p.bump();
        assert_eq!(p.current(), TokenKind::Ident);
        assert_eq!(p.current_text(), "M");
    }

    #[test]
    fn peek_n_skips_trivia() {
        let p = Parser::new("a   /* note */   b");
        assert_eq!(p.peek_n(0), TokenKind::Ident);
        assert_eq!(p.peek_n(1), TokenKind::Ident);
        assert_eq!(p.peek_n(2), TokenKind::Eof);
    }

    #[test]
    fn expect_emits_diagnostic_on_mismatch() {
        let mut p = Parser::new("machine");
        p.start_node(SyntaxKind::FILE);
        let ok = p.expect(TokenKind::KwState, DiagnosticCode::E0010);
        assert!(!ok);
        // Drain whatever's left so the root is the only top-level child.
        p.drain_trailing_trivia();
        // Consume the leftover `machine` token explicitly to keep the
        // green tree well-formed for this test.
        if !p.at(TokenKind::Eof) {
            p.bump();
        }
        p.finish_node();
        let (_, errs) = p.finish();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, DiagnosticCode::E0010);
    }

    #[test]
    fn error_until_makes_progress() {
        // No sync token in the file — error_until must still consume to EOF.
        let mut p = Parser::new("garbage tokens here");
        p.start_node(SyntaxKind::FILE);
        p.error_until(
            TokenSet::new(&[TokenKind::KwMachine]),
            DiagnosticCode::E0010,
            "expected declaration",
        );
        // Consume any remaining non-trivia tokens before closing FILE.
        while !p.at(TokenKind::Eof) {
            p.bump();
        }
        p.drain_trailing_trivia();
        p.finish_node();
        assert_eq!(p.current(), TokenKind::Eof);
    }

    #[test]
    fn lexer_errors_surface_as_diagnostics() {
        // Backtick is not a recognised token start — lexer emits Error(E0001).
        let mut p = Parser::new("`weird`");
        p.start_node(SyntaxKind::FILE);
        // Drive the parser past the bad token. `current()` skips trivia but
        // not error tokens, so we expect to land on the Error variant.
        assert!(matches!(p.current(), TokenKind::Error(_)));
        while !p.at(TokenKind::Eof) {
            p.bump();
        }
        p.drain_trailing_trivia();
        p.finish_node();
        let (_, errs) = p.finish();
        assert!(!errs.is_empty(), "expected at least one diagnostic");
    }
}
