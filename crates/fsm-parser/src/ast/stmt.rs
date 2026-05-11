//! Typed AST accessors for action-sublanguage statements.

use crate::cst::{SyntaxKind, SyntaxNode};

use super::{ast_node, AstNode};

ast_node!(StmtAssign, STMT_ASSIGN);
ast_node!(StmtIf, STMT_IF);
ast_node!(StmtElse, STMT_ELSE);
ast_node!(StmtWhile, STMT_WHILE);
ast_node!(StmtFor, STMT_FOR);
ast_node!(StmtCall, STMT_CALL);
ast_node!(StmtRaise, STMT_RAISE);
ast_node!(StmtSend, STMT_SEND);
ast_node!(StmtDefer, STMT_DEFER);

/// Discriminated union over every statement form. Useful when iterating
/// `ActionBlock::statements()` and dispatching by shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Stmt {
    Assign(StmtAssign),
    If(StmtIf),
    While(StmtWhile),
    For(StmtFor),
    Call(StmtCall),
    Raise(StmtRaise),
    Send(StmtSend),
    Defer(StmtDefer),
}

impl AstNode for Stmt {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::STMT_ASSIGN
                | SyntaxKind::STMT_IF
                | SyntaxKind::STMT_WHILE
                | SyntaxKind::STMT_FOR
                | SyntaxKind::STMT_CALL
                | SyntaxKind::STMT_RAISE
                | SyntaxKind::STMT_SEND
                | SyntaxKind::STMT_DEFER
        )
    }
    fn cast(node: SyntaxNode) -> Option<Self> {
        Some(match node.kind() {
            SyntaxKind::STMT_ASSIGN => Stmt::Assign(StmtAssign(node)),
            SyntaxKind::STMT_IF => Stmt::If(StmtIf(node)),
            SyntaxKind::STMT_WHILE => Stmt::While(StmtWhile(node)),
            SyntaxKind::STMT_FOR => Stmt::For(StmtFor(node)),
            SyntaxKind::STMT_CALL => Stmt::Call(StmtCall(node)),
            SyntaxKind::STMT_RAISE => Stmt::Raise(StmtRaise(node)),
            SyntaxKind::STMT_SEND => Stmt::Send(StmtSend(node)),
            SyntaxKind::STMT_DEFER => Stmt::Defer(StmtDefer(node)),
            _ => return None,
        })
    }
    fn syntax(&self) -> &SyntaxNode {
        match self {
            Stmt::Assign(n) => &n.0,
            Stmt::If(n) => &n.0,
            Stmt::While(n) => &n.0,
            Stmt::For(n) => &n.0,
            Stmt::Call(n) => &n.0,
            Stmt::Raise(n) => &n.0,
            Stmt::Send(n) => &n.0,
            Stmt::Defer(n) => &n.0,
        }
    }
}
