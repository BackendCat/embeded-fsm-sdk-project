//! Typed AST accessors for transition / guard / priority / action-block.

use crate::cst::{SyntaxKind, SyntaxNode};

use super::{ast_node, children, first_ident, AstChildren, AstNode};

ast_node!(TransitionDecl, TRANSITION_DECL);
ast_node!(InternalDecl, INTERNAL_DECL);
ast_node!(LocalDecl, LOCAL_DECL);
ast_node!(CompletionDecl, COMPLETION_DECL);
ast_node!(GuardClause, GUARD_CLAUSE);
ast_node!(PriorityClause, PRIORITY_CLAUSE);
ast_node!(ActionBlock, ACTION_BLOCK);
ast_node!(BranchHintNode, BRANCH_HINT);

/// v1.1-W4 branch-prediction hint kind read off a `BRANCH_HINT` CST node.
/// Maps 1:1 to `fsm_ir::BranchHint`; kept distinct so the parser crate does
/// not depend on `fsm-ir` (the analyzer performs the AST→IR mapping).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchHint {
    Likely,
    Rare,
}

impl BranchHintNode {
    /// `Likely` / `Rare` from the wrapped ident token. Returns `None` only
    /// on a malformed tree with no ident child (defensive; the parser
    /// always emits exactly the `likely`/`rare` token it matched).
    pub fn kind(&self) -> Option<BranchHint> {
        match first_ident(&self.0).as_deref() {
            Some("likely") => Some(BranchHint::Likely),
            Some("rare") => Some(BranchHint::Rare),
            _ => None,
        }
    }
}

impl PriorityClause {
    /// The integer value inside a `priority N` clause, e.g. `7` in
    /// `priority 7`. `None` when the clause carries no integer literal
    /// (parse-error recovery).
    ///
    /// **W0 (Doc 29 §3.2 B-clean):** body is the *exact* CST walk the
    /// analyzer's `extract_priority` performed verbatim (find the first
    /// `IntLiteral` token, strip `_`, decimal `parse`), relocated to its
    /// correct home. Deliberately decimal-only `str::parse` — NOT
    /// `parse_int_literal_i64` (no hex/binary): a `priority 0x10` would
    /// have parsed to `None` (default priority) before, so changing the
    /// radix handling here would alter lowered-IR bytes. Behaviour-inert
    /// by construction; the W0 §4.2 byte-identity gate proves it.
    pub fn value(&self) -> Option<i64> {
        let tok = self
            .0
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| t.kind() == SyntaxKind::IntLiteral)?;
        let s: String = tok.text().chars().filter(|c| *c != '_').collect();
        s.parse().ok()
    }
}

/// Best-effort scan of a transition-flavour node for an
/// `IDENT '(' IDENT ')'` pattern immediately after the leading trigger.
/// Returns the second ident (the payload-binding name in `on E(p) -> …`).
/// The grammar produces no typed node for the binding, so the green-tree
/// tokens are walked directly here — the parser's CST is the right home
/// for this parser-shape knowledge (W0 / Doc 29 §3.2 B-clean; ported
/// verbatim from the analyzer's old `extract_trigger_payload_binding`,
/// behaviour-inert).
fn payload_binding_of(node: &SyntaxNode) -> Option<String> {
    let mut tokens = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| !t.kind().is_trivia());
    // Token sequence we expect: KwOn IDENT LParen IDENT RParen ...
    let _on = tokens.next()?;
    let _trigger = tokens.next()?;
    let lparen = tokens.next()?;
    if lparen.kind() != SyntaxKind::LParen {
        return None;
    }
    let binding = tokens.next()?;
    if binding.kind() != SyntaxKind::Ident {
        return None;
    }
    let rparen = tokens.next()?;
    if rparen.kind() != SyntaxKind::RParen {
        return None;
    }
    Some(binding.text().to_string())
}

/// Common interface for the four transition-flavour nodes — they all carry
/// optional guard / priority / action-block children but differ in trigger
/// semantics. Accessors are duplicated rather than abstracted with a trait
/// because the access patterns are small (4 nodes × 4 accessors) and a
/// trait would obscure rather than help.

impl TransitionDecl {
    /// Trigger event identifier, e.g. `START` in `on START -> Running`.
    pub fn trigger(&self) -> Option<String> {
        // v1.1-W4: a leading optional `BRANCH_HINT` child wraps the
        // `likely`/`rare` ident. `first_ident` walks tokens of THIS node
        // only (not descendants), and the hint ident lives *inside* the
        // BRANCH_HINT child node, so it is correctly skipped — the first
        // direct ident token is still the trigger.
        first_ident(&self.0)
    }
    /// Target state identifier (after `->`).
    pub fn target(&self) -> Option<String> {
        // Two direct `Ident` tokens under a TransitionDecl: trigger then
        // target. The hint ident is nested in BRANCH_HINT (a child node),
        // not a direct token, so the indices are unaffected by W4.
        nth_ident(&self.0, 1)
    }
    /// Payload-binding ident in `on E(p) -> …` (W0 / Doc 29 §3.2). `None`
    /// when the trigger has no `(binding)`.
    pub fn payload_binding(&self) -> Option<String> {
        payload_binding_of(&self.0)
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
    /// v1.1-W4 optional `likely`/`rare` prefix. `None` ⇒ unhinted (no
    /// `__builtin_expect` in codegen). Read as a typed optional child.
    pub fn branch_hint(&self) -> Option<BranchHint> {
        super::child::<BranchHintNode>(&self.0).and_then(|h| h.kind())
    }
}

impl InternalDecl {
    pub fn trigger(&self) -> Option<String> {
        first_ident(&self.0)
    }
    /// Payload-binding ident in `internal on E(p): …` (W0 / Doc 29 §3.2).
    pub fn payload_binding(&self) -> Option<String> {
        payload_binding_of(&self.0)
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
    /// v1.1-W4 optional `likely`/`rare` prefix (e.g. `rare on FAULT : ...`).
    pub fn branch_hint(&self) -> Option<BranchHint> {
        super::child::<BranchHintNode>(&self.0).and_then(|h| h.kind())
    }
}

impl LocalDecl {
    pub fn trigger(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn target(&self) -> Option<String> {
        nth_ident(&self.0, 1)
    }
    /// Payload-binding ident in `local on E(p) ~> …` (W0 / Doc 29 §3.2).
    pub fn payload_binding(&self) -> Option<String> {
        payload_binding_of(&self.0)
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
    /// v1.1-W4 optional `likely`/`rare` prefix (e.g. `likely on E ~> T`).
    pub fn branch_hint(&self) -> Option<BranchHint> {
        super::child::<BranchHintNode>(&self.0).and_then(|h| h.kind())
    }
}

impl CompletionDecl {
    pub fn target(&self) -> Option<String> {
        // `done [g] -> Target` — first DIRECT ident is the target. A W4
        // `BRANCH_HINT` child (from `rare done -> Target`) nests its ident
        // inside the child node, so it does not shift this.
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
    /// v1.1-W4 optional `likely`/`rare` prefix (e.g. `rare done -> Fault`).
    pub fn branch_hint(&self) -> Option<BranchHint> {
        super::child::<BranchHintNode>(&self.0).and_then(|h| h.kind())
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
