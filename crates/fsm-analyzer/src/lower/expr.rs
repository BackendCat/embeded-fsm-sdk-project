//! Action / guard sub-language lowering.
//!
//! Doc 04 §8.5 + §8.7 + §9 / Doc 09 §7-§9. Walks the typed AST produced by
//! the Pratt expression parser and reshapes it into IR [`Statement`] /
//! [`GuardExpr`] / [`IrExpr`] trees the simulator and codegen consume. On a
//! malformed sub-expression we emit the most-recoverable IR fallback (see
//! [`guard_recovery`]) rather than panic — the parser already flagged the
//! syntactic error.
//!
//! AD-3 (2026-05-15): these were `LoweringCtx` methods + the free helpers
//! that followed them. Verbatim transcription with `self` replaced by
//! `&mut IdMinter` / `&LocCtx`. The only state these actually need is the
//! machine name (for `ev-…` ids) and `loc` (for cast loc) — same output.

use fsm_ir::{
    BinaryOp, BoolLit, CastExpr, CmpOp, EnumVariantLit, Expr as IrExpr, FieldRef, GuardExpr,
    GuardOperand, Literal, Statement, Type, UnaryOp,
};
use fsm_parser::ast::{self, AstNode};
use fsm_parser::cst::{SyntaxKind, SyntaxNode};

use super::ids::IdMinter;
use super::loc::LocCtx;
use super::machine::{lower_literal, lower_type_ref_node};

pub(super) fn lower_action_block(
    ids: &mut IdMinter,
    locs: &LocCtx,
    ab: &ast::ActionBlock,
) -> Vec<Statement> {
    let mut out = Vec::new();
    for stmt in ab.statements() {
        if let Some(s) = lower_stmt(ids, locs, &stmt) {
            out.push(s);
        }
    }
    out
}

fn lower_stmt(ids: &mut IdMinter, locs: &LocCtx, stmt: &ast::Stmt) -> Option<Statement> {
    match stmt {
        ast::Stmt::Assign(a) => lower_stmt_assign(ids, locs, a),
        ast::Stmt::If(i) => lower_stmt_if(ids, locs, i),
        ast::Stmt::While(w) => lower_stmt_while(ids, locs, w),
        ast::Stmt::For(f) => lower_stmt_for(ids, locs, f),
        ast::Stmt::Call(c) => lower_stmt_call(ids, locs, c),
        ast::Stmt::Raise(r) => lower_stmt_raise(ids, locs, r),
        ast::Stmt::Send(s) => lower_stmt_send(ids, locs, s),
        ast::Stmt::Defer(d) => lower_stmt_defer(ids, d),
    }
}

fn lower_stmt_assign(ids: &mut IdMinter, locs: &LocCtx, a: &ast::StmtAssign) -> Option<Statement> {
    // STMT_ASSIGN children: lhs Expr, RHS Expr.
    let mut exprs = a
        .syntax()
        .children()
        .filter_map(|n| ast::Expr::cast(n.clone()));
    let lhs_ast = exprs.next()?;
    let rhs_ast = exprs.next()?;
    let target = expr_to_field_ref(&lhs_ast)?;
    let value = lower_action_expr(ids, locs, &rhs_ast);
    Some(Statement::Assign { target, value })
}

fn lower_stmt_if(ids: &mut IdMinter, locs: &LocCtx, i: &ast::StmtIf) -> Option<Statement> {
    // STMT_IF children (in order): condition Expr, then-ACTION_BLOCK,
    // optional STMT_ELSE wrapping either another STMT_IF or an
    // ACTION_BLOCK.
    let mut iter = i.syntax().children();
    let cond_node = iter.next()?;
    let condition = ast::Expr::cast(cond_node.clone())
        .map(|e| lower_action_expr(ids, locs, &e))
        .unwrap_or_else(literal_false_expr);
    let then_block = iter.next()?;
    let then = if then_block.kind() == SyntaxKind::ACTION_BLOCK {
        ast::ActionBlock::cast(then_block.clone())
            .map(|ab| lower_action_block(ids, locs, &ab))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let else_ = iter
        .find(|n| n.kind() == SyntaxKind::STMT_ELSE)
        .map(|n| lower_stmt_else_body(ids, locs, &n))
        .unwrap_or_default();
    Some(Statement::If {
        condition,
        then,
        else_,
    })
}

fn lower_stmt_else_body(ids: &mut IdMinter, locs: &LocCtx, n: &SyntaxNode) -> Vec<Statement> {
    // STMT_ELSE wraps either an `if` chain (else-if) or an ACTION_BLOCK.
    for child in n.children() {
        match child.kind() {
            SyntaxKind::STMT_IF => {
                if let Some(if_stmt) = ast::StmtIf::cast(child.clone()) {
                    if let Some(s) = lower_stmt_if(ids, locs, &if_stmt) {
                        return vec![s];
                    }
                }
            }
            SyntaxKind::ACTION_BLOCK => {
                if let Some(ab) = ast::ActionBlock::cast(child.clone()) {
                    return lower_action_block(ids, locs, &ab);
                }
            }
            _ => {}
        }
    }
    Vec::new()
}

fn lower_stmt_while(ids: &mut IdMinter, locs: &LocCtx, w: &ast::StmtWhile) -> Option<Statement> {
    // STMT_WHILE children: condition Expr, body ACTION_BLOCK.
    let mut iter = w.syntax().children();
    let cond_node = iter.next()?;
    let condition = ast::Expr::cast(cond_node.clone())
        .map(|e| lower_action_expr(ids, locs, &e))
        .unwrap_or_else(literal_false_expr);
    let body = iter
        .find(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
        .and_then(ast::ActionBlock::cast)
        .map(|ab| lower_action_block(ids, locs, &ab))
        .unwrap_or_default();
    Some(Statement::While { condition, body })
}

fn lower_stmt_for(ids: &mut IdMinter, locs: &LocCtx, f: &ast::StmtFor) -> Option<Statement> {
    // STMT_FOR children (Pratt-ordered): init STMT_ASSIGN, condition Expr,
    // update STMT_ASSIGN, body ACTION_BLOCK.
    let children: Vec<SyntaxNode> = f.syntax().children().collect();
    let init_node = children
        .iter()
        .find(|n| n.kind() == SyntaxKind::STMT_ASSIGN)?;
    let init =
        ast::StmtAssign::cast(init_node.clone()).and_then(|a| lower_stmt_assign(ids, locs, &a))?;
    let mut assigns_seen = 0usize;
    let mut update: Option<Statement> = None;
    let mut condition: Option<IrExpr> = None;
    let mut body: Vec<Statement> = Vec::new();
    for n in &children {
        match n.kind() {
            SyntaxKind::STMT_ASSIGN => {
                assigns_seen += 1;
                if assigns_seen == 2 {
                    update = ast::StmtAssign::cast(n.clone())
                        .and_then(|a| lower_stmt_assign(ids, locs, &a));
                }
            }
            SyntaxKind::ACTION_BLOCK => {
                if let Some(ab) = ast::ActionBlock::cast(n.clone()) {
                    body = lower_action_block(ids, locs, &ab);
                }
            }
            _ => {
                if condition.is_none() {
                    if let Some(e) = ast::Expr::cast(n.clone()) {
                        condition = Some(lower_action_expr(ids, locs, &e));
                    }
                }
            }
        }
    }
    Some(Statement::For {
        init: Box::new(init),
        condition: condition.unwrap_or_else(literal_false_expr),
        update: Box::new(update.unwrap_or(Statement::Call {
            callee: String::new(),
            args: Vec::new(),
        })),
        body,
    })
}

fn lower_stmt_call(ids: &mut IdMinter, locs: &LocCtx, c: &ast::StmtCall) -> Option<Statement> {
    // STMT_CALL wraps a single Expr (a call or bare-ident). Allow the
    // bare-ident form to lower to a zero-arg call so users may write
    // `reset_link` as a no-arg extern invocation.
    let expr = c.syntax().children().find_map(ast::Expr::cast)?;
    match expr {
        ast::Expr::Call(call_node) => {
            let (callee, args) = lower_call_form(ids, locs, call_node.syntax())?;
            Some(Statement::Call { callee, args })
        }
        ast::Expr::NameRef(_) => {
            let callee = first_ident_text(expr.syntax())?;
            Some(Statement::Call {
                callee,
                args: Vec::new(),
            })
        }
        _ => None,
    }
}

fn lower_stmt_raise(ids: &mut IdMinter, locs: &LocCtx, r: &ast::StmtRaise) -> Option<Statement> {
    // `raise EVT(args?)` — first ident is the event name.
    let event = first_ident_text(r.syntax())?;
    let args = lower_arg_list_under(ids, locs, r.syntax());
    Some(Statement::Raise {
        event_id: format!("ev-{}-{event}", ids.machine_name),
        args,
    })
}

fn lower_stmt_send(ids: &mut IdMinter, locs: &LocCtx, s: &ast::StmtSend) -> Option<Statement> {
    // `send EVT(args?) to TARGET` — two ident tokens (event then target).
    let idents = ident_token_texts(s.syntax());
    let event = idents.first()?.clone();
    let machine_target = idents.get(1).cloned().unwrap_or_default();
    let args = lower_arg_list_under(ids, locs, s.syntax());
    Some(Statement::Send {
        event_id: format!("ev-{}-{event}", ids.machine_name),
        args,
        machine_id: machine_target,
    })
}

fn lower_stmt_defer(ids: &mut IdMinter, d: &ast::StmtDefer) -> Option<Statement> {
    let event = first_ident_text(d.syntax())?;
    Some(Statement::Defer {
        event_id: format!("ev-{}-{event}", ids.machine_name),
    })
}

fn lower_arg_list_under(ids: &mut IdMinter, locs: &LocCtx, parent: &SyntaxNode) -> Vec<IrExpr> {
    parent
        .children()
        .find(|n| n.kind() == SyntaxKind::ARG_LIST)
        .map(|al| lower_arg_list(ids, locs, &al))
        .unwrap_or_default()
}

fn lower_arg_list(ids: &mut IdMinter, locs: &LocCtx, node: &SyntaxNode) -> Vec<IrExpr> {
    node.children()
        .filter_map(|c| ast::Expr::cast(c.clone()))
        .map(|e| lower_action_expr(ids, locs, &e))
        .collect()
}

fn lower_action_expr(ids: &mut IdMinter, locs: &LocCtx, e: &ast::Expr) -> IrExpr {
    match e {
        ast::Expr::Binary(b) => lower_binary_expr(ids, locs, b.syntax()),
        ast::Expr::Unary(u) => lower_unary_expr(ids, locs, u.syntax()),
        ast::Expr::Cast(c) => lower_cast_expr(ids, locs, c.syntax()),
        ast::Expr::Call(c) => {
            let (callee, args) = lower_call_form(ids, locs, c.syntax())
                .unwrap_or_else(|| (String::new(), Vec::new()));
            IrExpr::Call { callee, args }
        }
        ast::Expr::FieldRef(f) => match field_ref_from_node(f.syntax()) {
            Some(field_ref) => IrExpr::FieldRef { field_ref },
            None => {
                // Treat `EnumName.Variant` as an enum-variant literal
                // when it isn't a `ctx.x` / `payload.x` reference.
                if let Some(lit) = enum_variant_from_field_ref(f.syntax()) {
                    IrExpr::Literal(Literal::EnumVariant(lit))
                } else {
                    IrExpr::Literal(Literal::Bool(BoolLit {
                        value: false,
                        loc: None,
                    }))
                }
            }
        },
        ast::Expr::Literal(l) => IrExpr::Literal(lower_literal(ids, locs, l.syntax()).unwrap_or(
            Literal::Bool(BoolLit {
                value: false,
                loc: None,
            }),
        )),
        ast::Expr::NameRef(n) => {
            // A bare identifier in an action-expression position is most
            // commonly a zero-arg extern call (e.g. `reset_link`). The
            // analyzer's scope check determines whether the name resolves
            // to an extern; here we conservatively model it as a Call
            // with no args so codegen/simulator can use it.
            let callee = first_ident_text(n.syntax()).unwrap_or_default();
            IrExpr::Call {
                callee,
                args: Vec::new(),
            }
        }
        ast::Expr::QualifiedName(q) => {
            // Pratt parser routes most qualified names through FieldRef,
            // but the EXPR_QUALIFIED_NAME shape may appear for explicit
            // enum-variant literals.
            if let Some(lit) = enum_variant_from_field_ref(q.syntax()) {
                IrExpr::Literal(Literal::EnumVariant(lit))
            } else {
                IrExpr::Literal(Literal::Bool(BoolLit {
                    value: false,
                    loc: None,
                }))
            }
        }
        ast::Expr::Paren(p) => p
            .syntax()
            .children()
            .find_map(ast::Expr::cast)
            .map(|inner| lower_action_expr(ids, locs, &inner))
            .unwrap_or_else(literal_false_expr),
        ast::Expr::GuardElse(_) => {
            // `else` is a guard-only marker; in action position fall back
            // to a constant false so the parser-emitted diagnostic
            // remains the sole source of error reporting.
            literal_false_expr()
        }
    }
}

fn lower_binary_expr(ids: &mut IdMinter, locs: &LocCtx, node: &SyntaxNode) -> IrExpr {
    let mut children = node.children();
    let lhs = children
        .next()
        .and_then(ast::Expr::cast)
        .map(|e| lower_action_expr(ids, locs, &e))
        .unwrap_or_else(literal_false_expr);
    let rhs = children
        .next()
        .and_then(ast::Expr::cast)
        .map(|e| lower_action_expr(ids, locs, &e))
        .unwrap_or_else(literal_false_expr);
    let op = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find_map(|t| binary_op_from_kind(t.kind()))
        .unwrap_or(BinaryOp::Eq);
    IrExpr::Binary {
        op,
        left: Box::new(lhs),
        right: Box::new(rhs),
    }
}

fn lower_unary_expr(ids: &mut IdMinter, locs: &LocCtx, node: &SyntaxNode) -> IrExpr {
    let operand = node
        .children()
        .find_map(ast::Expr::cast)
        .map(|e| lower_action_expr(ids, locs, &e))
        .unwrap_or_else(literal_false_expr);
    let op = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find_map(|t| unary_op_from_kind(t.kind()))
        .unwrap_or(UnaryOp::Not);
    IrExpr::Unary {
        op,
        operand: Box::new(operand),
    }
}

fn lower_cast_expr(ids: &mut IdMinter, locs: &LocCtx, node: &SyntaxNode) -> IrExpr {
    let operand = node
        .children()
        .find_map(ast::Expr::cast)
        .map(|e| lower_action_expr(ids, locs, &e))
        .unwrap_or_else(literal_false_expr);
    // OPAQUE-BUG-1 (same class): resolve from the CAST_EXPR node so a
    // `(opaque "T *") x` cast carries the verbatim C type instead of
    // silently falling back to `i64`.
    let target_type = lower_type_ref_node(node).unwrap_or(Type::Primitive { name: "i64".into() });
    IrExpr::Cast(CastExpr {
        operand: Box::new(operand),
        target_type,
        loc: locs.loc(node),
    })
}

fn lower_call_form(
    ids: &mut IdMinter,
    locs: &LocCtx,
    node: &SyntaxNode,
) -> Option<(String, Vec<IrExpr>)> {
    // EXPR_CALL children: callee Expr (typically EXPR_NAME_REF), ARG_LIST.
    let callee_node = node.children().next()?;
    let callee = first_ident_text(&callee_node)?;
    let args = lower_arg_list_under(ids, locs, node);
    Some((callee, args))
}

// ----- Guard expressions ---------------------------------------------------

pub(super) fn lower_guard_clause(
    ids: &mut IdMinter,
    locs: &LocCtx,
    g: &ast::GuardClause,
) -> GuardExpr {
    match g.expr() {
        Some(expr) => lower_guard_expr(ids, locs, &expr),
        // Empty `[]` recovers to a falsy guard so the transition stays
        // inert until the user authors a valid guard.
        None => GuardExpr::Not {
            operand: Box::new(GuardExpr::Else),
        },
    }
}

fn lower_guard_expr(ids: &mut IdMinter, locs: &LocCtx, e: &ast::Expr) -> GuardExpr {
    match e {
        ast::Expr::GuardElse(_) => GuardExpr::Else,
        ast::Expr::Literal(l) => match lower_literal(ids, locs, l.syntax()) {
            Some(Literal::Bool(b)) => {
                if b.value {
                    // `[true]` is exact-equal to "no guard"; emit a
                    // tautological field-cmp (1 == 1) so the IR remains
                    // serializable without an extra discriminant.
                    GuardExpr::ExternCall {
                        callee: "__true".into(),
                        args: vec![IrExpr::Literal(Literal::Bool(BoolLit {
                            value: true,
                            loc: None,
                        }))],
                    }
                } else {
                    GuardExpr::Not {
                        operand: Box::new(GuardExpr::Else),
                    }
                }
            }
            _ => guard_recovery(),
        },
        ast::Expr::Unary(u) => {
            // Only `!` is meaningful in guard context.
            let inner = u
                .syntax()
                .children()
                .find_map(ast::Expr::cast)
                .map(|e| lower_guard_expr(ids, locs, &e))
                .unwrap_or_else(guard_recovery);
            let is_bang = u
                .syntax()
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .any(|t| t.kind() == SyntaxKind::Bang);
            if is_bang {
                GuardExpr::Not {
                    operand: Box::new(inner),
                }
            } else {
                inner
            }
        }
        ast::Expr::Binary(b) => lower_guard_binary(ids, locs, b.syntax()),
        ast::Expr::Paren(p) => p
            .syntax()
            .children()
            .find_map(ast::Expr::cast)
            .map(|inner| lower_guard_expr(ids, locs, &inner))
            .unwrap_or_else(guard_recovery),
        ast::Expr::Call(c) => {
            let (callee, args) = lower_call_form(ids, locs, c.syntax())
                .unwrap_or_else(|| (String::new(), Vec::new()));
            GuardExpr::ExternCall { callee, args }
        }
        ast::Expr::NameRef(n) => {
            // A bare identifier in guard position is a zero-arg pure
            // extern call — the common case (`[can_start]`).
            let callee = first_ident_text(n.syntax()).unwrap_or_default();
            GuardExpr::ExternCall {
                callee,
                args: Vec::new(),
            }
        }
        ast::Expr::FieldRef(_) | ast::Expr::QualifiedName(_) | ast::Expr::Cast(_) => {
            guard_recovery()
        }
    }
}

fn lower_guard_binary(ids: &mut IdMinter, locs: &LocCtx, node: &SyntaxNode) -> GuardExpr {
    let op_tok = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| guard_op_kind(t.kind()).is_some());
    let kind = op_tok
        .as_ref()
        .map(|t| t.kind())
        .unwrap_or(SyntaxKind::EqEq);
    let mut child_exprs = node.children().filter_map(|c| ast::Expr::cast(c.clone()));
    let lhs = child_exprs.next();
    let rhs = child_exprs.next();
    match (kind, lhs, rhs) {
        (SyntaxKind::AmpAmp, Some(l), Some(r)) => GuardExpr::And {
            left: Box::new(lower_guard_expr(ids, locs, &l)),
            right: Box::new(lower_guard_expr(ids, locs, &r)),
        },
        (SyntaxKind::PipePipe, Some(l), Some(r)) => GuardExpr::Or {
            left: Box::new(lower_guard_expr(ids, locs, &l)),
            right: Box::new(lower_guard_expr(ids, locs, &r)),
        },
        (kind, Some(l), Some(r)) => {
            let op = match cmp_op_from_kind(kind) {
                Some(o) => o,
                None => return guard_recovery(),
            };
            let Some(lhs_ref) = expr_to_field_ref(&l) else {
                return guard_recovery();
            };
            let rhs_operand = match guard_operand_from_expr(&r) {
                Some(o) => o,
                None => match lower_literal(ids, locs, r.syntax()) {
                    Some(lit) => GuardOperand::Literal(lit),
                    None => return guard_recovery(),
                },
            };
            GuardExpr::FieldCmp {
                lhs: lhs_ref,
                op,
                rhs: rhs_operand,
            }
        }
        _ => guard_recovery(),
    }
}

/// Recovery sentinel for malformed guards: never-true so the transition
/// stays inert and the parser's diagnostic remains the surfaced error.
fn guard_recovery() -> GuardExpr {
    GuardExpr::Not {
        operand: Box::new(GuardExpr::Else),
    }
}

// ---------------------------------------------------------------------------
// Free helpers for action/guard lowering — pure tree-rewrites that don't
// need the IdMinter / LocCtx state.
// ---------------------------------------------------------------------------

/// Translate an action-language `Expr::FieldRef` into the IR's `FieldRef`,
/// which uses `Ctx { field } | Payload { field }`. Returns `None` for any
/// other shape (e.g. `EnumName.Variant`).
fn expr_to_field_ref(e: &ast::Expr) -> Option<FieldRef> {
    let node = e.syntax();
    if node.kind() != SyntaxKind::EXPR_FIELD_REF {
        return None;
    }
    field_ref_from_node(node)
}

/// Same as [`expr_to_field_ref`] but operating on a raw syntax node. Returns
/// `None` if the leading qualifier isn't `ctx` / `payload`.
fn field_ref_from_node(node: &SyntaxNode) -> Option<FieldRef> {
    let head = node.children().next().and_then(|c| first_ident_text(&c))?;
    let field = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)?
        .text()
        .to_string();
    match head.as_str() {
        "ctx" => Some(FieldRef::Ctx { field }),
        "payload" => Some(FieldRef::Payload { field }),
        _ => None,
    }
}

/// Translate an `EnumName.Variant` field-ref into an enum-variant literal.
/// Only fires when the leading qualifier is not `ctx` / `payload`.
fn enum_variant_from_field_ref(node: &SyntaxNode) -> Option<EnumVariantLit> {
    let enum_name = node.children().next().and_then(|c| first_ident_text(&c))?;
    if enum_name == "ctx" || enum_name == "payload" {
        return None;
    }
    let variant_name = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)?
        .text()
        .to_string();
    Some(EnumVariantLit {
        enum_name,
        variant_name,
        loc: None,
    })
}

/// First Ident text descending into the typed expression subtree.
fn first_ident_text(node: &SyntaxNode) -> Option<String> {
    node.descendants_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
}

fn ident_token_texts(node: &SyntaxNode) -> Vec<String> {
    node.children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .collect()
}

fn guard_operand_from_expr(e: &ast::Expr) -> Option<GuardOperand> {
    match e {
        ast::Expr::FieldRef(_) => {
            let f = expr_to_field_ref(e)?;
            Some(GuardOperand::FieldRef(f))
        }
        _ => None,
    }
}

fn binary_op_from_kind(k: SyntaxKind) -> Option<BinaryOp> {
    Some(match k {
        SyntaxKind::Plus => BinaryOp::Add,
        SyntaxKind::Minus => BinaryOp::Sub,
        SyntaxKind::Star => BinaryOp::Mul,
        SyntaxKind::Slash => BinaryOp::Div,
        SyntaxKind::Percent => BinaryOp::Mod,
        SyntaxKind::Amp => BinaryOp::BitAnd,
        SyntaxKind::Pipe => BinaryOp::BitOr,
        SyntaxKind::Caret => BinaryOp::BitXor,
        SyntaxKind::Shl => BinaryOp::Shl,
        SyntaxKind::Shr => BinaryOp::Shr,
        SyntaxKind::AmpAmp => BinaryOp::LogAnd,
        SyntaxKind::PipePipe => BinaryOp::LogOr,
        SyntaxKind::EqEq => BinaryOp::Eq,
        SyntaxKind::BangEq => BinaryOp::NotEq,
        SyntaxKind::Lt => BinaryOp::Lt,
        SyntaxKind::Gt => BinaryOp::Gt,
        SyntaxKind::Le => BinaryOp::LtEq,
        SyntaxKind::Ge => BinaryOp::GtEq,
        _ => return None,
    })
}

fn unary_op_from_kind(k: SyntaxKind) -> Option<UnaryOp> {
    Some(match k {
        SyntaxKind::Bang => UnaryOp::Not,
        SyntaxKind::Minus => UnaryOp::Neg,
        SyntaxKind::Tilde => UnaryOp::BitNot,
        _ => return None,
    })
}

fn guard_op_kind(k: SyntaxKind) -> Option<()> {
    matches!(
        k,
        SyntaxKind::EqEq
            | SyntaxKind::BangEq
            | SyntaxKind::Lt
            | SyntaxKind::Gt
            | SyntaxKind::Le
            | SyntaxKind::Ge
            | SyntaxKind::AmpAmp
            | SyntaxKind::PipePipe
    )
    .then_some(())
}

fn cmp_op_from_kind(k: SyntaxKind) -> Option<CmpOp> {
    Some(match k {
        SyntaxKind::EqEq => CmpOp::Eq,
        SyntaxKind::BangEq => CmpOp::NotEq,
        SyntaxKind::Lt => CmpOp::Lt,
        SyntaxKind::Gt => CmpOp::Gt,
        SyntaxKind::Le => CmpOp::LtEq,
        SyntaxKind::Ge => CmpOp::GtEq,
        _ => return None,
    })
}

fn literal_false_expr() -> IrExpr {
    IrExpr::Literal(Literal::Bool(BoolLit {
        value: false,
        loc: None,
    }))
}
