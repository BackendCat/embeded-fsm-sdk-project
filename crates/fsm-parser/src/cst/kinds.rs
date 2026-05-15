//! `SyntaxKind` — the union of every token (lexer-level) and every grammar
//! node (parser-level) tag stored in the rowan green tree.
//!
//! Per Doc 20 §4.2 and rust-analyzer convention, every variant has a stable
//! `u16` discriminant so the kind fits in `rowan::SyntaxKind(pub u16)`. The
//! discriminants are dense (`#[repr(u16)]` with default ordering) so an
//! `as u16`/`from_u16` round-trip is cheap.
//!
//! Layout:
//! - `0x00xx` — sentinel + lexer tokens.
//! - `0x01xx` — grammar nodes (one per rule).
//!
//! Per Doc 00 §B-05 and §7.3, expressions use Pratt parsing; the expression
//! grammar nodes (`EXPR_*`) reflect the precedence-table-friendly rewrite
//! from §8.7.1 rather than the left-recursive EBNF in §8.7.

#![allow(non_camel_case_types)]

use fsm_lexer::TokenKind;

/// Tag stored in the rowan green tree. One variant per lexer token, one per
/// grammar production. The numeric values are stable: they are persisted via
/// `rowan::SyntaxKind(u16)` and surfaced through [`crate::cst::FsmLanguage`].
///
/// New variants append. Reordering or removing variants is a breaking change.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // ─── Tokens (must come first; one per `TokenKind`) ───────────────────
    // Order MUST match `TokenKind` so the `From<TokenKind>` conversion is a
    // straightforward `match` over named variants — but the numeric values
    // themselves are independent of `TokenKind`'s internal layout.
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
    // Contextual keywords that the lexer emits as `Ident` but the parser
    // distinguishes by position: `entry`, `exit`, `events`, `queue`,
    // `if`, `while`, `for`, `entry_point`, `exit_point`, `fsm`, and
    // (v1.1-W4) the transition-prefix hints `likely` / `rare`. They have no
    // dedicated TokenKind, so they round-trip as `Ident` in the CST.
    // Keeping `likely`/`rare` contextual (vs. reserved `Kw*`) is a
    // deliberate back-compat choice: an existing `.fsm` that uses `likely`
    // or `rare` as a state / event / field / extern name keeps parsing,
    // because they are only special in transition-prefix position (see
    // grammar/state.rs `parse_state_item` + grammar/transition.rs).

    // Operators / delimiters.
    Arrow,        // ->
    HistoryArrow, // ~>
    Colon,
    Semicolon,
    Comma,
    Dot,
    Eq, // =
    At,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,

    EqEq,   // ==
    BangEq, // !=
    Lt,
    Gt,
    Le,
    Ge,
    AmpAmp,   // &&
    PipePipe, // ||
    Bang,     // !

    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    Amp,
    Pipe,
    Caret,
    Tilde,
    Shl, // <<
    Shr, // >>

    // Literals.
    IntLiteral,
    FloatLiteral,
    StringLiteral,

    // Names + trivia.
    Ident,
    StableId,
    Whitespace,
    Newline,
    LineComment,
    BlockComment,
    DocComment,

    // Lexer-error / sentinel tokens.
    Error,
    Eof,

    // ─── Grammar nodes (one per production) ──────────────────────────────
    /// Root node covering the whole file.
    FILE,
    LANGUAGE_DECL,
    LANGUAGE_VERSION,
    IMPORT_DECL,
    IMPORT_ALIAS,
    IMPORT_LIST,
    IMPORT_ITEM,
    FEATURE_DECL,
    CONST_DECL,
    /// Wrapping the right-hand side of a const declaration; a single
    /// `ConstExprNode` holds whatever expression form was parsed (literal,
    /// identifier, or a Pratt expression). Const evaluation is the analyzer's
    /// job.
    CONST_EXPR,
    ENUM_DECL,
    ENUM_VARIANT,
    EXTERN_DECL,
    PARAM_LIST,
    PARAM,
    MACHINE_DECL,
    /// `submachine ID { <machine-body> }`. A submachine is structurally a
    /// named, referenceable machine template (Doc 04 §15). Its body reuses
    /// the `machine_item` production, so the AST reuses the machine
    /// accessors. A distinct kind (vs. reusing MACHINE_DECL) is required so
    /// `File::submachines()` and `File::machines()` are disjoint and the
    /// analyzer can tell a template apart from an instantiable machine.
    SUBMACHINE_DECL,
    /// `is SubName` reference inside a `STATE_DECL`. Modelled as an optional
    /// child node of STATE_DECL (mirroring how GUARD_CLAUSE / FORK_TARGETS
    /// are optional children) rather than forking a second state kind, so
    /// every existing `StateDecl` accessor keeps working unchanged. Wraps
    /// the `is` keyword + the referenced submachine name ident.
    SUBMACHINE_REF,
    CONTEXT_BLOCK,
    FIELD_DECL,
    EVENTS_BLOCK,
    EVENT_DECL,
    PAYLOAD_LIST,
    PAYLOAD_FIELD,
    QUEUE_BLOCK,
    TARGET_BLOCK,
    CONFIG_ENTRY,
    INITIAL_DECL,
    /// `state ID { ... }`. The parser does not distinguish composite vs.
    /// simple at this stage — that is a semantic property derived from
    /// whether any nested `STATE_DECL` or `REGION_DECL` children exist. This
    /// matches the EBNF (§5).
    STATE_DECL,
    /// `entry : action_list`
    ENTRY_DECL,
    /// `exit : action_list`
    EXIT_DECL,
    /// `final IDENT`
    FINAL_DECL,
    ENTRY_POINT_DECL,
    EXIT_POINT_DECL,
    REGION_DECL,
    SHALLOW_HISTORY_DECL,
    DEEP_HISTORY_DECL,
    CHOICE_DECL,
    CHOICE_BRANCH,
    JUNCTION_DECL,
    JUNCTION_BRANCH,
    FORK_DECL,
    FORK_TARGETS,
    JOIN_DECL,
    JOIN_SOURCES,
    /// External transition `on EV [g] -> T : action`. Local (`~>`) and
    /// internal (no `->`) transitions are different kinds (LOCAL_DECL,
    /// INTERNAL_DECL) so the analyzer can distinguish them without re-
    /// inspecting the operator token. (Per Doc 00 §B-06 the IR also
    /// distinguishes — keep the CST tags equally rich.)
    TRANSITION_DECL,
    INTERNAL_DECL,
    LOCAL_DECL,
    /// `done [g] -> T : action` — completion transition. Guards permitted
    /// per Doc 00 §B-07.
    COMPLETION_DECL,
    AFTER_DECL,
    EVERY_DECL,
    EVERY_INTERNAL_DECL,
    DEFER_DECL,
    GUARD_CLAUSE,
    PRIORITY_CLAUSE,
    ACTION_BLOCK,
    /// `stable_id` annotation `@id("…")` attached to a declaration.
    STABLE_ID_ANNOT,

    // Statements (action sublanguage).
    STMT_ASSIGN,
    STMT_IF,
    STMT_ELSE,
    STMT_WHILE,
    STMT_FOR,
    STMT_CALL,
    STMT_RAISE,
    STMT_SEND,
    STMT_DEFER,

    // Expressions — one per Pratt level. The level a node sits at follows
    // Doc 04 §8.7.1.
    EXPR_BINARY,
    EXPR_UNARY,
    EXPR_CAST,
    EXPR_CALL,
    EXPR_FIELD_REF,
    EXPR_LITERAL,
    EXPR_NAME_REF,
    /// `EnumType.Variant` qualified name in expression position.
    EXPR_QUALIFIED_NAME,
    EXPR_PAREN,
    /// Argument list of a call expression or call statement.
    ARG_LIST,
    /// `else` in a choice-branch guard (`["else"]`). Modelled as a
    /// distinguished node so the analyzer can quickly find the catch-all.
    GUARD_ELSE,

    // Type references.
    TYPE_REF,
    OPAQUE_TYPE_REF,

    /// Sentinel for "the parser saw garbage here". The byte range is still
    /// preserved (rowan round-trip property) but the surrounding analyzer
    /// can use this as a "skip subtree" hint.
    ERROR_NODE,

    /// v1.1-W4: optional `likely` / `rare` prefix on a transition. Modelled
    /// as an optional child node of the transition node (mirroring how
    /// GUARD_CLAUSE / SUBMACHINE_REF are optional children) rather than a
    /// bare token, so the AST exposes a typed `branch_hint()` accessor and
    /// the rowan round-trip preserves the prefix verbatim. Wraps the single
    /// `likely`/`rare` ident token. Appended last (before `__LAST`) so all
    /// existing discriminants are unchanged (rowan persistence stability).
    BRANCH_HINT,

    /// MUST be the last variant. Counts variants for compile-time bound.
    __LAST,
}

impl SyntaxKind {
    /// `true` for the lexer-trivia kinds (whitespace, newline, all three
    /// comment variants). Used by the parser to filter trivia from the
    /// AST view while still inserting it into the CST.
    pub const fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::Whitespace
                | SyntaxKind::Newline
                | SyntaxKind::LineComment
                | SyntaxKind::BlockComment
                | SyntaxKind::DocComment
        )
    }

    /// `true` for kinds the parser treats as tokens (not as composite nodes).
    /// Used when classifying a `SyntaxElement`.
    pub const fn is_token(self) -> bool {
        (self as u16) <= (SyntaxKind::Eof as u16)
    }

    /// Pack into the raw `rowan::SyntaxKind` for storage in the green tree.
    #[inline]
    pub fn into_raw(self) -> rowan::SyntaxKind {
        rowan::SyntaxKind(self as u16)
    }

    /// Reverse of `into_raw`. Returns `None` if the raw value is outside the
    /// enum range — this should never happen for trees produced by this
    /// crate.
    pub fn from_raw(raw: rowan::SyntaxKind) -> Option<SyntaxKind> {
        if raw.0 < SyntaxKind::__LAST as u16 {
            // Safety: `SyntaxKind` is `#[repr(u16)]` with dense variants
            // from 0 to `__LAST - 1`. The check above guarantees `raw.0`
            // lies in that range.
            // Using a transmute would be tempting but `unsafe` is forbidden
            // crate-wide; the explicit match below is equally fast in
            // release builds and remains safe.
            Some(from_raw_safe(raw.0))
        } else {
            None
        }
    }
}

/// Safe `u16 → SyntaxKind` conversion. The compiler turns the exhaustive
/// match into a jump table; performance matches a transmute.
fn from_raw_safe(n: u16) -> SyntaxKind {
    // Build a small lookup table at compile time. The table indexes every
    // possible variant ordinal. We use a `const` array so the data lives in
    // `.rodata`.
    const TABLE: &[SyntaxKind] = &all_kinds();
    debug_assert!((n as usize) < TABLE.len(), "from_raw_safe out of bounds");
    TABLE[n as usize]
}

/// Materialise every variant in declaration order. Used by `from_raw_safe`
/// to build a lookup table.
const fn all_kinds() -> [SyntaxKind; SyntaxKind::__LAST as usize] {
    use SyntaxKind::*;
    // Listing the variants explicitly (rather than using a derive macro
    // crate) keeps build times low and avoids a third-party dep.
    [
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
        Arrow,
        HistoryArrow,
        Colon,
        Semicolon,
        Comma,
        Dot,
        Eq,
        At,
        LBrace,
        RBrace,
        LParen,
        RParen,
        LBracket,
        RBracket,
        EqEq,
        BangEq,
        Lt,
        Gt,
        Le,
        Ge,
        AmpAmp,
        PipePipe,
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
        Shl,
        Shr,
        IntLiteral,
        FloatLiteral,
        StringLiteral,
        Ident,
        StableId,
        Whitespace,
        Newline,
        LineComment,
        BlockComment,
        DocComment,
        Error,
        Eof,
        FILE,
        LANGUAGE_DECL,
        LANGUAGE_VERSION,
        IMPORT_DECL,
        IMPORT_ALIAS,
        IMPORT_LIST,
        IMPORT_ITEM,
        FEATURE_DECL,
        CONST_DECL,
        CONST_EXPR,
        ENUM_DECL,
        ENUM_VARIANT,
        EXTERN_DECL,
        PARAM_LIST,
        PARAM,
        MACHINE_DECL,
        SUBMACHINE_DECL,
        SUBMACHINE_REF,
        CONTEXT_BLOCK,
        FIELD_DECL,
        EVENTS_BLOCK,
        EVENT_DECL,
        PAYLOAD_LIST,
        PAYLOAD_FIELD,
        QUEUE_BLOCK,
        TARGET_BLOCK,
        CONFIG_ENTRY,
        INITIAL_DECL,
        STATE_DECL,
        ENTRY_DECL,
        EXIT_DECL,
        FINAL_DECL,
        ENTRY_POINT_DECL,
        EXIT_POINT_DECL,
        REGION_DECL,
        SHALLOW_HISTORY_DECL,
        DEEP_HISTORY_DECL,
        CHOICE_DECL,
        CHOICE_BRANCH,
        JUNCTION_DECL,
        JUNCTION_BRANCH,
        FORK_DECL,
        FORK_TARGETS,
        JOIN_DECL,
        JOIN_SOURCES,
        TRANSITION_DECL,
        INTERNAL_DECL,
        LOCAL_DECL,
        COMPLETION_DECL,
        AFTER_DECL,
        EVERY_DECL,
        EVERY_INTERNAL_DECL,
        DEFER_DECL,
        GUARD_CLAUSE,
        PRIORITY_CLAUSE,
        ACTION_BLOCK,
        STABLE_ID_ANNOT,
        STMT_ASSIGN,
        STMT_IF,
        STMT_ELSE,
        STMT_WHILE,
        STMT_FOR,
        STMT_CALL,
        STMT_RAISE,
        STMT_SEND,
        STMT_DEFER,
        EXPR_BINARY,
        EXPR_UNARY,
        EXPR_CAST,
        EXPR_CALL,
        EXPR_FIELD_REF,
        EXPR_LITERAL,
        EXPR_NAME_REF,
        EXPR_QUALIFIED_NAME,
        EXPR_PAREN,
        ARG_LIST,
        GUARD_ELSE,
        TYPE_REF,
        OPAQUE_TYPE_REF,
        ERROR_NODE,
        BRANCH_HINT,
        // __LAST not in table (we only need indexes 0..__LAST).
    ]
}

/// Map a lexer `TokenKind` to the corresponding `SyntaxKind`. Total — every
/// `TokenKind` has a dedicated `SyntaxKind`. `TokenKind::Error(_)` collapses
/// to `SyntaxKind::Error`; the diagnostic code on the error token is handled
/// by the parser's recovery layer, not by the CST.
pub fn syntax_kind_from_token(kind: TokenKind) -> SyntaxKind {
    use SyntaxKind as S;
    match kind {
        TokenKind::KwAfter => S::KwAfter,
        TokenKind::KwAs => S::KwAs,
        TokenKind::KwBool => S::KwBool,
        TokenKind::KwCancel => S::KwCancel,
        TokenKind::KwChoice => S::KwChoice,
        TokenKind::KwComposite => S::KwComposite,
        TokenKind::KwConst => S::KwConst,
        TokenKind::KwContext => S::KwContext,
        TokenKind::KwDeepHistory => S::KwDeepHistory,
        TokenKind::KwDefer => S::KwDefer,
        TokenKind::KwDone => S::KwDone,
        TokenKind::KwElse => S::KwElse,
        TokenKind::KwEnum => S::KwEnum,
        TokenKind::KwEvery => S::KwEvery,
        TokenKind::KwExport => S::KwExport,
        TokenKind::KwExtern => S::KwExtern,
        TokenKind::KwF32 => S::KwF32,
        TokenKind::KwF64 => S::KwF64,
        TokenKind::KwFalse => S::KwFalse,
        TokenKind::KwFeature => S::KwFeature,
        TokenKind::KwFinal => S::KwFinal,
        TokenKind::KwFork => S::KwFork,
        TokenKind::KwI8 => S::KwI8,
        TokenKind::KwI16 => S::KwI16,
        TokenKind::KwI32 => S::KwI32,
        TokenKind::KwI64 => S::KwI64,
        TokenKind::KwImport => S::KwImport,
        TokenKind::KwInitial => S::KwInitial,
        TokenKind::KwIs => S::KwIs,
        TokenKind::KwJoin => S::KwJoin,
        TokenKind::KwJunction => S::KwJunction,
        TokenKind::KwLanguage => S::KwLanguage,
        TokenKind::KwMachine => S::KwMachine,
        TokenKind::KwMs => S::KwMs,
        TokenKind::KwOn => S::KwOn,
        TokenKind::KwOpaque => S::KwOpaque,
        TokenKind::KwParallel => S::KwParallel,
        TokenKind::KwPriority => S::KwPriority,
        TokenKind::KwPure => S::KwPure,
        TokenKind::KwRaise => S::KwRaise,
        TokenKind::KwRegion => S::KwRegion,
        TokenKind::KwSchedule => S::KwSchedule,
        TokenKind::KwSend => S::KwSend,
        TokenKind::KwShallowHistory => S::KwShallowHistory,
        TokenKind::KwState => S::KwState,
        TokenKind::KwSubmachine => S::KwSubmachine,
        TokenKind::KwTarget => S::KwTarget,
        TokenKind::KwTo => S::KwTo,
        TokenKind::KwTrue => S::KwTrue,
        TokenKind::KwU8 => S::KwU8,
        TokenKind::KwU16 => S::KwU16,
        TokenKind::KwU32 => S::KwU32,
        TokenKind::KwU64 => S::KwU64,
        TokenKind::Arrow => S::Arrow,
        TokenKind::HistoryArrow => S::HistoryArrow,
        TokenKind::Colon => S::Colon,
        TokenKind::Semicolon => S::Semicolon,
        TokenKind::Comma => S::Comma,
        TokenKind::Dot => S::Dot,
        TokenKind::Eq => S::Eq,
        TokenKind::At => S::At,
        TokenKind::LBrace => S::LBrace,
        TokenKind::RBrace => S::RBrace,
        TokenKind::LParen => S::LParen,
        TokenKind::RParen => S::RParen,
        TokenKind::LBracket => S::LBracket,
        TokenKind::RBracket => S::RBracket,
        TokenKind::EqEq => S::EqEq,
        TokenKind::BangEq => S::BangEq,
        TokenKind::Lt => S::Lt,
        TokenKind::Gt => S::Gt,
        TokenKind::Le => S::Le,
        TokenKind::Ge => S::Ge,
        TokenKind::AmpAmp => S::AmpAmp,
        TokenKind::PipePipe => S::PipePipe,
        TokenKind::Bang => S::Bang,
        TokenKind::Plus => S::Plus,
        TokenKind::Minus => S::Minus,
        TokenKind::Star => S::Star,
        TokenKind::Slash => S::Slash,
        TokenKind::Percent => S::Percent,
        TokenKind::Amp => S::Amp,
        TokenKind::Pipe => S::Pipe,
        TokenKind::Caret => S::Caret,
        TokenKind::Tilde => S::Tilde,
        TokenKind::Shl => S::Shl,
        TokenKind::Shr => S::Shr,
        TokenKind::IntLiteral => S::IntLiteral,
        TokenKind::FloatLiteral => S::FloatLiteral,
        TokenKind::StringLiteral => S::StringLiteral,
        TokenKind::Ident => S::Ident,
        TokenKind::StableId => S::StableId,
        TokenKind::Whitespace => S::Whitespace,
        TokenKind::Newline => S::Newline,
        TokenKind::LineComment => S::LineComment,
        TokenKind::BlockComment => S::BlockComment,
        TokenKind::DocComment => S::DocComment,
        TokenKind::Error(_) => S::Error,
        TokenKind::Eof => S::Eof,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_token_kinds() {
        // Every lexer token kind maps to a syntax kind whose discriminant is
        // valid (below `__LAST`) and that lies in the "is_token" range.
        for tk in [
            TokenKind::KwMachine,
            TokenKind::Arrow,
            TokenKind::HistoryArrow,
            TokenKind::IntLiteral,
            TokenKind::Ident,
            TokenKind::Eof,
        ] {
            let sk = syntax_kind_from_token(tk);
            assert!(sk.is_token(), "{tk:?} -> {sk:?} should be a token kind");
            let raw = sk.into_raw();
            assert_eq!(SyntaxKind::from_raw(raw), Some(sk));
        }
    }

    #[test]
    fn nodes_round_trip() {
        for sk in [
            SyntaxKind::FILE,
            SyntaxKind::MACHINE_DECL,
            SyntaxKind::TRANSITION_DECL,
            SyntaxKind::EXPR_BINARY,
            SyntaxKind::ERROR_NODE,
        ] {
            assert!(!sk.is_token());
            assert_eq!(SyntaxKind::from_raw(sk.into_raw()), Some(sk));
        }
    }

    #[test]
    fn trivia_kinds_classified() {
        assert!(SyntaxKind::Whitespace.is_trivia());
        assert!(SyntaxKind::LineComment.is_trivia());
        assert!(SyntaxKind::DocComment.is_trivia());
        assert!(!SyntaxKind::Ident.is_trivia());
        assert!(!SyntaxKind::MACHINE_DECL.is_trivia());
    }

    #[test]
    fn from_raw_out_of_bounds_is_none() {
        assert!(SyntaxKind::from_raw(rowan::SyntaxKind(u16::MAX)).is_none());
    }
}
