//! Typed AST view layered on top of the rowan CST.
//!
//! Each AST node is a `#[repr(transparent)]` wrapper around a [`SyntaxNode`]
//! whose `kind()` matches a specific [`SyntaxKind`]. Per the rust-analyzer
//! pattern, the wrappers cost nothing to construct and let callers query
//! fields without touching the green-tree API.
//!
//! Access patterns:
//! - **Iteration over typed children** via [`AstChildren`].
//! - **Optional fields** return `Option<T>` — the CST may be incomplete on
//!   parse error.
//! - **Trivia is hidden** — accessors filter out whitespace/comments.

use std::marker::PhantomData;

use crate::cst::{SyntaxKind, SyntaxNode, SyntaxToken};

pub mod expr;
pub mod state;
pub mod stmt;
pub mod top_level;
pub mod transition;

pub use expr::*;
pub use state::*;
pub use stmt::*;
pub use top_level::*;
pub use transition::*;

/// Trait implemented by every typed AST node. Mirrors the rust-analyzer
/// `AstNode` shape: `cast` converts a raw `SyntaxNode` to the typed view if
/// the kind matches; `syntax` recovers the underlying node.
pub trait AstNode: Sized {
    /// `true` if this AST type represents `kind`.
    fn can_cast(kind: SyntaxKind) -> bool;
    /// Attempt to view `node` as `Self`. Returns `None` if the kind mismatches.
    fn cast(node: SyntaxNode) -> Option<Self>;
    /// Recover the underlying CST node.
    fn syntax(&self) -> &SyntaxNode;
}

/// Typed iterator over the named-typed children of a CST node. Skips
/// trivia and any sibling that does not match `T::can_cast`.
#[derive(Debug, Clone)]
pub struct AstChildren<T> {
    inner: rowan::SyntaxNodeChildren<crate::cst::FsmLanguage>,
    _ty: PhantomData<T>,
}

impl<T: AstNode> AstChildren<T> {
    fn new(parent: &SyntaxNode) -> Self {
        Self {
            inner: parent.children(),
            _ty: PhantomData,
        }
    }
}

impl<T: AstNode> Iterator for AstChildren<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.by_ref().find_map(T::cast)
    }
}

/// Find the first typed child of `parent`.
pub fn child<T: AstNode>(parent: &SyntaxNode) -> Option<T> {
    parent.children().find_map(T::cast)
}

/// Iterator over all typed children.
pub fn children<T: AstNode>(parent: &SyntaxNode) -> AstChildren<T> {
    AstChildren::new(parent)
}

/// First child token of `kind` (filters trivia automatically by ignoring
/// trivia kinds).
pub fn child_token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    parent
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == kind)
}

/// First non-trivia identifier token under `parent`.
pub fn first_ident(parent: &SyntaxNode) -> Option<String> {
    parent
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
}

/// Debug helper — depth-first dump of the AST/CST shape for snapshot tests
/// and `--emit-cst` output.
pub fn ast_dump(root: &SyntaxNode) -> String {
    use std::fmt::Write as _;
    fn rec(n: &SyntaxNode, indent: usize, out: &mut String) {
        let _ = writeln!(out, "{:indent$}{:?}", "", n.kind(), indent = indent);
        for child in n.children_with_tokens() {
            match child {
                rowan::NodeOrToken::Node(child_node) => rec(&child_node, indent + 2, out),
                rowan::NodeOrToken::Token(t) => {
                    let k = t.kind();
                    if !k.is_trivia() {
                        let _ = writeln!(
                            out,
                            "{:indent$}{:?} {:?}",
                            "",
                            k,
                            t.text(),
                            indent = indent + 2
                        );
                    }
                }
            }
        }
    }
    let mut out = String::new();
    rec(root, 0, &mut out);
    out
}

// ─── Generic single-kind wrapper macro ───────────────────────────────────
//
// Each AST node type is just a thin newtype around `SyntaxNode`. The macro
// produces the boilerplate `Debug`/`AstNode` impls so the per-type modules
// can focus on accessors. The accessors themselves are hand-written because
// they vary too much to fit a one-pattern macro.

macro_rules! ast_node {
    ($name:ident, $kind:ident) => {
        #[derive(Clone, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(pub(crate) $crate::cst::SyntaxNode);

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.0.text_range())
                    .finish()
            }
        }

        impl $crate::ast::AstNode for $name {
            fn can_cast(kind: $crate::cst::SyntaxKind) -> bool {
                kind == $crate::cst::SyntaxKind::$kind
            }
            fn cast(node: $crate::cst::SyntaxNode) -> Option<Self> {
                if Self::can_cast(node.kind()) {
                    Some(Self(node))
                } else {
                    None
                }
            }
            fn syntax(&self) -> &$crate::cst::SyntaxNode {
                &self.0
            }
        }
    };
}
pub(crate) use ast_node;
