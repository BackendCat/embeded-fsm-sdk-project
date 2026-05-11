//! A 128-bit bitmask over `TokenKind` discriminants. Used by the parser to
//! make `at_any_of(set)` checks branch-free, and to drive panic-mode
//! recovery: a sync-set is just a `TokenSet` of "land here when lost."
//!
//! The set is `Copy` and entirely `const`-constructible so common sets can
//! be defined as `const FOO: TokenSet = TokenSet::new(&[…])` next to the
//! grammar rules that use them.

use fsm_lexer::TokenKind;

/// 128 bits suffice today: the [`TokenKind`] enum has roughly 100 variants
/// (53 keywords + ~30 operators + literals + trivia + sentinels). If a new
/// variant pushes the count past 128 we widen this to `[u64; 4]`.
const SLOTS: usize = 2;

/// Bit set over `TokenKind` ordinals.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TokenSet([u64; SLOTS]);

impl TokenSet {
    /// Empty set.
    pub const EMPTY: TokenSet = TokenSet([0; SLOTS]);

    /// Build a set from a slice of kinds. Available in `const` context.
    pub const fn new(kinds: &[TokenKind]) -> TokenSet {
        let mut slots = [0u64; SLOTS];
        let mut i = 0;
        while i < kinds.len() {
            let ord = token_ord(kinds[i]);
            let slot = ord / 64;
            let bit = ord % 64;
            slots[slot] |= 1u64 << bit;
            i += 1;
        }
        TokenSet(slots)
    }

    /// Set union — useful for building per-context sync sets composed from
    /// smaller pieces (e.g. `STATE_ITEM_STARTS | DECL_STARTS`).
    pub const fn union(self, other: TokenSet) -> TokenSet {
        TokenSet([self.0[0] | other.0[0], self.0[1] | other.0[1]])
    }

    /// Set difference.
    pub const fn minus(self, other: TokenSet) -> TokenSet {
        TokenSet([self.0[0] & !other.0[0], self.0[1] & !other.0[1]])
    }

    /// Membership test.
    pub const fn contains(self, kind: TokenKind) -> bool {
        let ord = token_ord(kind);
        let slot = ord / 64;
        let bit = ord % 64;
        (self.0[slot] >> bit) & 1 == 1
    }
}

/// Ordinal of a `TokenKind` for indexing into the bitmask.
///
/// We do not depend on `as usize` because `TokenKind::Error(DiagnosticCode)`
/// has a payload — Rust does not let us cast such enums directly. The
/// explicit `match` keeps the function `const` and also gives a single place
/// to verify that no two variants share an ordinal.
///
/// **MUST** be kept in sync with `TokenKind`. New `TokenKind` variants
/// without a corresponding ordinal here will produce a `const`-time error
/// the moment they appear in a `TokenSet::new(&[…])` call site.
const fn token_ord(kind: TokenKind) -> usize {
    match kind {
        TokenKind::KwAfter => 0,
        TokenKind::KwAs => 1,
        TokenKind::KwBool => 2,
        TokenKind::KwCancel => 3,
        TokenKind::KwChoice => 4,
        TokenKind::KwComposite => 5,
        TokenKind::KwConst => 6,
        TokenKind::KwContext => 7,
        TokenKind::KwDeepHistory => 8,
        TokenKind::KwDefer => 9,
        TokenKind::KwDone => 10,
        TokenKind::KwElse => 11,
        TokenKind::KwEnum => 12,
        TokenKind::KwEvery => 13,
        TokenKind::KwExport => 14,
        TokenKind::KwExtern => 15,
        TokenKind::KwF32 => 16,
        TokenKind::KwF64 => 17,
        TokenKind::KwFalse => 18,
        TokenKind::KwFeature => 19,
        TokenKind::KwFinal => 20,
        TokenKind::KwFork => 21,
        TokenKind::KwI8 => 22,
        TokenKind::KwI16 => 23,
        TokenKind::KwI32 => 24,
        TokenKind::KwI64 => 25,
        TokenKind::KwImport => 26,
        TokenKind::KwInitial => 27,
        TokenKind::KwIs => 28,
        TokenKind::KwJoin => 29,
        TokenKind::KwJunction => 30,
        TokenKind::KwLanguage => 31,
        TokenKind::KwMachine => 32,
        TokenKind::KwMs => 33,
        TokenKind::KwOn => 34,
        TokenKind::KwOpaque => 35,
        TokenKind::KwParallel => 36,
        TokenKind::KwPriority => 37,
        TokenKind::KwPure => 38,
        TokenKind::KwRaise => 39,
        TokenKind::KwRegion => 40,
        TokenKind::KwSchedule => 41,
        TokenKind::KwSend => 42,
        TokenKind::KwShallowHistory => 43,
        TokenKind::KwState => 44,
        TokenKind::KwSubmachine => 45,
        TokenKind::KwTarget => 46,
        TokenKind::KwTo => 47,
        TokenKind::KwTrue => 48,
        TokenKind::KwU8 => 49,
        TokenKind::KwU16 => 50,
        TokenKind::KwU32 => 51,
        TokenKind::KwU64 => 52,
        TokenKind::Arrow => 53,
        TokenKind::HistoryArrow => 54,
        TokenKind::Colon => 55,
        TokenKind::Semicolon => 56,
        TokenKind::Comma => 57,
        TokenKind::Dot => 58,
        TokenKind::Eq => 59,
        TokenKind::At => 60,
        TokenKind::LBrace => 61,
        TokenKind::RBrace => 62,
        TokenKind::LParen => 63,
        TokenKind::RParen => 64,
        TokenKind::LBracket => 65,
        TokenKind::RBracket => 66,
        TokenKind::EqEq => 67,
        TokenKind::BangEq => 68,
        TokenKind::Lt => 69,
        TokenKind::Gt => 70,
        TokenKind::Le => 71,
        TokenKind::Ge => 72,
        TokenKind::AmpAmp => 73,
        TokenKind::PipePipe => 74,
        TokenKind::Bang => 75,
        TokenKind::Plus => 76,
        TokenKind::Minus => 77,
        TokenKind::Star => 78,
        TokenKind::Slash => 79,
        TokenKind::Percent => 80,
        TokenKind::Amp => 81,
        TokenKind::Pipe => 82,
        TokenKind::Caret => 83,
        TokenKind::Tilde => 84,
        TokenKind::Shl => 85,
        TokenKind::Shr => 86,
        TokenKind::IntLiteral => 87,
        TokenKind::FloatLiteral => 88,
        TokenKind::StringLiteral => 89,
        TokenKind::Ident => 90,
        TokenKind::StableId => 91,
        TokenKind::Whitespace => 92,
        TokenKind::Newline => 93,
        TokenKind::LineComment => 94,
        TokenKind::BlockComment => 95,
        TokenKind::DocComment => 96,
        TokenKind::Error(_) => 97,
        TokenKind::Eof => 98,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_contains_nothing() {
        let s = TokenSet::EMPTY;
        assert!(!s.contains(TokenKind::Eof));
        assert!(!s.contains(TokenKind::KwMachine));
    }

    #[test]
    fn set_construction_and_lookup() {
        const SET: TokenSet =
            TokenSet::new(&[TokenKind::KwMachine, TokenKind::KwState, TokenKind::Eof]);
        assert!(SET.contains(TokenKind::KwMachine));
        assert!(SET.contains(TokenKind::KwState));
        assert!(SET.contains(TokenKind::Eof));
        assert!(!SET.contains(TokenKind::KwImport));
    }

    #[test]
    fn union_and_minus() {
        const A: TokenSet = TokenSet::new(&[TokenKind::KwMachine]);
        const B: TokenSet = TokenSet::new(&[TokenKind::KwState]);
        let u = A.union(B);
        assert!(u.contains(TokenKind::KwMachine));
        assert!(u.contains(TokenKind::KwState));

        let m = u.minus(B);
        assert!(m.contains(TokenKind::KwMachine));
        assert!(!m.contains(TokenKind::KwState));
    }

    #[test]
    fn high_bits_in_second_slot() {
        // KwU64 has ordinal 52 — first slot; Eof has 98 — second slot.
        const SET: TokenSet = TokenSet::new(&[TokenKind::KwU64, TokenKind::Eof]);
        assert!(SET.contains(TokenKind::KwU64));
        assert!(SET.contains(TokenKind::Eof));
        assert!(!SET.contains(TokenKind::KwU32));
    }
}
