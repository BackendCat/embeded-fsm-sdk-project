//! Typed AST accessors for transition / guard / priority / action-block.

use super::{ast_node, children, first_ident, AstChildren, AstNode};

ast_node!(TransitionDecl, TRANSITION_DECL);
ast_node!(InternalDecl, INTERNAL_DECL);
ast_node!(LocalDecl, LOCAL_DECL);
ast_node!(CompletionDecl, COMPLETION_DECL);
ast_node!(GuardClause, GUARD_CLAUSE);
ast_node!(PriorityClause, PRIORITY_CLAUSE);
ast_node!(ActionBlock, ACTION_BLOCK);

/// Common interface for the four transition-flavour nodes — they all carry
/// optional guard / priority / action-block children but differ in trigger
/// semantics. Accessors are duplicated rather than abstracted with a trait
/// because the access patterns are small (4 nodes × 4 accessors) and a
/// trait would obscure rather than help.

impl TransitionDecl {
    /// Trigger event identifier, e.g. `START` in `on START -> Running`.
    pub fn trigger(&self) -> Option<String> {
        first_ident(&self.0)
    }
    /// Target state identifier (after `->`).
    pub fn target(&self) -> Option<String> {
        // Two `Ident` tokens under a TransitionDecl: trigger then target.
        nth_ident(&self.0, 1)
    }
    pub fn guard(&self) -> Option<GuardClause> {
        super::child(&self.0)
    }
    pub fn priority(&self) -> Option<PriorityClause> {
        super::child(&self.0)
    }
    pub fn actions(&self) -> Option<ActionBlock> {
        super::child(&self.0)
    }
}

impl InternalDecl {
    pub fn trigger(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn guard(&self) -> Option<GuardClause> {
        super::child(&self.0)
    }
    pub fn priority(&self) -> Option<PriorityClause> {
        super::child(&self.0)
    }
    pub fn actions(&self) -> Option<ActionBlock> {
        super::child(&self.0)
    }
}

impl LocalDecl {
    pub fn trigger(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn target(&self) -> Option<String> {
        nth_ident(&self.0, 1)
    }
    pub fn guard(&self) -> Option<GuardClause> {
        super::child(&self.0)
    }
    pub fn priority(&self) -> Option<PriorityClause> {
        super::child(&self.0)
    }
    pub fn actions(&self) -> Option<ActionBlock> {
        super::child(&self.0)
    }
}

impl CompletionDecl {
    pub fn target(&self) -> Option<String> {
        // `done [g] -> Target` — first ident is the target.
        first_ident(&self.0)
    }
    pub fn guard(&self) -> Option<GuardClause> {
        super::child(&self.0)
    }
    pub fn priority(&self) -> Option<PriorityClause> {
        super::child(&self.0)
    }
    pub fn actions(&self) -> Option<ActionBlock> {
        super::child(&self.0)
    }
}

impl GuardClause {
    /// The single expression inside `[ … ]`. May be `None` on empty/error
    /// recovery (e.g. `[]`).
    pub fn expr(&self) -> Option<crate::ast::Expr> {
        // The expression is the first child node (skipping the bracket
        // tokens which are leaf tokens, not nodes).
        self.0.children().find_map(crate::ast::Expr::cast)
    }
}

impl ActionBlock {
    pub fn statements(&self) -> AstChildren<crate::ast::Stmt> {
        children(&self.0)
    }
}

fn nth_ident(parent: &crate::cst::SyntaxNode, n: usize) -> Option<String> {
    let mut count = 0;
    for el in parent.children_with_tokens() {
        if let Some(t) = el.as_token() {
            if t.kind() == crate::cst::SyntaxKind::Ident {
                if count == n {
                    return Some(t.text().to_string());
                }
                count += 1;
            }
        }
    }
    None
}
