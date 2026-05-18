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
    /// **W0 (Doc 29 §3.3):** every direct `initial` decl under this region.
    /// Used by the parallel-region check (FSM-E0600 "region has no
    /// `initial`"). 3-line typed-child iterator; behaviour-identical to the
    /// old `children().any(kind == INITIAL_DECL)` CST predicate.
    pub fn initials(&self) -> AstChildren<crate::ast::InitialDecl> {
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
    /// Branch target state name (after `->`). `first_ident` walks only this
    /// node's *direct* tokens; the guard's idents live inside the nested
    /// `GUARD_CLAUSE` child node, so the first direct ident is the target —
    /// the same reasoning that makes `TransitionDecl::trigger` skip a
    /// nested `BRANCH_HINT` ident.
    pub fn target(&self) -> Option<String> {
        first_ident(&self.0)
    }
    /// The `[ … ]` guard of this branch (`[else]` lowers to `GuardExpr::Else`
    /// in `lower_guard_clause`). `grammar/state.rs::parse_choice_branch`
    /// always wraps the bracket form in a `GUARD_CLAUSE` child; this mirrors
    /// `TransitionDecl::guard` exactly (typed first child of that kind).
    pub fn guard(&self) -> Option<crate::ast::GuardClause> {
        super::child(&self.0)
    }
    /// The optional `: action_list` block. Mirrors `TransitionDecl::actions`
    /// — the typed first `ACTION_BLOCK` child (`None` when the branch has no
    /// `:` suffix).
    pub fn actions(&self) -> Option<crate::ast::ActionBlock> {
        super::child(&self.0)
    }
}

impl JunctionBranch {
    /// Branch target state name (after `->`); see `ChoiceBranch::target`.
    pub fn target(&self) -> Option<String> {
        first_ident(&self.0)
    }
    /// The `[ … ]` guard of this branch; see `ChoiceBranch::guard`. The
    /// junction grammar (`parse_choice_branch_inner`) builds the identical
    /// `GUARD_CLAUSE`/`ACTION_BLOCK` child shape as a choice branch.
    pub fn guard(&self) -> Option<crate::ast::GuardClause> {
        super::child(&self.0)
    }
    /// The optional `: action_list` block; see `ChoiceBranch::actions`.
    pub fn actions(&self) -> Option<crate::ast::ActionBlock> {
        super::child(&self.0)
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
    /// W0 (Doc 29 §3.2): action block of an `every` timer. See the timer
    /// accessor block below for the shared rationale.
    pub fn action_block(&self) -> Option<crate::ast::ActionBlock> {
        action_block_child(&self.0)
    }
    pub fn duration(&self) -> Option<crate::ast::ConstExpr> {
        super::child(&self.0)
    }
}

// W0 (Doc 29 §3.2 B-clean): the timer decls (`after` / `every` /
// `every_internal`) carry an action block and a `N ms` duration as
// children. Before W0 the analyzer dropped to CST for both (`lower::state`
// `action_block_under` / `duration_ms`). These accessors are the typed
// home, mirroring the `EntryDecl::action_block` precedent above exactly.
// Each body is the *exact* CST walk the analyzer performed, relocated —
// behaviour-inert by construction (the W0 §4.2 byte-identity gate proves
// it). `duration()` returns the typed `CONST_EXPR` node; const-folding it
// stays the analyzer's job (it threads file consts the parser cannot see).
impl AfterDecl {
    pub fn action_block(&self) -> Option<crate::ast::ActionBlock> {
        action_block_child(&self.0)
    }
    pub fn duration(&self) -> Option<crate::ast::ConstExpr> {
        super::child(&self.0)
    }
}

impl EveryInternalDecl {
    pub fn action_block(&self) -> Option<crate::ast::ActionBlock> {
        action_block_child(&self.0)
    }
    pub fn duration(&self) -> Option<crate::ast::ConstExpr> {
        super::child(&self.0)
    }
}

/// First `ACTION_BLOCK` child of a timer node — the verbatim relocation of
/// the analyzer's old `action_block_under` helper (W0 / Doc 29 §3.2).
fn action_block_child(n: &SyntaxNode) -> Option<crate::ast::ActionBlock> {
    n.children()
        .find(|c| c.kind() == crate::cst::SyntaxKind::ACTION_BLOCK)
        .and_then(crate::ast::ActionBlock::cast)
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
