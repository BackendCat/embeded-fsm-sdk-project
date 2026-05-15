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
use crate::limits::ParseLimits;
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
    /// DoS hardening (P1-5, Doc 00 §7.12 G-02). Per-call limits + the
    /// running depth counter. `current_depth` is incremented through
    /// [`DepthGuard`] in recursive grammar rules so it always reflects the
    /// nesting depth of the active rule even on error-return paths.
    limits: ParseLimits,
    /// Number of recursive grammar frames currently on the stack. Compared
    /// against `limits.max_recursion_depth` by [`DepthGuard::enter`].
    current_depth: u32,
    /// Sticky flag: once the depth cap has been hit at least once during
    /// this parse call, recursive rules short-circuit instead of repeating
    /// the same diagnostic on every descent. Caller-facing diagnostics
    /// remain "one per limit-hit nest", not "one per descended frame".
    depth_limit_reported: bool,
    /// Exclusive end index of file-leading trivia. The constructor advances
    /// `pos` past any whitespace/comment tokens that precede the first real
    /// token so lookahead works immediately, but it must NOT emit them into
    /// the green builder yet — no node is open at construction time, and
    /// rowan asserts a single root (a token emitted before the first
    /// `start_node` becomes a stray root-level child and trips
    /// `builder.rs:113 left == right`, the PARSE-BUG-1 panic). The grammar
    /// driver flushes `tokens[0..leading_trivia_end]` via
    /// [`Parser::flush_leading_trivia`] *after* opening the root `FILE`
    /// node, so leading banner/license comments land inside the file node
    /// and the CST stays byte-exact lossless.
    leading_trivia_end: usize,
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
    /// parser can do constant-time peeks. Uses [`ParseLimits::DEFAULT`].
    pub fn new(src: &'src str) -> Self {
        let tokens = fsm_lexer::tokenize(src);
        Self::from_tokens_with_limits(src, tokens, ParseLimits::DEFAULT)
    }

    /// Like [`Parser::new`] but with caller-supplied limits. Used by
    /// [`crate::parse_with_limits`] and by test code that needs to drive
    /// the depth/byte caps at unusual values.
    pub fn with_limits(src: &'src str, limits: ParseLimits) -> Self {
        let tokens = fsm_lexer::tokenize(src);
        Self::from_tokens_with_limits(src, tokens, limits)
    }

    /// Alternative entry point for callers that already have a token
    /// vector — useful for the LSP incremental-reparse pipeline (Doc 20
    /// §4.5). Uses [`ParseLimits::DEFAULT`].
    pub fn from_tokens(src: &'src str, tokens: Vec<Token>) -> Self {
        Self::from_tokens_with_limits(src, tokens, ParseLimits::DEFAULT)
    }

    /// Most-general constructor — caller supplies both the token vector
    /// and the limit configuration. The other constructors are thin
    /// adapters.
    pub fn from_tokens_with_limits(
        src: &'src str,
        tokens: Vec<Token>,
        limits: ParseLimits,
    ) -> Self {
        let mut p = Self {
            src,
            tokens,
            pos: 0,
            builder: rowan::GreenNodeBuilder::new(),
            errors: Vec::new(),
            pending_stable_id_start: None,
            limits,
            current_depth: 0,
            depth_limit_reported: false,
            leading_trivia_end: 0,
        };
        // Advance `pos` past file-leading trivia so `current()`/`peek_n()`
        // see the first real token, but DON'T emit those tokens yet — no
        // green node is open at construction time. The grammar driver
        // opens the root `FILE` node and then calls
        // `flush_leading_trivia()` to attach `tokens[0..leading_trivia_end]`
        // inside it (PARSE-BUG-1: emitting before the first `start_node`
        // leaves stray root children and trips rowan's single-root
        // assertion at builder.rs:113).
        p.skip_leading_trivia_no_emit();
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

    /// Constructor-only: advance `pos` past file-leading trivia **without**
    /// emitting into the green builder, recording the boundary in
    /// [`Self::leading_trivia_end`]. Used because no node is open at
    /// construction time (see the field doc / PARSE-BUG-1). Lookahead is
    /// correct immediately after; the deferred tokens are flushed by
    /// [`Self::flush_leading_trivia`] once the root node is open.
    fn skip_leading_trivia_no_emit(&mut self) {
        while let Some(tok) = self.tokens.get(self.pos) {
            if tok.kind == TokenKind::Eof || !tok.kind.is_trivia() {
                break;
            }
            self.pos += 1;
        }
        self.leading_trivia_end = self.pos;
    }

    /// Emit the file-leading trivia tokens (recorded by
    /// [`Self::skip_leading_trivia_no_emit`]) into the currently-open node.
    /// MUST be called by the grammar driver immediately after opening the
    /// root `FILE` node and before any other token, so leading
    /// banner/license comments + whitespace are captured inside the file
    /// node and the CST round-trips the source byte-for-byte. Idempotent:
    /// a second call is a no-op (the range is emitted exactly once).
    pub fn flush_leading_trivia(&mut self) {
        for i in 0..self.leading_trivia_end {
            self.emit_token_at(i);
        }
        // Mark as flushed so a defensive double-call can't duplicate the
        // tokens (would break the byte-exact round-trip).
        self.leading_trivia_end = 0;
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

    // ─── Recursion depth tracking (P1-5, Doc 00 §7.12 G-02) ─────────────
    //
    // Every recursive grammar rule should call [`Parser::enter_recursion`]
    // at the top, check the returned `DepthGuard` for the limit-exceeded
    // signal, and let the guard's `Drop` decrement the counter on exit.
    // The pattern is:
    //
    // ```text
    // pub fn parse_expr(p: &mut Parser, ...) {
    //     let _guard = match p.enter_recursion() {
    //         Some(g) => g,
    //         None => return, // diagnostic already emitted
    //     };
    //     // ... do the work ...
    // }
    // ```

    /// Snapshot of the configured limits. Cheap (Copy).
    pub fn limits(&self) -> ParseLimits {
        self.limits
    }

    /// Current recursion depth. Visible for tests / diagnostics; rules
    /// should use [`Parser::enter_recursion`] rather than touching this
    /// directly.
    pub fn current_depth(&self) -> u32 {
        self.current_depth
    }

    /// Run `body` inside a recursive grammar frame, transparently tracking
    /// the depth counter. Returns the `body`'s value on success, or
    /// `default` if the call would exceed `limits.max_recursion_depth`
    /// (in which case `body` is **not** invoked and a single diagnostic
    /// is appended on the first hit).
    ///
    /// The counter is decremented on every exit path — early `return`,
    /// `?` propagation, or panic — because the [`DepthGuard`] holds the
    /// `&mut Parser` via [`std::marker::PhantomData`] and decrements in
    /// its `Drop` impl. The closure receives the parser back through
    /// the guard's `parser` accessor so it can call any parser method
    /// without re-borrowing tricks.
    pub fn with_recursion<R>(
        &mut self,
        default: R,
        body: impl FnOnce(&mut Parser<'src>) -> R,
    ) -> R {
        if self.current_depth >= self.limits.max_recursion_depth {
            if !self.depth_limit_reported {
                self.depth_limit_reported = true;
                let span = self.current_span();
                let msg = format!(
                    "input exceeds maximum recursion depth ({})",
                    self.limits.max_recursion_depth
                );
                self.errors
                    .push(Diagnostic::new(DiagnosticCode::E0010, span).with_message(msg));
            }
            return default;
        }
        let mut guard = DepthGuard::enter(self);
        body(guard.parser())
    }
}

/// RAII helper that increments [`Parser::current_depth`] on construction
/// and decrements on drop. Constructed exclusively through
/// [`Parser::with_recursion`]; the type is public only so the `Drop` impl
/// is documented for readers tracing the depth-budget machinery.
#[derive(Debug)]
pub struct DepthGuard<'p, 'src> {
    parser: &'p mut Parser<'src>,
}

impl<'p, 'src> DepthGuard<'p, 'src> {
    /// Increment the parser's depth counter and produce a guard. The
    /// caller MUST have verified the depth budget before calling — the
    /// public entry point is [`Parser::with_recursion`].
    fn enter(parser: &'p mut Parser<'src>) -> Self {
        parser.current_depth += 1;
        Self { parser }
    }

    /// Borrow the parser back out for use inside the recursive rule.
    pub fn parser(&mut self) -> &mut Parser<'src> {
        self.parser
    }
}

impl<'p, 'src> Drop for DepthGuard<'p, 'src> {
    fn drop(&mut self) {
        // Saturating: enter() always increments before constructing the
        // guard, so the counter is > 0 here, but the saturating sub keeps
        // future refactors safe.
        self.parser.current_depth = self.parser.current_depth.saturating_sub(1);
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
