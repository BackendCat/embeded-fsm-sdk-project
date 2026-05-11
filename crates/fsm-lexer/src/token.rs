//! `Token` and `TokenKind` definitions for FSM-Lang.
//!
//! Reference: `docs/04-DSL-Specification.md` §1.5 (keywords) + §1.6 (operators
//! and delimiters) + §1.3 (literals) + §1.2 (identifiers) + §1.4 (comments).
//!
//! Tokens are produced by the lexer as `(kind, span)` pairs over a borrowed
//! `&str`; the caller slices the source to recover token text. This matches the
//! "zero-copy" contract from Doc 20 §3.

use core::fmt;

use fsm_diagnostics::{DiagnosticCode, Span};

/// A single lexical token.
///
/// `kind` selects the variant; `span` is the half-open byte range in the source
/// string. For literals and identifiers, the source slice carries the textual
/// value — the lexer does not allocate per token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    /// Convenience constructor.
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Every lexical category produced by [`crate::Lexer`].
///
/// Keywords (Doc 04 §1.5) are `Kw*`. Operators and delimiters (§1.6) follow
/// their conventional names. Literals (§1.3) split into `IntLiteral`,
/// `FloatLiteral`, and `StringLiteral` — `true`/`false` are keywords per the
/// §1.5 table, NOT literals. Identifiers (§1.2) and stable-ID annotations
/// (§14) are `Ident` and `StableId`. Comments and whitespace are preserved as
/// trivia so the CST / formatter can round-trip the source verbatim.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // ─── Keywords (Doc 04 §1.5, all 53) ──────────────────────────────────
    KwAfter,
    KwAs,
    KwBool,
    KwCancel,
    KwChoice,
    KwComposite,
    KwConst,
    KwContext,
    KwDeepHistory,
    KwDefer,
    KwDone,
    KwElse,
    KwEnum,
    KwEvery,
    KwExport,
    KwExtern,
    KwF32,
    KwF64,
    KwFalse,
    KwFeature,
    KwFinal,
    KwFork,
    KwI8,
    KwI16,
    KwI32,
    KwI64,
    KwImport,
    KwInitial,
    KwIs,
    KwJoin,
    KwJunction,
    KwLanguage,
    KwMachine,
    KwMs,
    KwOn,
    KwOpaque,
    KwParallel,
    KwPriority,
    KwPure,
    KwRaise,
    KwRegion,
    KwSchedule,
    KwSend,
    KwShallowHistory,
    KwState,
    KwSubmachine,
    KwTarget,
    KwTo,
    KwTrue,
    KwU8,
    KwU16,
    KwU32,
    KwU64,

    // ─── Operators and delimiters (Doc 04 §1.6) ──────────────────────────
    /// `->`
    Arrow,
    /// `~>` (local transition / history-target arrow)
    HistoryArrow,
    Colon,
    Semicolon,
    Comma,
    Dot,
    /// `=`
    Eq,
    /// `@` — when followed by an identifier the lexer emits `StableId`
    /// instead; bare `@` is only produced when the following character is
    /// not an identifier-start.
    At,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,

    /// `==`
    EqEq,
    /// `!=`
    BangEq,
    Lt,
    Gt,
    /// `<=`
    Le,
    /// `>=`
    Ge,
    /// `&&`
    AmpAmp,
    /// `||`
    PipePipe,
    /// `!`
    Bang,

    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    Amp,
    Pipe,
    Caret,
    Tilde,
    /// `<<`
    Shl,
    /// `>>`
    Shr,

    // ─── Literals (Doc 04 §1.3) ──────────────────────────────────────────
    /// Decimal, hex (`0x…`), or binary (`0b…`) integer. Raw slice is preserved
    /// in `Token::span`; parsing to a numeric value is the parser's job
    /// (see Doc 04 §3.2 on overflow semantics).
    IntLiteral,
    FloatLiteral,
    StringLiteral,

    // ─── Names (Doc 04 §1.2 + §14) ───────────────────────────────────────
    Ident,
    /// `@some_id` form. The `@` plus the following identifier is one token;
    /// a bare `@` not followed by an identifier-start lexes as [`At`].
    ///
    /// [`At`]: TokenKind::At
    StableId,

    // ─── Trivia (preserved for CST + formatter) ──────────────────────────
    Whitespace,
    Newline,
    LineComment,
    BlockComment,
    /// `///` — attaches to the next declaration per Doc 04 §1.4.
    DocComment,

    // ─── Lexer-internal ──────────────────────────────────────────────────
    /// Unrecognised input. Carries the diagnostic code that should be raised
    /// when the parser sees this token. The lexer never aborts; the parser
    /// is responsible for translating `Error` tokens into user-facing
    /// `Diagnostic` objects (Doc 20 §3.4).
    Error(DiagnosticCode),
    /// Returned once the source is exhausted; subsequent `next_token` calls
    /// keep returning `Eof`.
    Eof,
}

impl TokenKind {
    /// `true` for trivia tokens — whitespace, newlines, and the three comment
    /// variants. The parser may filter these out; the formatter preserves
    /// them.
    pub const fn is_trivia(&self) -> bool {
        matches!(
            self,
            TokenKind::Whitespace
                | TokenKind::Newline
                | TokenKind::LineComment
                | TokenKind::BlockComment
                | TokenKind::DocComment
        )
    }

    /// `true` if this kind is a reserved keyword from Doc 04 §1.5.
    pub const fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::KwAfter
                | TokenKind::KwAs
                | TokenKind::KwBool
                | TokenKind::KwCancel
                | TokenKind::KwChoice
                | TokenKind::KwComposite
                | TokenKind::KwConst
                | TokenKind::KwContext
                | TokenKind::KwDeepHistory
                | TokenKind::KwDefer
                | TokenKind::KwDone
                | TokenKind::KwElse
                | TokenKind::KwEnum
                | TokenKind::KwEvery
                | TokenKind::KwExport
                | TokenKind::KwExtern
                | TokenKind::KwF32
                | TokenKind::KwF64
                | TokenKind::KwFalse
                | TokenKind::KwFeature
                | TokenKind::KwFinal
                | TokenKind::KwFork
                | TokenKind::KwI8
                | TokenKind::KwI16
                | TokenKind::KwI32
                | TokenKind::KwI64
                | TokenKind::KwImport
                | TokenKind::KwInitial
                | TokenKind::KwIs
                | TokenKind::KwJoin
                | TokenKind::KwJunction
                | TokenKind::KwLanguage
                | TokenKind::KwMachine
                | TokenKind::KwMs
                | TokenKind::KwOn
                | TokenKind::KwOpaque
                | TokenKind::KwParallel
                | TokenKind::KwPriority
                | TokenKind::KwPure
                | TokenKind::KwRaise
                | TokenKind::KwRegion
                | TokenKind::KwSchedule
                | TokenKind::KwSend
                | TokenKind::KwShallowHistory
                | TokenKind::KwState
                | TokenKind::KwSubmachine
                | TokenKind::KwTarget
                | TokenKind::KwTo
                | TokenKind::KwTrue
                | TokenKind::KwU8
                | TokenKind::KwU16
                | TokenKind::KwU32
                | TokenKind::KwU64
        )
    }
}

/// Human-readable label used by parser diagnostics ("expected `;`, found …").
///
/// Operators render as the source text (`Arrow` → `"->"`). Keywords render
/// as the keyword spelling (`KwMachine` → `"machine"`). Open-ended categories
/// (`Ident`, `IntLiteral`, …) render as a category noun.
impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            // Keywords — exact source spelling.
            TokenKind::KwAfter => "after",
            TokenKind::KwAs => "as",
            TokenKind::KwBool => "bool",
            TokenKind::KwCancel => "cancel",
            TokenKind::KwChoice => "choice",
            TokenKind::KwComposite => "composite",
            TokenKind::KwConst => "const",
            TokenKind::KwContext => "context",
            TokenKind::KwDeepHistory => "deep_history",
            TokenKind::KwDefer => "defer",
            TokenKind::KwDone => "done",
            TokenKind::KwElse => "else",
            TokenKind::KwEnum => "enum",
            TokenKind::KwEvery => "every",
            TokenKind::KwExport => "export",
            TokenKind::KwExtern => "extern",
            TokenKind::KwF32 => "f32",
            TokenKind::KwF64 => "f64",
            TokenKind::KwFalse => "false",
            TokenKind::KwFeature => "feature",
            TokenKind::KwFinal => "final",
            TokenKind::KwFork => "fork",
            TokenKind::KwI8 => "i8",
            TokenKind::KwI16 => "i16",
            TokenKind::KwI32 => "i32",
            TokenKind::KwI64 => "i64",
            TokenKind::KwImport => "import",
            TokenKind::KwInitial => "initial",
            TokenKind::KwIs => "is",
            TokenKind::KwJoin => "join",
            TokenKind::KwJunction => "junction",
            TokenKind::KwLanguage => "language",
            TokenKind::KwMachine => "machine",
            TokenKind::KwMs => "ms",
            TokenKind::KwOn => "on",
            TokenKind::KwOpaque => "opaque",
            TokenKind::KwParallel => "parallel",
            TokenKind::KwPriority => "priority",
            TokenKind::KwPure => "pure",
            TokenKind::KwRaise => "raise",
            TokenKind::KwRegion => "region",
            TokenKind::KwSchedule => "schedule",
            TokenKind::KwSend => "send",
            TokenKind::KwShallowHistory => "shallow_history",
            TokenKind::KwState => "state",
            TokenKind::KwSubmachine => "submachine",
            TokenKind::KwTarget => "target",
            TokenKind::KwTo => "to",
            TokenKind::KwTrue => "true",
            TokenKind::KwU8 => "u8",
            TokenKind::KwU16 => "u16",
            TokenKind::KwU32 => "u32",
            TokenKind::KwU64 => "u64",

            // Operators and delimiters — source spelling.
            TokenKind::Arrow => "->",
            TokenKind::HistoryArrow => "~>",
            TokenKind::Colon => ":",
            TokenKind::Semicolon => ";",
            TokenKind::Comma => ",",
            TokenKind::Dot => ".",
            TokenKind::Eq => "=",
            TokenKind::At => "@",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            TokenKind::EqEq => "==",
            TokenKind::BangEq => "!=",
            TokenKind::Lt => "<",
            TokenKind::Gt => ">",
            TokenKind::Le => "<=",
            TokenKind::Ge => ">=",
            TokenKind::AmpAmp => "&&",
            TokenKind::PipePipe => "||",
            TokenKind::Bang => "!",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::Amp => "&",
            TokenKind::Pipe => "|",
            TokenKind::Caret => "^",
            TokenKind::Tilde => "~",
            TokenKind::Shl => "<<",
            TokenKind::Shr => ">>",

            // Open-ended categories.
            TokenKind::IntLiteral => "integer literal",
            TokenKind::FloatLiteral => "float literal",
            TokenKind::StringLiteral => "string literal",
            TokenKind::Ident => "identifier",
            TokenKind::StableId => "stable id",
            TokenKind::Whitespace => "whitespace",
            TokenKind::Newline => "newline",
            TokenKind::LineComment => "line comment",
            TokenKind::BlockComment => "block comment",
            TokenKind::DocComment => "doc comment",

            TokenKind::Error(_) => "error",
            TokenKind::Eof => "end of file",
        };
        f.write_str(s)
    }
}

/// Reverse lookup `identifier slice → keyword TokenKind`. Returns `None` when
/// the slice is not one of the 53 reserved words in Doc 04 §1.5. The lexer
/// uses this to "promote" an identifier run to a keyword variant.
///
/// Implemented as a `match` over byte length first, then over the slice value:
/// the Rust compiler is able to lower this to a jump table over short string
/// keys, and it avoids any allocation or hashing per token (matches the
/// zero-copy contract).
pub(crate) fn keyword_kind(ident: &str) -> Option<TokenKind> {
    Some(match ident {
        "after" => TokenKind::KwAfter,
        "as" => TokenKind::KwAs,
        "bool" => TokenKind::KwBool,
        "cancel" => TokenKind::KwCancel,
        "choice" => TokenKind::KwChoice,
        "composite" => TokenKind::KwComposite,
        "const" => TokenKind::KwConst,
        "context" => TokenKind::KwContext,
        "deep_history" => TokenKind::KwDeepHistory,
        "defer" => TokenKind::KwDefer,
        "done" => TokenKind::KwDone,
        "else" => TokenKind::KwElse,
        "enum" => TokenKind::KwEnum,
        "every" => TokenKind::KwEvery,
        "export" => TokenKind::KwExport,
        "extern" => TokenKind::KwExtern,
        "f32" => TokenKind::KwF32,
        "f64" => TokenKind::KwF64,
        "false" => TokenKind::KwFalse,
        "feature" => TokenKind::KwFeature,
        "final" => TokenKind::KwFinal,
        "fork" => TokenKind::KwFork,
        "i8" => TokenKind::KwI8,
        "i16" => TokenKind::KwI16,
        "i32" => TokenKind::KwI32,
        "i64" => TokenKind::KwI64,
        "import" => TokenKind::KwImport,
        "initial" => TokenKind::KwInitial,
        "is" => TokenKind::KwIs,
        "join" => TokenKind::KwJoin,
        "junction" => TokenKind::KwJunction,
        "language" => TokenKind::KwLanguage,
        "machine" => TokenKind::KwMachine,
        "ms" => TokenKind::KwMs,
        "on" => TokenKind::KwOn,
        "opaque" => TokenKind::KwOpaque,
        "parallel" => TokenKind::KwParallel,
        "priority" => TokenKind::KwPriority,
        "pure" => TokenKind::KwPure,
        "raise" => TokenKind::KwRaise,
        "region" => TokenKind::KwRegion,
        "schedule" => TokenKind::KwSchedule,
        "send" => TokenKind::KwSend,
        "shallow_history" => TokenKind::KwShallowHistory,
        "state" => TokenKind::KwState,
        "submachine" => TokenKind::KwSubmachine,
        "target" => TokenKind::KwTarget,
        "to" => TokenKind::KwTo,
        "true" => TokenKind::KwTrue,
        "u8" => TokenKind::KwU8,
        "u16" => TokenKind::KwU16,
        "u32" => TokenKind::KwU32,
        "u64" => TokenKind::KwU64,
        _ => return None,
    })
}
