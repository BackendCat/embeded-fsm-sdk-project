//! Lossless CST built on top of the [`rowan`] green-tree library.
//!
//! Per Doc 20 §4.1, the CST is the parser's primary output: it preserves
//! every token including trivia (whitespace, comments) so the formatter and
//! the LSP incremental-reparse pipeline can reconstruct the source byte for
//! byte.
//!
//! Type aliases provided here let downstream crates spell out concrete
//! `SyntaxNode` / `SyntaxToken` types without re-deriving the `Language`
//! bound every time.

pub mod kinds;

pub use kinds::{syntax_kind_from_token, SyntaxKind};

pub use rowan::{GreenNode, GreenNodeBuilder, GreenToken};

/// The `rowan::Language` implementation for FSM-Lang. A zero-sized witness;
/// instances are never constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FsmLanguage {}

impl rowan::Language for FsmLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        // Trees produced by this crate always carry valid kinds. Trees
        // observed elsewhere must come through this function — if a caller
        // hands us a hand-crafted invalid raw kind they would have to use
        // unsafe to do so. Falling back to `ERROR_NODE` is the safe choice
        // and keeps the conversion total, as `rowan::Language` requires.
        SyntaxKind::from_raw(raw).unwrap_or(SyntaxKind::ERROR_NODE)
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into_raw()
    }
}

/// Strongly typed alias for a CST node.
pub type SyntaxNode = rowan::SyntaxNode<FsmLanguage>;
/// Strongly typed alias for a CST token.
pub type SyntaxToken = rowan::SyntaxToken<FsmLanguage>;
/// Strongly typed alias for either a node or a token.
pub type SyntaxElement = rowan::SyntaxElement<FsmLanguage>;

#[cfg(test)]
mod tests {
    use super::*;
    use rowan::Language;

    #[test]
    fn language_roundtrip() {
        let raw = FsmLanguage::kind_to_raw(SyntaxKind::MACHINE_DECL);
        assert_eq!(FsmLanguage::kind_from_raw(raw), SyntaxKind::MACHINE_DECL);
    }

    #[test]
    fn invalid_raw_falls_back_to_error_node() {
        let raw = rowan::SyntaxKind(u16::MAX);
        assert_eq!(FsmLanguage::kind_from_raw(raw), SyntaxKind::ERROR_NODE);
    }
}
