//! Action-sublanguage statement formatter.
//!
//! Doc 04 §8.7 statements:
//!   `assign_stmt | if_stmt | while_stmt | for_stmt | call_action`
//!   `| raise_action | send_action | defer_stmt`
//!
//! An `ACTION_BLOCK` node holds zero or more of these flat under it. The
//! formatter decides between two layouts:
//!
//! - **Inline** — one short statement on one line, used for `entry: { fn(); }`
//!   and short transition actions per Doc 19 §9.
//! - **Multi-line** — each statement on its own line, indented one level
//!   deeper than the opening `{`.

use fsm_parser::{SyntaxKind, SyntaxNode};
use rowan::NodeOrToken;

use super::expr::emit_expr;
use super::writer::FormatWriter;

/// Emit an `ACTION_BLOCK`. `surrounding_indent` controls inline vs.
/// multi-line: if the writer is already on a line with content and the
/// block has at most one simple statement, inline; otherwise multi-line.
pub(crate) fn emit_action_block(
    w: &mut FormatWriter,
    node: &SyntaxNode,
    inline_threshold_chars: usize,
) {
    debug_assert_eq!(node.kind(), SyntaxKind::ACTION_BLOCK);

    let stmts: Vec<SyntaxNode> = node
        .children()
        .filter(|c| is_statement_kind(c.kind()))
        .collect();

    let inline_ok = stmts.len() <= 1
        && stmts
            .first()
            .map(|s| !is_compound_statement(s.kind()))
            .unwrap_or(true)
        && estimate_stmt_width(&stmts) <= inline_threshold_chars;

    if inline_ok {
        // Inline: `{ stmt; }`. If there are zero statements emit `{ }`.
        w.write("{");
        if let Some(s) = stmts.first() {
            w.space();
            emit_statement(w, s);
            w.write(";");
            w.space();
        } else {
            w.space();
        }
        w.write("}");
    } else {
        // Multi-line: opening brace stays on caller's current line.
        w.write("{");
        w.newline();
        w.indent();
        for s in &stmts {
            emit_statement(w, s);
            w.write(";");
            w.newline();
        }
        w.dedent();
        w.write("}");
    }
}

fn is_statement_kind(k: SyntaxKind) -> bool {
    matches!(
        k,
        SyntaxKind::STMT_ASSIGN
            | SyntaxKind::STMT_CALL
            | SyntaxKind::STMT_RAISE
            | SyntaxKind::STMT_SEND
            | SyntaxKind::STMT_DEFER
            | SyntaxKind::STMT_IF
            | SyntaxKind::STMT_WHILE
            | SyntaxKind::STMT_FOR
    )
}

fn is_compound_statement(k: SyntaxKind) -> bool {
    matches!(
        k,
        SyntaxKind::STMT_IF | SyntaxKind::STMT_WHILE | SyntaxKind::STMT_FOR
    )
}

fn estimate_stmt_width(stmts: &[SyntaxNode]) -> usize {
    stmts
        .iter()
        .map(|s| s.text().to_string().len())
        .sum::<usize>()
        + stmts.len()
}

/// Emit an ACTION_BLOCK as `stmt; stmt; …` with no surrounding braces.
/// Used by every Doc 04 §8 form that introduces `: action_list` —
/// transitions, internal transitions, entry/exit, completion, timers,
/// choice/junction branches. The braced-block variant (`{ … }`) is used
/// **only** as the body of compound statements (`if`/`while`/`for`).
pub(crate) fn emit_inline_action_list(w: &mut FormatWriter, action: &SyntaxNode) {
    let stmts: Vec<SyntaxNode> = action
        .children()
        .filter(|n| is_statement_kind(n.kind()))
        .collect();
    for (i, s) in stmts.iter().enumerate() {
        if i > 0 {
            w.write("; ");
        }
        emit_statement(w, s);
    }
}

/// Dispatch one statement.
pub(crate) fn emit_statement(w: &mut FormatWriter, node: &SyntaxNode) {
    match node.kind() {
        SyntaxKind::STMT_ASSIGN => emit_assign(w, node),
        SyntaxKind::STMT_CALL => emit_call_stmt(w, node),
        SyntaxKind::STMT_RAISE => emit_raise(w, node),
        SyntaxKind::STMT_SEND => emit_send(w, node),
        SyntaxKind::STMT_DEFER => emit_defer(w, node),
        SyntaxKind::STMT_IF => emit_if(w, node),
        SyntaxKind::STMT_WHILE => emit_while(w, node),
        SyntaxKind::STMT_FOR => emit_for(w, node),
        _ => {
            // Defensive raw re-emit for unknown forms (e.g. ERROR_NODE).
            w.write(node.text().to_string().trim());
        }
    }
}

fn emit_assign(w: &mut FormatWriter, node: &SyntaxNode) {
    // Two expression children: lhs, rhs. The CST does not name them, so
    // we take them in order.
    let exprs: Vec<SyntaxNode> = node.children().filter(|n| is_expr_kind(n.kind())).collect();
    if let Some(lhs) = exprs.first() {
        emit_expr(w, lhs);
        w.write(" = ");
    }
    if let Some(rhs) = exprs.get(1) {
        emit_expr(w, rhs);
    }
}

fn emit_call_stmt(w: &mut FormatWriter, node: &SyntaxNode) {
    if let Some(inner) = node.children().find(|n| is_expr_kind(n.kind())) {
        emit_expr(w, &inner);
    }
}

fn emit_raise(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("raise ");
    let mut wrote_name = false;
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Token(t) if !t.kind().is_trivia() => {
                if t.kind() == fsm_parser::SyntaxKind::Ident && !wrote_name {
                    w.write(t.text());
                    wrote_name = true;
                }
            }
            NodeOrToken::Node(n) if n.kind() == SyntaxKind::ARG_LIST => {
                emit_arg_list(w, &n);
            }
            _ => {}
        }
    }
}

fn emit_send(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("send ");
    let idents: Vec<String> = node
        .children_with_tokens()
        .filter_map(|c| match c {
            NodeOrToken::Token(t) if t.kind() == SyntaxKind::Ident => Some(t.text().to_string()),
            _ => None,
        })
        .collect();
    let event = idents.first().cloned().unwrap_or_default();
    let target = idents.get(1).cloned().unwrap_or_default();
    w.write(&event);
    if let Some(arg_list) = node.children().find(|n| n.kind() == SyntaxKind::ARG_LIST) {
        emit_arg_list(w, &arg_list);
    }
    w.write(" to ");
    w.write(&target);
}

fn emit_defer(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("defer ");
    for child in node.children_with_tokens() {
        if let NodeOrToken::Token(t) = child {
            if t.kind() == SyntaxKind::Ident {
                w.write(t.text());
                break;
            }
        }
    }
}

fn emit_if(w: &mut FormatWriter, node: &SyntaxNode) {
    // The shape is: KwIf-text-ident, LParen, expr, RParen, ACTION_BLOCK,
    // [STMT_ELSE]. We can identify the parts by node kind / position.
    let exprs: Vec<SyntaxNode> = node.children().filter(|n| is_expr_kind(n.kind())).collect();
    let blocks: Vec<SyntaxNode> = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
        .collect();

    w.write("if (");
    if let Some(cond) = exprs.first() {
        emit_expr(w, cond);
    }
    w.write(") ");
    if let Some(then_block) = blocks.first() {
        emit_action_block(w, then_block, /* inline_threshold = */ 0);
    }
    if let Some(else_node) = node.children().find(|n| n.kind() == SyntaxKind::STMT_ELSE) {
        emit_else(w, &else_node);
    }
}

fn emit_else(w: &mut FormatWriter, node: &SyntaxNode) {
    // `else { … }` or `else if (…) { … }`.
    w.write(" else");
    let nested_if = node.children().find(|n| n.kind() == SyntaxKind::STMT_IF);
    if let Some(nested_if) = nested_if {
        w.space();
        emit_if(w, &nested_if);
        return;
    }
    if let Some(block) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
    {
        w.space();
        emit_action_block(w, &block, 0);
    }
}

fn emit_while(w: &mut FormatWriter, node: &SyntaxNode) {
    let exprs: Vec<SyntaxNode> = node.children().filter(|n| is_expr_kind(n.kind())).collect();
    w.write("while (");
    if let Some(c) = exprs.first() {
        emit_expr(w, c);
    }
    w.write(") ");
    if let Some(block) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
    {
        emit_action_block(w, &block, 0);
    }
}

fn emit_for(w: &mut FormatWriter, node: &SyntaxNode) {
    let assigns: Vec<SyntaxNode> = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::STMT_ASSIGN)
        .collect();
    let cond = node.children().find(|n| is_expr_kind(n.kind()));
    w.write("for (");
    if let Some(init) = assigns.first() {
        emit_assign(w, init);
    }
    w.write("; ");
    if let Some(c) = cond {
        emit_expr(w, &c);
    }
    w.write("; ");
    if let Some(step) = assigns.get(1) {
        emit_assign(w, step);
    }
    w.write(") ");
    if let Some(block) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
    {
        emit_action_block(w, &block, 0);
    }
}

fn emit_arg_list(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("(");
    let mut first = true;
    for child in node.children() {
        if !first {
            w.write(", ");
        }
        first = false;
        emit_expr(w, &child);
    }
    w.write(")");
}

fn is_expr_kind(k: SyntaxKind) -> bool {
    matches!(
        k,
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
