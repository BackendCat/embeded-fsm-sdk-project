//! Typed AST accessors for state declarations and pseudo-states.

use crate::cst::SyntaxNode;

use super::{ast_node, children, first_ident, AstChildren, AstNode};

ast_node!(StateDecl, STATE_DECL);
ast_node!(SubmachineRef, SUBMACHINE_REF);
ast_node!(EntryDecl, ENTRY_DECL);
ast_node!(ExitDecl, EXIT_DECL);
ast_node!(FinalDecl, FINAL_DECL);
ast_node!(EntryPointDecl, ENTRY_POINT_DECL);
ast_node!(ExitPointDecl, EXIT_POINT_DECL);
ast_node!(RegionDecl, REGION_DECL);
ast_node!(ShallowHistoryDecl, SHALLOW_HISTORY_DECL);
ast_node!(DeepHistoryDecl, DEEP_HISTORY_DECL);
ast_node!(ChoiceDecl, CHOICE_DECL);
ast_node!(ChoiceBranch, CHOICE_BRANCH);
ast_node!(JunctionDecl, JUNCTION_DECL);
ast_node!(JunctionBranch, JUNCTION_BRANCH);
ast_node!(ForkDecl, FORK_DECL);
ast_node!(ForkTargets, FORK_TARGETS);
ast_node!(JoinDecl, JOIN_DECL);
ast_node!(JoinSources, JOIN_SOURCES);
ast_node!(AfterDecl, AFTER_DECL);
ast_node!(EveryDecl, EVERY_DECL);
ast_node!(EveryInternalDecl, EVERY_INTERNAL_DECL);
ast_node!(DeferDecl, DEFER_DECL);

impl StateDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn nested_states(&self) -> AstChildren<StateDecl> {
        children(&self.0)
    }
    pub fn regions(&self) -> AstChildren<RegionDecl> {
        children(&self.0)
    }
    pub fn entry(&self) -> Option<EntryDecl> {
        super::child(&self.0)
    }
    pub fn exit(&self) -> Option<ExitDecl> {
        super::child(&self.0)
    }
    pub fn transitions(&self) -> AstChildren<crate::ast::TransitionDecl> {
        children(&self.0)
    }
    pub fn internal_transitions(&self) -> AstChildren<crate::ast::InternalDecl> {
        children(&self.0)
    }
    pub fn local_transitions(&self) -> AstChildren<crate::ast::LocalDecl> {
        children(&self.0)
    }
    pub fn completions(&self) -> AstChildren<crate::ast::CompletionDecl> {
        children(&self.0)
    }
    pub fn after(&self) -> AstChildren<AfterDecl> {
        children(&self.0)
    }
    pub fn every(&self) -> AstChildren<EveryDecl> {
        children(&self.0)
    }
    pub fn every_internal(&self) -> AstChildren<EveryInternalDecl> {
        children(&self.0)
    }
    pub fn defers(&self) -> AstChildren<DeferDecl> {
        children(&self.0)
    }
    /// The optional `is SubName` binding (Doc 04 §15). `None` for an
    /// ordinary state; `Some(_)` when the state's behaviour is an instance
    /// of a named submachine. Modelled as an optional child node (mirroring
    /// `entry`/`exit`) — a single STATE_DECL kind carries both shapes.
    pub fn submachine_ref(&self) -> Option<SubmachineRef> {
        super::child(&self.0)
    }
}

impl SubmachineRef {
    /// The referenced submachine name (the ident after `is`).
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl FinalDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl RegionDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn states(&self) -> AstChildren<StateDecl> {
        children(&self.0)
    }
}

impl ShallowHistoryDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn default(&self) -> Option<crate::ast::InitialDecl> {
        super::child(&self.0)
    }
}

impl DeepHistoryDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn default(&self) -> Option<crate::ast::InitialDecl> {
        super::child(&self.0)
    }
}

impl ChoiceDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn branches(&self) -> AstChildren<ChoiceBranch> {
        children(&self.0)
    }
}

impl JunctionDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn branches(&self) -> AstChildren<JunctionBranch> {
        children(&self.0)
    }
}

impl ForkDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn targets(&self) -> Option<ForkTargets> {
        super::child(&self.0)
    }
}

impl ForkTargets {
    pub fn names(&self) -> Vec<String> {
        ident_tokens(&self.0)
    }
}

impl JoinDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn sources(&self) -> Option<JoinSources> {
        super::child(&self.0)
    }
}

impl JoinSources {
    pub fn names(&self) -> Vec<String> {
        ident_tokens(&self.0)
    }
}

impl EntryPointDecl {
    pub fn name(&self) -> Option<String> {
        ident_tokens(&self.0).into_iter().next()
    }
    pub fn target(&self) -> Option<String> {
        ident_tokens(&self.0).into_iter().nth(1)
    }
}

impl ExitPointDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl DeferDecl {
    pub fn event(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

/// Return every direct `Ident` token's text under `n`. Used to extract
/// `{a, b, c}` lists where the AST simply stores tokens rather than a
/// dedicated child node per identifier.
pub(crate) fn ident_tokens(n: &SyntaxNode) -> Vec<String> {
    n.children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| t.kind() == crate::cst::SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .collect()
}

impl ChoiceBranch {
    pub fn target(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl JunctionBranch {
    pub fn target(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl AfterDecl {
    pub fn target(&self) -> Option<String> {
        first_ident(&self.0)
    }
    /// v1.1-W4 optional `likely`/`rare` prefix (e.g. `rare after 5000 ms ->
    /// Fault`). `None` ⇒ unhinted. Typed optional child.
    pub fn branch_hint(&self) -> Option<crate::ast::BranchHint> {
        super::child::<crate::ast::BranchHintNode>(&self.0).and_then(|h| h.kind())
    }
}

impl EveryDecl {
    pub fn target(&self) -> Option<String> {
        first_ident(&self.0)
    }
    /// v1.1-W4 optional `likely`/`rare` prefix (e.g. `likely every 100 ms ->
    /// Poll`). `None` ⇒ unhinted. Typed optional child.
    pub fn branch_hint(&self) -> Option<crate::ast::BranchHint> {
        super::child::<crate::ast::BranchHintNode>(&self.0).and_then(|h| h.kind())
    }
}

// EntryDecl / ExitDecl: action-block is the only non-trivial child; expose
// a typed accessor for the action block.
impl EntryDecl {
    pub fn action_block(&self) -> Option<crate::ast::ActionBlock> {
        crate::ast::ActionBlock::cast(self.0.children().next()?)
    }
}

impl ExitDecl {
    pub fn action_block(&self) -> Option<crate::ast::ActionBlock> {
        crate::ast::ActionBlock::cast(self.0.children().next()?)
    }
}
