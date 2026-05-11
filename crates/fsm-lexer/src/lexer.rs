//! The lexer state machine.
//!
//! ## Contract
//!
//! - Single-pass over a borrowed `&str`. No allocation per token.
//! - Always lex to end of file: errors emit a `TokenKind::Error(code)` token
//!   covering the offending byte range and lexing continues. The lexer never
//!   panics on malformed input.
//! - Trivia (whitespace, newlines, comments) is preserved so the CST /
//!   formatter can round-trip the source verbatim (Doc 20 §3, Doc 19).
//!
//! ## UTF-8 error recovery
//!
//! Per `docs/00-Decisions-And-Reconciliation.md` §B fix to VALIDATION_REPORT
//! 2.1: when an unrecognised character is encountered, advance `pos` by
//! `ch.len_utf8()` — NOT by one byte. Advancing by one byte mid-codepoint
//! would split a multi-byte UTF-8 character and corrupt the stream from that
//! point forward. A regression test (`utf8_multibyte_does_not_corrupt_stream`)
//! locks this in.
//!
//! ## Underscore rules in numeric literals
//!
//! Doc 04 §1.3 is silent on edge cases; the resolution from Doc 00 is:
//!
//! - Underscores between digits are always allowed (`1_000_000`, `0xFF_AB`).
//! - Trailing underscore is allowed (`123_` parses as the value `123`).
//! - Double underscores are allowed (`1__000` parses as `1000`).
//! - Leading underscore immediately after a base prefix is REJECTED with
//!   `FSM-E0004` (`0x_FF`, `0b_10` are invalid).
//! - Leading underscore on a bare decimal literal cannot happen: `_123` lexes
//!   as an identifier.

use core::fmt;

use fsm_diagnostics::{DiagnosticCode, Span};

use crate::token::{keyword_kind, Token, TokenKind};

/// Byte-Order Mark (UTF-8 form). Stripped silently from the head of the
/// source per Doc 04 §1.1.
const BOM: &str = "\u{FEFF}";

/// Streaming tokenizer over a `&str` source.
///
/// `pos` is a byte index into `src`. The lexer is a simple cursor — no
/// internal lookahead beyond peeking the next 1–2 bytes. `next_token`
/// advances the cursor; `peek_token` does not.
///
/// The lexer is intentionally not `Iterator` — callers usually want the
/// explicit `Eof` sentinel so they can distinguish "stream ended" from
/// "iterator yielded `None`".
#[derive(Clone)]
pub struct Lexer<'src> {
    src: &'src str,
    pos: usize,
}

impl fmt::Debug for Lexer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lexer")
            .field("pos", &self.pos)
            .field("remaining_bytes", &(self.src.len() - self.pos))
            .finish()
    }
}

impl<'src> Lexer<'src> {
    /// Construct a lexer over `src`. A leading UTF-8 BOM is silently consumed
    /// per Doc 04 §1.1.
    pub fn new(src: &'src str) -> Self {
        let pos = if src.starts_with(BOM) { BOM.len() } else { 0 };
        Self { src, pos }
    }

    /// Yield the next token. Once the source is exhausted, returns `Eof`
    /// repeatedly with a zero-width span anchored at `src.len()`.
    pub fn next_token(&mut self) -> Token {
        if self.pos >= self.src.len() {
            return Token::new(TokenKind::Eof, Span::empty(self.src.len()));
        }
        let start = self.pos;
        let ch = self.peek_char().expect("position checked above");

        // Order of these branches mirrors the spec section flow:
        //   §1.1 BOM (handled in constructor; mid-stream BOMs are treated as
        //        unknown chars and yield an Error token).
        //   trivia → comments → identifiers/keywords → numbers → strings
        //   → operators → stable IDs (`@`) → unknown char fallback.
        match ch {
            ' ' | '\t' => self.lex_whitespace(start),
            '\n' | '\r' => self.lex_newline(start),
            '/' => self.lex_slash_or_comment(start),
            c if is_ident_start(c) => self.lex_ident_or_keyword(start),
            c if c.is_ascii_digit() => self.lex_number(start),
            '"' => self.lex_string(start),
            '@' => self.lex_at_or_stable_id(start),
            '-' => self.lex_minus_or_arrow(start),
            '~' => self.lex_tilde_or_history_arrow(start),
            '=' => self.lex_eq_or_eqeq(start),
            '!' => self.lex_bang_or_bangeq(start),
            '<' => self.lex_lt_le_or_shl(start),
            '>' => self.lex_gt_ge_or_shr(start),
            '&' => self.lex_amp_or_ampamp(start),
            '|' => self.lex_pipe_or_pipepipe(start),
            ':' => self.advance_single(start, TokenKind::Colon),
            ';' => self.advance_single(start, TokenKind::Semicolon),
            ',' => self.advance_single(start, TokenKind::Comma),
            '.' => self.advance_single(start, TokenKind::Dot),
            '{' => self.advance_single(start, TokenKind::LBrace),
            '}' => self.advance_single(start, TokenKind::RBrace),
            '(' => self.advance_single(start, TokenKind::LParen),
            ')' => self.advance_single(start, TokenKind::RParen),
            '[' => self.advance_single(start, TokenKind::LBracket),
            ']' => self.advance_single(start, TokenKind::RBracket),
            '+' => self.advance_single(start, TokenKind::Plus),
            '*' => self.advance_single(start, TokenKind::Star),
            '%' => self.advance_single(start, TokenKind::Percent),
            '^' => self.advance_single(start, TokenKind::Caret),
            _ => self.lex_unknown(start, ch),
        }
    }

    /// Look at the next token without consuming it. Implemented by cloning the
    /// cursor — cheap because [`Lexer`] is a 16-byte `Copy`-style struct.
    pub fn peek_token(&mut self) -> Token {
        let saved = self.pos;
        let tok = self.next_token();
        self.pos = saved;
        tok
    }

    // ─── Cursor helpers ──────────────────────────────────────────────────

    fn peek_char(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_byte_at(&self, offset: usize) -> Option<u8> {
        self.src.as_bytes().get(self.pos + offset).copied()
    }

    fn advance_single(&mut self, start: usize, kind: TokenKind) -> Token {
        // Single-byte ASCII punctuation — safe to bump by 1.
        self.pos += 1;
        Token::new(kind, Span::new(start, self.pos))
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.pos)
    }

    // ─── Trivia ──────────────────────────────────────────────────────────

    fn lex_whitespace(&mut self, start: usize) -> Token {
        // Consume runs of plain space + tab. Newlines get their own token so
        // the formatter can preserve blank-line structure.
        while let Some(b) = self.peek_byte_at(0) {
            if b == b' ' || b == b'\t' {
                self.pos += 1;
            } else {
                break;
            }
        }
        Token::new(TokenKind::Whitespace, self.span_from(start))
    }

    fn lex_newline(&mut self, start: usize) -> Token {
        // Both LF and CRLF as one Newline token; lone CR also accepted.
        match self.peek_byte_at(0) {
            Some(b'\r') => {
                self.pos += 1;
                if self.peek_byte_at(0) == Some(b'\n') {
                    self.pos += 1;
                }
            }
            Some(b'\n') => self.pos += 1,
            _ => unreachable!("lex_newline only called on \\r or \\n"),
        }
        Token::new(TokenKind::Newline, self.span_from(start))
    }

    fn lex_slash_or_comment(&mut self, start: usize) -> Token {
        match self.peek_byte_at(1) {
            Some(b'/') => self.lex_line_or_doc_comment(start),
            Some(b'*') => self.lex_block_comment(start),
            _ => self.advance_single(start, TokenKind::Slash),
        }
    }

    fn lex_line_or_doc_comment(&mut self, start: usize) -> Token {
        // We already know src[pos..pos+2] == "//".
        self.pos += 2;
        let kind = if self.peek_byte_at(0) == Some(b'/') {
            // `///` doc comment. Per Doc 04 §1.4 it attaches to the next
            // declaration; the parser keeps it as trivia in the CST.
            self.pos += 1;
            TokenKind::DocComment
        } else {
            TokenKind::LineComment
        };
        // Consume to end of line. The terminating newline itself is NOT part
        // of the comment token — it becomes the next Newline token. This
        // keeps line-number reconstruction simple for the formatter.
        while let Some(b) = self.peek_byte_at(0) {
            if b == b'\n' || b == b'\r' {
                break;
            }
            // Advance by a whole UTF-8 char so non-ASCII bytes in comments
            // (perfectly legal — comments are UTF-8) do not get split.
            let ch = self.peek_char().expect("byte present but not a char?");
            self.pos += ch.len_utf8();
        }
        Token::new(kind, self.span_from(start))
    }

    fn lex_block_comment(&mut self, start: usize) -> Token {
        // We already know src[pos..pos+2] == "/*". Block comments do NOT
        // nest (Doc 04 §1.4 — explicit). Unterminated yields E0003.
        self.pos += 2;
        loop {
            match (self.peek_byte_at(0), self.peek_byte_at(1)) {
                (Some(b'*'), Some(b'/')) => {
                    self.pos += 2;
                    return Token::new(TokenKind::BlockComment, self.span_from(start));
                }
                (Some(_), _) => {
                    // Advance whole-char so multi-byte content stays intact.
                    let ch = self.peek_char().expect("byte present");
                    self.pos += ch.len_utf8();
                }
                (None, _) => {
                    // EOF before `*/`. Emit an Error covering the unterminated
                    // comment range; the parser may surface this as FSM-E0003.
                    return Token::new(
                        TokenKind::Error(DiagnosticCode::E0003),
                        self.span_from(start),
                    );
                }
            }
        }
    }

    // ─── Identifiers + keywords ──────────────────────────────────────────

    fn lex_ident_or_keyword(&mut self, start: usize) -> Token {
        // First char already validated as is_ident_start. Consume the run.
        self.pos += 1;
        while let Some(b) = self.peek_byte_at(0) {
            if is_ident_continue_byte(b) {
                self.pos += 1;
            } else {
                break;
            }
        }
        let slice = &self.src[start..self.pos];
        let kind = keyword_kind(slice).unwrap_or(TokenKind::Ident);
        Token::new(kind, self.span_from(start))
    }

    // ─── Numbers ─────────────────────────────────────────────────────────

    fn lex_number(&mut self, start: usize) -> Token {
        // Check for hex / binary prefix (a leading `0x` / `0b` only — `0X`
        // and `0B` are not accepted per Doc 04 §1.3's "0x"/"0b" literal
        // forms).
        if self.peek_byte_at(0) == Some(b'0') {
            match self.peek_byte_at(1) {
                Some(b'x') => return self.lex_hex(start),
                Some(b'b') => return self.lex_binary(start),
                _ => {}
            }
        }
        self.lex_decimal_or_float(start)
    }

    fn lex_hex(&mut self, start: usize) -> Token {
        self.pos += 2; // consume `0x`
        let digits_start = self.pos;
        // Leading underscore directly after the prefix is INVALID per the
        // resolution recorded in this module's top doc comment.
        if self.peek_byte_at(0) == Some(b'_') {
            // Drain anything that looks like literal characters so the error
            // span covers the offending region and the parser can recover.
            while let Some(b) = self.peek_byte_at(0) {
                if b.is_ascii_hexdigit() || b == b'_' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            return Token::new(
                TokenKind::Error(DiagnosticCode::E0004),
                self.span_from(start),
            );
        }
        let mut saw_digit = false;
        while let Some(b) = self.peek_byte_at(0) {
            if b.is_ascii_hexdigit() {
                self.pos += 1;
                saw_digit = true;
            } else if b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if !saw_digit {
            // `0x` with nothing after it — invalid integer literal.
            // digits_start is unused on this path but recorded for symmetry.
            let _ = digits_start;
            return Token::new(
                TokenKind::Error(DiagnosticCode::E0004),
                self.span_from(start),
            );
        }
        Token::new(TokenKind::IntLiteral, self.span_from(start))
    }

    fn lex_binary(&mut self, start: usize) -> Token {
        self.pos += 2; // consume `0b`
        if self.peek_byte_at(0) == Some(b'_') {
            while let Some(b) = self.peek_byte_at(0) {
                if matches!(b, b'0' | b'1' | b'_') {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            return Token::new(
                TokenKind::Error(DiagnosticCode::E0004),
                self.span_from(start),
            );
        }
        let mut saw_digit = false;
        while let Some(b) = self.peek_byte_at(0) {
            if b == b'0' || b == b'1' {
                self.pos += 1;
                saw_digit = true;
            } else if b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if !saw_digit {
            return Token::new(
                TokenKind::Error(DiagnosticCode::E0004),
                self.span_from(start),
            );
        }
        Token::new(TokenKind::IntLiteral, self.span_from(start))
    }

    fn lex_decimal_or_float(&mut self, start: usize) -> Token {
        // Consume the integer-part digits + underscores.
        while let Some(b) = self.peek_byte_at(0) {
            if b.is_ascii_digit() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        // Float promotion requires `digit . digit` per Doc 04 §1.3:
        //     float = digit , { digit } , "." , digit , { digit } ;
        // A trailing `.` with no fractional digits is therefore NOT a float;
        // we leave the `.` for the next token (e.g., `5.size` → `Int(5)` `.`
        // `Ident(size)`). This matches the EBNF and avoids ambiguity with
        // member-access chains.
        if self.peek_byte_at(0) == Some(b'.')
            && self
                .peek_byte_at(1)
                .map(|b| b.is_ascii_digit())
                .unwrap_or(false)
        {
            self.pos += 1; // consume `.`
            while let Some(b) = self.peek_byte_at(0) {
                if b.is_ascii_digit() || b == b'_' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            return Token::new(TokenKind::FloatLiteral, self.span_from(start));
        }
        Token::new(TokenKind::IntLiteral, self.span_from(start))
    }

    // ─── String literals ────────────────────────────────────────────────

    fn lex_string(&mut self, start: usize) -> Token {
        self.pos += 1; // consume opening `"`
        loop {
            match self.peek_byte_at(0) {
                None => {
                    // EOF inside string → unterminated literal (E0002).
                    return Token::new(
                        TokenKind::Error(DiagnosticCode::E0002),
                        self.span_from(start),
                    );
                }
                Some(b'"') => {
                    self.pos += 1;
                    return Token::new(TokenKind::StringLiteral, self.span_from(start));
                }
                Some(b'\\') => {
                    // Escape — accept the spec-listed set (\\, \", \n, \r, \t).
                    // We do NOT validate further here; the parser may want to
                    // emit a more specific diagnostic for unknown escapes when
                    // it materialises the value. The lexer's job is to keep
                    // the stream aligned past the escape.
                    self.pos += 1;
                    if let Some(esc) = self.peek_char() {
                        self.pos += esc.len_utf8();
                    }
                }
                Some(b'\n') | Some(b'\r') => {
                    // Doc 04 §1.3 strings can contain UTF-8 except `"` / `\`;
                    // raw newlines are not explicitly forbidden but a string
                    // running off the line is almost certainly a missing `"`.
                    // Treat as unterminated.
                    return Token::new(
                        TokenKind::Error(DiagnosticCode::E0002),
                        self.span_from(start),
                    );
                }
                Some(_) => {
                    let ch = self.peek_char().expect("byte present");
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    // ─── Stable IDs ──────────────────────────────────────────────────────

    fn lex_at_or_stable_id(&mut self, start: usize) -> Token {
        self.pos += 1; // consume `@`
                       // Stable-ID form: `@` IMMEDIATELY followed by an identifier-start.
                       // No whitespace allowed between `@` and the name per Doc 04 §14.
                       // If the next char is not ident-start, emit a bare `At` token so the
                       // parser can recover (e.g., the `@id("…")` annotation form, which is
                       // `At` `Ident("id")` `LParen` …).
        match self.peek_char() {
            Some(c) if is_ident_start(c) => {
                self.pos += c.len_utf8();
                while let Some(b) = self.peek_byte_at(0) {
                    if is_ident_continue_byte(b) {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                Token::new(TokenKind::StableId, self.span_from(start))
            }
            _ => Token::new(TokenKind::At, self.span_from(start)),
        }
    }

    // ─── Multi-char operators ────────────────────────────────────────────

    fn lex_minus_or_arrow(&mut self, start: usize) -> Token {
        if self.peek_byte_at(1) == Some(b'>') {
            self.pos += 2;
            Token::new(TokenKind::Arrow, self.span_from(start))
        } else {
            self.advance_single(start, TokenKind::Minus)
        }
    }

    fn lex_tilde_or_history_arrow(&mut self, start: usize) -> Token {
        if self.peek_byte_at(1) == Some(b'>') {
            self.pos += 2;
            Token::new(TokenKind::HistoryArrow, self.span_from(start))
        } else {
            self.advance_single(start, TokenKind::Tilde)
        }
    }

    fn lex_eq_or_eqeq(&mut self, start: usize) -> Token {
        if self.peek_byte_at(1) == Some(b'=') {
            self.pos += 2;
            Token::new(TokenKind::EqEq, self.span_from(start))
        } else {
            self.advance_single(start, TokenKind::Eq)
        }
    }

    fn lex_bang_or_bangeq(&mut self, start: usize) -> Token {
        if self.peek_byte_at(1) == Some(b'=') {
            self.pos += 2;
            Token::new(TokenKind::BangEq, self.span_from(start))
        } else {
            self.advance_single(start, TokenKind::Bang)
        }
    }

    fn lex_lt_le_or_shl(&mut self, start: usize) -> Token {
        match self.peek_byte_at(1) {
            Some(b'=') => {
                self.pos += 2;
                Token::new(TokenKind::Le, self.span_from(start))
            }
            Some(b'<') => {
                self.pos += 2;
                Token::new(TokenKind::Shl, self.span_from(start))
            }
            _ => self.advance_single(start, TokenKind::Lt),
        }
    }

    fn lex_gt_ge_or_shr(&mut self, start: usize) -> Token {
        match self.peek_byte_at(1) {
            Some(b'=') => {
                self.pos += 2;
                Token::new(TokenKind::Ge, self.span_from(start))
            }
            Some(b'>') => {
                self.pos += 2;
                Token::new(TokenKind::Shr, self.span_from(start))
            }
            _ => self.advance_single(start, TokenKind::Gt),
        }
    }

    fn lex_amp_or_ampamp(&mut self, start: usize) -> Token {
        if self.peek_byte_at(1) == Some(b'&') {
            self.pos += 2;
            Token::new(TokenKind::AmpAmp, self.span_from(start))
        } else {
            self.advance_single(start, TokenKind::Amp)
        }
    }

    fn lex_pipe_or_pipepipe(&mut self, start: usize) -> Token {
        if self.peek_byte_at(1) == Some(b'|') {
            self.pos += 2;
            Token::new(TokenKind::PipePipe, self.span_from(start))
        } else {
            self.advance_single(start, TokenKind::Pipe)
        }
    }

    // ─── Unknown character fallback ──────────────────────────────────────

    fn lex_unknown(&mut self, start: usize, ch: char) -> Token {
        // CRITICAL — Doc 00 §B fix to VALIDATION_REPORT 2.1: advance by the
        // FULL UTF-8 width of the character, not 1 byte. Otherwise multi-byte
        // characters get split and every subsequent token is misaligned.
        self.pos += ch.len_utf8();
        Token::new(
            TokenKind::Error(DiagnosticCode::E0001),
            self.span_from(start),
        )
    }
}

// ─── Free-standing convenience ───────────────────────────────────────────

/// Tokenize `src` to end of file. Returns every token including trivia and
/// `Error` tokens; the final entry is always `Eof`.
///
/// This is the function most callers want. Use [`Lexer::next_token`] /
/// [`Lexer::peek_token`] directly when you need to stream without buffering.
pub fn tokenize(src: &str) -> Vec<Token> {
    let mut lx = Lexer::new(src);
    let mut out = Vec::new();
    loop {
        let tok = lx.next_token();
        let is_eof = matches!(tok.kind, TokenKind::Eof);
        out.push(tok);
        if is_eof {
            break;
        }
    }
    out
}

// ─── Char-class helpers ──────────────────────────────────────────────────

/// First byte/char of an identifier per Doc 04 §1.2: ASCII letter or `_`.
/// Underscores are accepted as a START character because well-known DSL
/// patterns like `_unused_target` and `_internal_state` are useful even
/// though they are not strictly the EBNF letter-set; the parser may still
/// emit a style lint over leading-underscore idents.
fn is_ident_start(c: char) -> bool {
    matches!(c, 'A'..='Z' | 'a'..='z' | '_')
}

/// Subsequent characters per Doc 04 §1.2: letter, digit, or `_`. Operates
/// on a raw byte so the hot path stays branch-light; non-ASCII bytes always
/// fall through to the unknown-character handler (which uses `char` width).
fn is_ident_continue_byte(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: tokenize `src` and drop trivia. Most rule tests want the
    /// non-trivia sequence.
    fn lex_no_trivia(src: &str) -> Vec<Token> {
        tokenize(src)
            .into_iter()
            .filter(|t| !t.kind.is_trivia())
            .collect()
    }

    /// Helper: pull just the kinds for a quick structural assert.
    fn kinds(src: &str) -> Vec<TokenKind> {
        lex_no_trivia(src).into_iter().map(|t| t.kind).collect()
    }

    // ─── Construction + EOF ──────────────────────────────────────────────

    #[test]
    fn empty_source_is_just_eof() {
        let toks = tokenize("");
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].kind, TokenKind::Eof);
        assert_eq!(toks[0].span, Span::empty(0));
    }

    #[test]
    fn eof_is_idempotent() {
        let mut lx = Lexer::new("");
        assert_eq!(lx.next_token().kind, TokenKind::Eof);
        assert_eq!(lx.next_token().kind, TokenKind::Eof);
        assert_eq!(lx.next_token().kind, TokenKind::Eof);
    }

    #[test]
    fn bom_is_silently_consumed() {
        // U+FEFF is 3 UTF-8 bytes.
        let src = "\u{FEFF}machine";
        let toks = lex_no_trivia(src);
        assert_eq!(toks.len(), 2);
        assert_eq!(toks[0].kind, TokenKind::KwMachine);
        // Span should start after the BOM (offset 3), not at 0.
        assert_eq!(toks[0].span, Span::new(3, 10));
    }

    #[test]
    fn peek_does_not_advance() {
        let mut lx = Lexer::new("machine Foo");
        let a = lx.peek_token();
        let b = lx.peek_token();
        let c = lx.next_token();
        assert_eq!(a.kind, TokenKind::KwMachine);
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.kind, c.kind);
        assert_eq!(a.span, c.span);
    }

    // ─── Every keyword variant ───────────────────────────────────────────

    #[test]
    fn all_53_keywords_resolve_to_their_variant() {
        // Table-driven; one entry per keyword. If a keyword is added to
        // Doc 04 §1.5, add it here and to `keyword_kind` in token.rs.
        let table: &[(&str, TokenKind)] = &[
            ("after", TokenKind::KwAfter),
            ("as", TokenKind::KwAs),
            ("bool", TokenKind::KwBool),
            ("cancel", TokenKind::KwCancel),
            ("choice", TokenKind::KwChoice),
            ("composite", TokenKind::KwComposite),
            ("const", TokenKind::KwConst),
            ("context", TokenKind::KwContext),
            ("deep_history", TokenKind::KwDeepHistory),
            ("defer", TokenKind::KwDefer),
            ("done", TokenKind::KwDone),
            ("else", TokenKind::KwElse),
            ("enum", TokenKind::KwEnum),
            ("every", TokenKind::KwEvery),
            ("export", TokenKind::KwExport),
            ("extern", TokenKind::KwExtern),
            ("f32", TokenKind::KwF32),
            ("f64", TokenKind::KwF64),
            ("false", TokenKind::KwFalse),
            ("feature", TokenKind::KwFeature),
            ("final", TokenKind::KwFinal),
            ("fork", TokenKind::KwFork),
            ("i8", TokenKind::KwI8),
            ("i16", TokenKind::KwI16),
            ("i32", TokenKind::KwI32),
            ("i64", TokenKind::KwI64),
            ("import", TokenKind::KwImport),
            ("initial", TokenKind::KwInitial),
            ("is", TokenKind::KwIs),
            ("join", TokenKind::KwJoin),
            ("junction", TokenKind::KwJunction),
            ("language", TokenKind::KwLanguage),
            ("machine", TokenKind::KwMachine),
            ("ms", TokenKind::KwMs),
            ("on", TokenKind::KwOn),
            ("opaque", TokenKind::KwOpaque),
            ("parallel", TokenKind::KwParallel),
            ("priority", TokenKind::KwPriority),
            ("pure", TokenKind::KwPure),
            ("raise", TokenKind::KwRaise),
            ("region", TokenKind::KwRegion),
            ("schedule", TokenKind::KwSchedule),
            ("send", TokenKind::KwSend),
            ("shallow_history", TokenKind::KwShallowHistory),
            ("state", TokenKind::KwState),
            ("submachine", TokenKind::KwSubmachine),
            ("target", TokenKind::KwTarget),
            ("to", TokenKind::KwTo),
            ("true", TokenKind::KwTrue),
            ("u8", TokenKind::KwU8),
            ("u16", TokenKind::KwU16),
            ("u32", TokenKind::KwU32),
            ("u64", TokenKind::KwU64),
        ];
        assert_eq!(table.len(), 53, "keyword table drift vs Doc 04 §1.5");
        for (src, expected) in table {
            let toks = lex_no_trivia(src);
            assert_eq!(toks.len(), 2, "for {src:?}");
            assert_eq!(toks[0].kind, *expected, "for {src:?}");
            assert_eq!(toks[1].kind, TokenKind::Eof);
        }
    }

    #[test]
    fn keyword_prefix_is_still_an_identifier() {
        // `machine_a` shares a 7-char prefix with `machine` but is its own ident.
        let toks = lex_no_trivia("machine_a machine machineX");
        assert_eq!(toks.len(), 4);
        assert_eq!(toks[0].kind, TokenKind::Ident);
        assert_eq!(toks[1].kind, TokenKind::KwMachine);
        assert_eq!(toks[2].kind, TokenKind::Ident);
        assert_eq!(toks[3].kind, TokenKind::Eof);
    }

    #[test]
    fn leading_underscore_is_identifier_not_number() {
        let toks = lex_no_trivia("_internal");
        assert_eq!(toks[0].kind, TokenKind::Ident);
        assert_eq!(toks[0].span, Span::new(0, 9));
    }

    // ─── Every operator and delimiter ────────────────────────────────────

    #[test]
    fn all_operators_and_delimiters() {
        let table: &[(&str, TokenKind)] = &[
            ("->", TokenKind::Arrow),
            ("~>", TokenKind::HistoryArrow),
            (":", TokenKind::Colon),
            (";", TokenKind::Semicolon),
            (",", TokenKind::Comma),
            (".", TokenKind::Dot),
            ("=", TokenKind::Eq),
            ("@", TokenKind::At),
            ("{", TokenKind::LBrace),
            ("}", TokenKind::RBrace),
            ("(", TokenKind::LParen),
            (")", TokenKind::RParen),
            ("[", TokenKind::LBracket),
            ("]", TokenKind::RBracket),
            ("==", TokenKind::EqEq),
            ("!=", TokenKind::BangEq),
            ("<", TokenKind::Lt),
            (">", TokenKind::Gt),
            ("<=", TokenKind::Le),
            (">=", TokenKind::Ge),
            ("&&", TokenKind::AmpAmp),
            ("||", TokenKind::PipePipe),
            ("!", TokenKind::Bang),
            ("+", TokenKind::Plus),
            ("-", TokenKind::Minus),
            ("*", TokenKind::Star),
            ("/", TokenKind::Slash),
            ("%", TokenKind::Percent),
            ("&", TokenKind::Amp),
            ("|", TokenKind::Pipe),
            ("^", TokenKind::Caret),
            ("~", TokenKind::Tilde),
            ("<<", TokenKind::Shl),
            (">>", TokenKind::Shr),
        ];
        for (src, expected) in table {
            let toks = lex_no_trivia(src);
            assert_eq!(toks.len(), 2, "for {src:?}");
            assert_eq!(toks[0].kind, *expected, "for {src:?}");
        }
    }

    #[test]
    fn arrow_disambiguates_against_minus_gt() {
        // `- >` (with space) is `Minus` then `Gt`, not `Arrow`.
        let toks = lex_no_trivia("- >");
        assert_eq!(toks[0].kind, TokenKind::Minus);
        assert_eq!(toks[1].kind, TokenKind::Gt);
    }

    #[test]
    fn shl_shr_are_two_char_tokens() {
        let toks = lex_no_trivia("a<<1>>2");
        assert_eq!(
            toks.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::Ident,
                TokenKind::Shl,
                TokenKind::IntLiteral,
                TokenKind::Shr,
                TokenKind::IntLiteral,
                TokenKind::Eof,
            ]
        );
    }

    // ─── Integer literals ────────────────────────────────────────────────

    #[test]
    fn decimal_literal_simple() {
        let toks = lex_no_trivia("42");
        assert_eq!(toks[0].kind, TokenKind::IntLiteral);
        assert_eq!(toks[0].span, Span::new(0, 2));
    }

    #[test]
    fn decimal_literal_with_underscores() {
        let toks = lex_no_trivia("1_000_000");
        assert_eq!(toks[0].kind, TokenKind::IntLiteral);
        assert_eq!(toks[0].span, Span::new(0, 9));
    }

    #[test]
    fn decimal_trailing_underscore_is_legal() {
        // Resolution recorded in this module's top doc comment.
        let toks = lex_no_trivia("123_");
        assert_eq!(toks[0].kind, TokenKind::IntLiteral);
        assert_eq!(toks[0].span, Span::new(0, 4));
    }

    #[test]
    fn decimal_double_underscore_is_legal() {
        let toks = lex_no_trivia("1__000");
        assert_eq!(toks[0].kind, TokenKind::IntLiteral);
    }

    #[test]
    fn hex_literal() {
        let toks = lex_no_trivia("0xFF_AB_CD");
        assert_eq!(toks[0].kind, TokenKind::IntLiteral);
        assert_eq!(toks[0].span, Span::new(0, 10));
    }

    #[test]
    fn binary_literal() {
        let toks = lex_no_trivia("0b1010_1100");
        assert_eq!(toks[0].kind, TokenKind::IntLiteral);
    }

    #[test]
    fn hex_leading_underscore_after_prefix_is_invalid() {
        let toks = lex_no_trivia("0x_FF");
        match toks[0].kind {
            TokenKind::Error(DiagnosticCode::E0004) => {}
            other => panic!("expected E0004, got {other:?}"),
        }
        // The error span MUST cover the whole offending literal so the parser
        // can underline it in the editor.
        assert_eq!(toks[0].span, Span::new(0, 5));
    }

    #[test]
    fn binary_leading_underscore_after_prefix_is_invalid() {
        let toks = lex_no_trivia("0b_10");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Error(DiagnosticCode::E0004)
        ));
    }

    #[test]
    fn hex_with_no_digits_is_invalid() {
        let toks = lex_no_trivia("0x");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Error(DiagnosticCode::E0004)
        ));
    }

    // ─── Float literals ──────────────────────────────────────────────────

    #[test]
    fn float_literal_basic() {
        let toks = lex_no_trivia("3.14");
        assert_eq!(toks[0].kind, TokenKind::FloatLiteral);
        assert_eq!(toks[0].span, Span::new(0, 4));
    }

    #[test]
    fn float_literal_leading_zero() {
        let toks = lex_no_trivia("0.5");
        assert_eq!(toks[0].kind, TokenKind::FloatLiteral);
    }

    #[test]
    fn bare_dot_five_is_dot_then_int() {
        // EBNF requires `digit . digit`; a leading dot is NOT a float.
        let toks = lex_no_trivia(".5");
        assert_eq!(toks[0].kind, TokenKind::Dot);
        assert_eq!(toks[1].kind, TokenKind::IntLiteral);
    }

    #[test]
    fn trailing_dot_is_int_then_dot() {
        // `5.` (no fractional digit) is NOT a float. Useful for things like
        // `5.size` member access — but `size` is an ident so this stays sane.
        let toks = lex_no_trivia("5.");
        assert_eq!(toks[0].kind, TokenKind::IntLiteral);
        assert_eq!(toks[0].span, Span::new(0, 1));
        assert_eq!(toks[1].kind, TokenKind::Dot);
    }

    // ─── String literals ─────────────────────────────────────────────────

    #[test]
    fn string_basic() {
        let toks = lex_no_trivia(r#""hello""#);
        assert_eq!(toks[0].kind, TokenKind::StringLiteral);
        assert_eq!(toks[0].span, Span::new(0, 7));
    }

    #[test]
    fn string_with_escapes() {
        let toks = lex_no_trivia(r#""line1\nline2\t\"quoted\\""#);
        assert_eq!(toks[0].kind, TokenKind::StringLiteral);
    }

    #[test]
    fn string_with_utf8_content() {
        // UTF-8 inside strings is allowed (Doc 04 §1.3 string_char).
        let toks = lex_no_trivia("\"привет 你好 🙂\"");
        assert_eq!(toks[0].kind, TokenKind::StringLiteral);
    }

    #[test]
    fn unterminated_string_yields_e0002() {
        let toks = lex_no_trivia(r#""open"#);
        assert!(matches!(
            toks[0].kind,
            TokenKind::Error(DiagnosticCode::E0002)
        ));
    }

    #[test]
    fn string_with_embedded_newline_is_unterminated() {
        let toks = tokenize("\"abc\ndef\"");
        let first = toks.iter().find(|t| !t.kind.is_trivia()).unwrap();
        assert!(matches!(
            first.kind,
            TokenKind::Error(DiagnosticCode::E0002)
        ));
    }

    // ─── Comments ────────────────────────────────────────────────────────

    #[test]
    fn line_comment_preserved_as_trivia() {
        let toks = tokenize("// hi\nmachine");
        assert_eq!(toks[0].kind, TokenKind::LineComment);
        assert_eq!(toks[0].span, Span::new(0, 5));
        assert_eq!(toks[1].kind, TokenKind::Newline);
        assert_eq!(toks[2].kind, TokenKind::KwMachine);
    }

    #[test]
    fn doc_comment_distinct_from_line_comment() {
        let toks = tokenize("/// doc\n// regular\n");
        assert_eq!(toks[0].kind, TokenKind::DocComment);
        // `// regular` after the newline → LineComment.
        let line_comment = toks
            .iter()
            .find(|t| matches!(t.kind, TokenKind::LineComment))
            .expect("line comment present");
        let line_text = &"/// doc\n// regular\n"[line_comment.span.start..line_comment.span.end];
        assert_eq!(line_text, "// regular");
    }

    #[test]
    fn block_comment_simple() {
        let toks = tokenize("/* hello */");
        assert_eq!(toks[0].kind, TokenKind::BlockComment);
        assert_eq!(toks[0].span, Span::new(0, 11));
    }

    #[test]
    fn block_comment_does_not_nest() {
        // Per Doc 04 §1.4 explicit "does not nest". So the first `*/` closes.
        // The trailing `*/` here becomes a `Star Slash` token pair.
        let toks = lex_no_trivia("/* outer /* inner */ */");
        assert!(matches!(toks[0].kind, TokenKind::Star | TokenKind::Slash));
    }

    #[test]
    fn unterminated_block_comment_yields_e0003() {
        let toks = lex_no_trivia("/* open");
        assert!(matches!(
            toks[0].kind,
            TokenKind::Error(DiagnosticCode::E0003)
        ));
    }

    // ─── Stable IDs ──────────────────────────────────────────────────────

    #[test]
    fn stable_id_is_one_token() {
        let toks = lex_no_trivia("@id_a");
        assert_eq!(toks[0].kind, TokenKind::StableId);
        assert_eq!(toks[0].span, Span::new(0, 5));
    }

    #[test]
    fn at_followed_by_space_is_bare_at() {
        let toks = tokenize("@ id_a");
        // Filter trivia for clarity.
        let non_trivia: Vec<_> = toks
            .into_iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            non_trivia,
            vec![TokenKind::At, TokenKind::Ident, TokenKind::Eof]
        );
    }

    #[test]
    fn at_followed_by_paren_is_bare_at() {
        // `@id("…")` form: lexer produces `At` `Ident("id")` `LParen` ...
        // even though there's no space between `@` and `id` — `@id` IS
        // a StableId. Test the bare-`@` recovery case via `@(`.
        let toks = lex_no_trivia("@(x)");
        assert_eq!(
            toks.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::At,
                TokenKind::LParen,
                TokenKind::Ident,
                TokenKind::RParen,
                TokenKind::Eof,
            ]
        );
    }

    // ─── UTF-8 error recovery — the critical regression ──────────────────

    #[test]
    fn utf8_multibyte_does_not_corrupt_stream() {
        // The Cyrillic 'ё' (U+0451) is 2 bytes in UTF-8. Doc 00 §B fix:
        // the lexer must advance by ch.len_utf8(), not 1. We assert both the
        // span width and that the next token after it lexes correctly.
        let src = "ё machine";
        let toks = tokenize(src);
        // First non-EOF non-trivia token is the Error.
        let first = &toks[0];
        assert!(matches!(
            first.kind,
            TokenKind::Error(DiagnosticCode::E0001)
        ));
        assert_eq!(first.span, Span::new(0, "ё".len()));
        assert_eq!(first.span.len(), 2);
        // The lexer should NOT split the multi-byte char — subsequent tokens
        // must resolve normally.
        let kws: Vec<TokenKind> = toks
            .iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            kws,
            vec![
                TokenKind::Error(DiagnosticCode::E0001),
                TokenKind::KwMachine,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn utf8_four_byte_char_advances_full_width() {
        // U+1F642 SLIGHTLY SMILING FACE = 4 bytes in UTF-8.
        let src = "\u{1F642}x";
        let toks = tokenize(src);
        let err = &toks[0];
        assert!(matches!(err.kind, TokenKind::Error(DiagnosticCode::E0001)));
        assert_eq!(err.span.len(), 4, "should advance 4 bytes for emoji");
        let non_trivia: Vec<TokenKind> = toks
            .into_iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            non_trivia,
            vec![
                TokenKind::Error(DiagnosticCode::E0001),
                TokenKind::Ident,
                TokenKind::Eof,
            ]
        );
    }

    // ─── Whitespace + newlines ───────────────────────────────────────────

    #[test]
    fn whitespace_and_newline_separate_tokens() {
        let toks = tokenize("  \n\t");
        let kinds: Vec<TokenKind> = toks.into_iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Whitespace,
                TokenKind::Newline,
                TokenKind::Whitespace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn crlf_is_one_newline_token() {
        let toks = tokenize("a\r\nb");
        let nl = toks
            .iter()
            .find(|t| matches!(t.kind, TokenKind::Newline))
            .unwrap();
        assert_eq!(nl.span.len(), 2);
    }

    // ─── Display impl ────────────────────────────────────────────────────

    #[test]
    fn display_renders_source_spellings() {
        assert_eq!(TokenKind::KwMachine.to_string(), "machine");
        assert_eq!(TokenKind::Arrow.to_string(), "->");
        assert_eq!(TokenKind::HistoryArrow.to_string(), "~>");
        assert_eq!(TokenKind::Ident.to_string(), "identifier");
        assert_eq!(TokenKind::IntLiteral.to_string(), "integer literal");
        assert_eq!(TokenKind::Eof.to_string(), "end of file");
    }

    // ─── Realistic mini-source ───────────────────────────────────────────

    #[test]
    fn small_machine_lex_smoke() {
        // Note: `2.0` is a FloatLiteral by EBNF (digit . digit). The
        // `language` declaration in Doc 04 §2 uses two integers separated by
        // `.` — `language fsm 2 . 0` would parse as int-dot-int. For lexer
        // testing we use a form that yields an integer naturally.
        let src =
            "language fsm 2.0\nmachine M { initial S\nstate S { on E -> S after 5 ms -> S } }";
        let toks = kinds(src);
        assert!(toks.contains(&TokenKind::KwLanguage));
        assert!(toks.contains(&TokenKind::KwMachine));
        assert!(toks.contains(&TokenKind::KwInitial));
        assert!(toks.contains(&TokenKind::KwState));
        assert!(toks.contains(&TokenKind::KwOn));
        assert!(toks.contains(&TokenKind::Arrow));
        assert!(toks.contains(&TokenKind::FloatLiteral));
        assert!(toks.contains(&TokenKind::IntLiteral));
        assert_eq!(toks.last(), Some(&TokenKind::Eof));
    }

    // ─── Coverage sweep: every TokenKind variant produced by at least one
    // source (compact regression net for refactors). ─────────────────────

    #[test]
    fn every_variant_is_producible() {
        // We assemble a single source that touches every variant, then
        // assert the produced set covers them all (modulo `Eof` which always
        // appears last).
        let src = concat!(
            // language line — keywords + int + dot.
            "language fsm 2.0\n",
            // every keyword once (catches an accidental Display regression).
            "after as bool cancel choice composite const context deep_history defer ",
            "done else enum every export extern f32 f64 false feature ",
            "final fork i8 i16 i32 i64 import initial is join junction language ",
            "machine ms on opaque parallel priority pure raise region schedule ",
            "send shallow_history state submachine target to true u8 u16 u32 u64\n",
            // operators
            "-> ~> : ; , . = @ { } ( ) [ ] == != < > <= >= && || ! + - * / % & | ^ ~ << >>\n",
            // numeric / string / float / stable id / ident
            "1_000 0xFF 0b10 3.14 \"hi\" @my_id ident\n",
            // comments
            "// line\n/// doc\n/* block */\n",
            // illegal char to spawn Error
            "ё\n",
        );
        let mut seen: std::collections::HashSet<core::mem::Discriminant<TokenKind>> =
            std::collections::HashSet::new();
        for t in tokenize(src) {
            seen.insert(core::mem::discriminant(&t.kind));
        }

        // Build the full target set by constructing one token of every
        // variant.
        let all: &[TokenKind] = &[
            // keywords
            TokenKind::KwAfter,
            TokenKind::KwAs,
            TokenKind::KwBool,
            TokenKind::KwCancel,
            TokenKind::KwChoice,
            TokenKind::KwComposite,
            TokenKind::KwConst,
            TokenKind::KwContext,
            TokenKind::KwDeepHistory,
            TokenKind::KwDefer,
            TokenKind::KwDone,
            TokenKind::KwElse,
            TokenKind::KwEnum,
            TokenKind::KwEvery,
            TokenKind::KwExport,
            TokenKind::KwExtern,
            TokenKind::KwF32,
            TokenKind::KwF64,
            TokenKind::KwFalse,
            TokenKind::KwFeature,
            TokenKind::KwFinal,
            TokenKind::KwFork,
            TokenKind::KwI8,
            TokenKind::KwI16,
            TokenKind::KwI32,
            TokenKind::KwI64,
            TokenKind::KwImport,
            TokenKind::KwInitial,
            TokenKind::KwIs,
            TokenKind::KwJoin,
            TokenKind::KwJunction,
            TokenKind::KwLanguage,
            TokenKind::KwMachine,
            TokenKind::KwMs,
            TokenKind::KwOn,
            TokenKind::KwOpaque,
            TokenKind::KwParallel,
            TokenKind::KwPriority,
            TokenKind::KwPure,
            TokenKind::KwRaise,
            TokenKind::KwRegion,
            TokenKind::KwSchedule,
            TokenKind::KwSend,
            TokenKind::KwShallowHistory,
            TokenKind::KwState,
            TokenKind::KwSubmachine,
            TokenKind::KwTarget,
            TokenKind::KwTo,
            TokenKind::KwTrue,
            TokenKind::KwU8,
            TokenKind::KwU16,
            TokenKind::KwU32,
            TokenKind::KwU64,
            // operators / delimiters
            TokenKind::Arrow,
            TokenKind::HistoryArrow,
            TokenKind::Colon,
            TokenKind::Semicolon,
            TokenKind::Comma,
            TokenKind::Dot,
            TokenKind::Eq,
            TokenKind::At,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::EqEq,
            TokenKind::BangEq,
            TokenKind::Lt,
            TokenKind::Gt,
            TokenKind::Le,
            TokenKind::Ge,
            TokenKind::AmpAmp,
            TokenKind::PipePipe,
            TokenKind::Bang,
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Amp,
            TokenKind::Pipe,
            TokenKind::Caret,
            TokenKind::Tilde,
            TokenKind::Shl,
            TokenKind::Shr,
            // names
            TokenKind::Ident,
            TokenKind::StableId,
            // literals
            TokenKind::IntLiteral,
            TokenKind::FloatLiteral,
            TokenKind::StringLiteral,
            // trivia
            TokenKind::Whitespace,
            TokenKind::Newline,
            TokenKind::LineComment,
            TokenKind::BlockComment,
            TokenKind::DocComment,
            // lexer-internal
            TokenKind::Error(DiagnosticCode::E0001),
            TokenKind::Eof,
        ];
        for kind in all {
            assert!(
                seen.contains(&core::mem::discriminant(kind)),
                "variant not produced by the sweep source: {kind:?}"
            );
        }
    }
}
