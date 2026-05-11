//! Typed AST accessors for expressions and the guard-else marker.

use crate::cst::{SyntaxKind, SyntaxNode};

use super::{ast_node, AstNode};

ast_node!(ExprBinary, EXPR_BINARY);
ast_node!(ExprUnary, EXPR_UNARY);
ast_node!(ExprCast, EXPR_CAST);
ast_node!(ExprCall, EXPR_CALL);
ast_node!(ExprFieldRef, EXPR_FIELD_REF);
ast_node!(ExprLiteral, EXPR_LITERAL);
ast_node!(ExprNameRef, EXPR_NAME_REF);
ast_node!(ExprQualifiedName, EXPR_QUALIFIED_NAME);
ast_node!(ExprParen, EXPR_PAREN);
ast_node!(ArgList, ARG_LIST);
ast_node!(GuardElse, GUARD_ELSE);

/// Discriminated union over every expression shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Expr {
    Binary(ExprBinary),
    Unary(ExprUnary),
    Cast(ExprCast),
    Call(ExprCall),
    FieldRef(ExprFieldRef),
    Literal(ExprLiteral),
    NameRef(ExprNameRef),
    QualifiedName(ExprQualifiedName),
    Paren(ExprParen),
    GuardElse(GuardElse),
}

impl AstNode for Expr {
    fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::EXPR_BINARY
                | SyntaxKind::EXPR_UNARY
                | SyntaxKind::EXPR_CAST
                | SyntaxKind::EXPR_CALL
                | SyntaxKind::EXPR_FIELD_REF
                | SyntaxKind::EXPR_LITERAL
                | SyntaxKind::EXPR_NAME_REF
                | SyntaxKind::EXPR_QUALIFIED_NAME
                | SyntaxKind::EXPR_PAREN
                | SyntaxKind::GUARD_ELSE
        )
    }

    fn cast(node: SyntaxNode) -> Option<Self> {
        Some(match node.kind() {
            SyntaxKind::EXPR_BINARY => Expr::Binary(ExprBinary(node)),
            SyntaxKind::EXPR_UNARY => Expr::Unary(ExprUnary(node)),
            SyntaxKind::EXPR_CAST => Expr::Cast(ExprCast(node)),
            SyntaxKind::EXPR_CALL => Expr::Call(ExprCall(node)),
            SyntaxKind::EXPR_FIELD_REF => Expr::FieldRef(ExprFieldRef(node)),
            SyntaxKind::EXPR_LITERAL => Expr::Literal(ExprLiteral(node)),
            SyntaxKind::EXPR_NAME_REF => Expr::NameRef(ExprNameRef(node)),
            SyntaxKind::EXPR_QUALIFIED_NAME => Expr::QualifiedName(ExprQualifiedName(node)),
            SyntaxKind::EXPR_PAREN => Expr::Paren(ExprParen(node)),
            SyntaxKind::GUARD_ELSE => Expr::GuardElse(GuardElse(node)),
            _ => return None,
        })
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Expr::Binary(n) => &n.0,
            Expr::Unary(n) => &n.0,
            Expr::Cast(n) => &n.0,
            Expr::Call(n) => &n.0,
            Expr::FieldRef(n) => &n.0,
            Expr::Literal(n) => &n.0,
            Expr::NameRef(n) => &n.0,
            Expr::QualifiedName(n) => &n.0,
            Expr::Paren(n) => &n.0,
            Expr::GuardElse(n) => &n.0,
        }
    }
}
