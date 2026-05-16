//! Lossless CST built on top of the [`rowan`] green-tree library.
//!
//! Per Doc 20 §4.1, the CST is the parser's primary output: it preserves
//! every token including trivia (whitespace, comments) so a consumer can
//! reconstruct the source byte for byte.
//!
//! **Intended consumers of the raw CST view (verified at HEAD, W0 /
//! Doc 29 §3.4 / AUDIT_B P1-A1):**
//! - **`fsm-lsp`** — incremental-reparse + positional capabilities
//!   (hover/definition/rename/semantic-tokens/…). This is the legitimate
//!   primary `fsm_parser::cst::*`-path consumer (12 modules).
//! - **`fsm-formatter`** does **not** consume this `cst::` module path: it
//!   reconstructs source byte-for-byte via the top-level
//!   `fsm_parser::{SyntaxNode, SyntaxToken}` re-exports (the lossless tree),
//!   not the `cst::` path. (The earlier "for the formatter and LSP"
//!   framing in AUDIT_B:115 / Doc 00 §11.49 was stale — a W0
//!   verify-the-record correction.)
//! - **`fsm-analyzer`** consumes the typed [`crate::ast`] view and uses
//!   `cst` only for the documented W0 / Doc 29 §3.4 leave-and-explain
//!   residuals (R-1 OPAQUE-BUG-1 parent-node type resolution, R-2
//!   document-order/heterogeneous-child dispatch, R-3 the deliberately
//!   shallow expression/statement sublanguage, R-4 `util::span_of`-family
//!   positional helpers) — never for routine traversal, which goes through
//!   `ast::*` typed accessors.
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
