//! Expression formatter.
//!
//! Walks the Pratt-shaped expression subtree (`EXPR_BINARY`, `EXPR_UNARY`,
//! `EXPR_CAST`, `EXPR_CALL`, `EXPR_FIELD_REF`, `EXPR_NAME_REF`,
//! `EXPR_LITERAL`, `EXPR_PAREN`, `EXPR_QUALIFIED_NAME`, `GUARD_ELSE`).
//!
//! ## Paren stripping
//!
//! Doc 04 §8.7.1 defines a 10-level precedence table. The parser emits an
//! `EXPR_PAREN` node wherever the user wrote `(…)`; the formatter keeps
//! parens only when removing them would change the AST shape (i.e. they
//! are *necessary* for the surrounding context) or when they wrap an
//! operator at a lower binding power than its parent.
//!
//! Algorithm:
//!
//! 1. If the paren's inner expression is a primary (literal, name,
//!    field ref, call) — strip; primaries don't need grouping.
//! 2. If the paren is the LHS or RHS of a binary operator and the inner
//!    binary's lbp >= the parent's lbp, strip. The parser will re-bind
//!    correctly on re-parse.
//! 3. Otherwise keep — playing safe.
//!
//! Reading the parser's exact precedence table from each emit-site is
//! tedious; we centralise the lookup in `binary_lbp`.

use fsm_parser::{SyntaxKind, SyntaxNode};
use rowan::NodeOrToken;

use super::writer::FormatWriter;

/// Public entry — format any expression subtree.
pub(crate) fn emit_expr(w: &mut FormatWriter, node: &SyntaxNode) {
    emit_with_parent_bp(
        w, node, /* parent_lbp = */ 0, /* on_rhs = */ false,
    );
}

fn emit_with_parent_bp(w: &mut FormatWriter, node: &SyntaxNode, parent_lbp: u8, on_rhs: bool) {
    match node.kind() {
        SyntaxKind::EXPR_LITERAL | SyntaxKind::EXPR_NAME_REF => {
            emit_atom(w, node);
        }
        SyntaxKind::EXPR_QUALIFIED_NAME => {
            emit_atom(w, node);
        }
        SyntaxKind::EXPR_FIELD_REF => {
            emit_field_ref(w, node);
        }
        SyntaxKind::EXPR_CALL => {
            emit_call(w, node);
        }
        SyntaxKind::EXPR_UNARY => {
            emit_unary(w, node);
        }
        SyntaxKind::EXPR_CAST => {
            emit_cast(w, node);
        }
        SyntaxKind::EXPR_BINARY => {
            emit_binary(w, node, parent_lbp, on_rhs);
        }
        SyntaxKind::EXPR_PAREN => {
            emit_paren(w, node, parent_lbp, on_rhs);
        }
        SyntaxKind::GUARD_ELSE => {
            w.write("else");
        }
        _ => {
            // ERROR_NODE / unknown — verbatim re-emit so we never lose
            // user text on partially-parsed input.
            re_emit_raw(w, node);
        }
    }
}

fn emit_atom(w: &mut FormatWriter, node: &SyntaxNode) {
    for child in node.children_with_tokens() {
        if let NodeOrToken::Token(tok) = child {
            if !tok.kind().is_trivia() {
                w.write(tok.text());
            }
        }
    }
}

fn emit_field_ref(w: &mut FormatWriter, node: &SyntaxNode) {
    // The CST shape is `EXPR_FIELD_REF` containing the receiver
    // (NameRef/FieldRef), a Dot token, then the ident — re-emit
    // concatenated with `.` (no spaces).
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(n) => emit_with_parent_bp(w, &n, /* parent_lbp = */ 10, false),
            NodeOrToken::Token(tok) if !tok.kind().is_trivia() => {
                w.write(tok.text());
            }
            _ => {}
        }
    }
}

fn emit_call(w: &mut FormatWriter, node: &SyntaxNode) {
    // EXPR_CALL := callee (NameRef / FieldRef) , ARG_LIST.
    let mut first = true;
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(n) if n.kind() == SyntaxKind::ARG_LIST => {
                emit_arg_list(w, &n);
            }
            NodeOrToken::Node(n) => {
                if !first {
                    // Defensive: should not happen — the parser only puts
                    // one callee node before the arg list.
                }
                first = false;
                emit_with_parent_bp(w, &n, 10, false);
            }
            _ => {}
        }
    }
}

fn emit_arg_list(w: &mut FormatWriter, node: &SyntaxNode) {
    // Format args inline: `(a, b, c)`. Multi-line argument breaking is a
    // future enhancement (Doc 19 §13.3 hints at it for fork targets).
    w.write("(");
    let mut first = true;
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(arg) => {
                if !first {
                    w.write(", ");
                }
                first = false;
                emit_with_parent_bp(w, &arg, 0, false);
            }
            NodeOrToken::Token(tok) => {
                // Commas / LParen / RParen handled implicitly by us.
                // Trivia inside arg lists is dropped — Doc 19 §3.4 / §3.7
                // would otherwise reintroduce nondeterminism.
                let _ = tok;
            }
        }
    }
    w.write(")");
}

fn emit_unary(w: &mut FormatWriter, node: &SyntaxNode) {
    // Operator token first, then operand.
    let mut wrote_op = false;
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Token(tok) if !tok.kind().is_trivia() => {
                w.write(tok.text());
                wrote_op = true;
            }
            NodeOrToken::Node(n) => {
                emit_with_parent_bp(w, &n, /* parent_lbp = */ 7, false);
            }
            _ => {}
        }
        if wrote_op {
            // Some grammars want a space after the unary operator;
            // FSM-Lang's `!x` / `-x` are tight per Doc 19 §10 examples.
            // Keep tight.
        }
    }
}

fn emit_cast(w: &mut FormatWriter, node: &SyntaxNode) {
    // operand , `as` , type_ref. Spaces around `as`.
    let mut wrote_operand = false;
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(n)
                if n.kind() == SyntaxKind::TYPE_REF || n.kind() == SyntaxKind::OPAQUE_TYPE_REF =>
            {
                w.write(" as ");
                emit_type_ref(w, &n);
            }
            NodeOrToken::Node(n) => {
                if !wrote_operand {
                    emit_with_parent_bp(w, &n, 8, false);
                    wrote_operand = true;
                } else {
                    // Defensive — shouldn't happen.
                    emit_with_parent_bp(w, &n, 8, false);
                }
            }
            NodeOrToken::Token(tok) => {
                // Skip the `as` keyword token — we emit our own
                // canonical spacing above when we see the type ref.
                let _ = tok;
            }
        }
    }
}

pub(crate) fn emit_type_ref(w: &mut FormatWriter, node: &SyntaxNode) {
    match node.kind() {
        SyntaxKind::OPAQUE_TYPE_REF => {
            // `opaque "C_type"`.
            let mut wrote_kw = false;
            for child in node.children_with_tokens() {
                if let NodeOrToken::Token(tok) = child {
                    if tok.kind().is_trivia() {
                        continue;
                    }
                    if !wrote_kw {
                        w.write(tok.text());
                        wrote_kw = true;
                    } else {
                        w.space();
                        w.write(tok.text());
                    }
                }
            }
        }
        _ => {
            // Plain primitive ident OR enum-qualified-name like Foo.Bar.
            for child in node.children_with_tokens() {
                if let NodeOrToken::Token(tok) = child {
                    if !tok.kind().is_trivia() {
                        w.write(tok.text());
                    }
                }
            }
        }
    }
}

fn emit_binary(w: &mut FormatWriter, node: &SyntaxNode, parent_lbp: u8, on_rhs: bool) {
    let (_, _) = (parent_lbp, on_rhs);
    // CST shape: <lhs node> <op token> <rhs node>.
    let mut lhs_emitted = false;
    let mut op_text: Option<String> = None;
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(n) => {
                if !lhs_emitted {
                    // First child: lhs. We don't yet know our own op so
                    // we cannot pass the right `parent_lbp` to the child
                    // — but the only effect of `parent_lbp` is paren
                    // stripping in EXPR_PAREN, which conservatively keeps
                    // parens when ambiguous. Use 0 here; on the right
                    // operand we use the op's rbp after we know it.
                    emit_with_parent_bp(w, &n, 0, false);
                    lhs_emitted = true;
                } else {
                    // RHS — needs the op's binding power.
                    let op = op_text.as_deref().unwrap_or("");
                    let rbp = binary_rbp_for_op(op);
                    if let Some(op_t) = op_text.as_ref() {
                        w.space();
                        w.write(op_t);
                        w.space();
                    }
                    emit_with_parent_bp(w, &n, rbp, true);
                    op_text = None;
                }
            }
            NodeOrToken::Token(tok) if !tok.kind().is_trivia() => {
                op_text = Some(tok.text().to_string());
            }
            _ => {}
        }
    }
}

/// Lookup the left-binding-power of a binary operator, in the units of
/// Doc 04 §8.7.1 (higher binds tighter).
pub(crate) fn binary_lbp_for_op(op: &str) -> u8 {
    match op {
        "||" => 0,
        "&&" => 1,
        "==" | "!=" | "<" | ">" | "<=" | ">=" => 2,
        "&" | "^" | "|" => 3,
        "<<" | ">>" => 4,
        "+" | "-" => 5,
        "*" | "/" => 6,
        "%" => 6,
        _ => 0,
    }
}

fn binary_rbp_for_op(op: &str) -> u8 {
    // Left-associative everywhere — rbp = lbp + 1.
    let lbp = binary_lbp_for_op(op);
    lbp.saturating_add(1)
}

fn emit_paren(w: &mut FormatWriter, node: &SyntaxNode, parent_lbp: u8, on_rhs: bool) {
    // Find the inner non-trivia non-token node.
    let mut inner: Option<SyntaxNode> = None;
    for child in node.children_with_tokens() {
        if let NodeOrToken::Node(n) = child {
            inner = Some(n);
            break;
        }
    }
    let inner = match inner {
        Some(i) => i,
        None => {
            // Empty paren — should not happen in valid input. Re-emit
            // verbatim.
            re_emit_raw(w, node);
            return;
        }
    };

    if can_strip_paren(&inner, parent_lbp, on_rhs) {
        emit_with_parent_bp(w, &inner, parent_lbp, on_rhs);
    } else {
        w.write("(");
        emit_with_parent_bp(w, &inner, 0, false);
        w.write(")");
    }
}

/// Decide whether `inner` no longer needs the surrounding parens given
/// the parent expression's binding power.
///
/// Rules (intentionally conservative — over-keeping parens is harmless;
/// over-stripping them changes the AST):
///
/// - Primaries and field refs / calls / casts / unaries never need parens
///   when nested inside a binary expression — they have higher precedence
///   than every binary op.
/// - Binary inside binary: strip iff the inner op's lbp > parent_lbp
///   (strictly greater — equal lbp is left-associative; on the RHS we
///   compare against lbp+1 effectively).
/// - At the top of an expression (parent_lbp == 0), strip if the inner
///   is anything but a binary at lbp 0.
fn can_strip_paren(inner: &SyntaxNode, parent_lbp: u8, _on_rhs: bool) -> bool {
    match inner.kind() {
        SyntaxKind::EXPR_LITERAL
        | SyntaxKind::EXPR_NAME_REF
        | SyntaxKind::EXPR_QUALIFIED_NAME
        | SyntaxKind::EXPR_FIELD_REF
        | SyntaxKind::EXPR_CALL
        | SyntaxKind::EXPR_UNARY
        | SyntaxKind::EXPR_CAST => true,
        SyntaxKind::EXPR_PAREN => true, // `((x))` collapses to `x`.
        SyntaxKind::EXPR_BINARY => {
            // Inspect the inner op.
            let inner_op = inner.children_with_tokens().find_map(|c| match c {
                NodeOrToken::Token(t) if !t.kind().is_trivia() => Some(t.text().to_string()),
                _ => None,
            });
            let inner_lbp = match inner_op.as_deref() {
                Some(op) => binary_lbp_for_op(op),
                None => return false,
            };
            // Strip if inner binds at least as tight as the parent.
            // Doc convention: parens needed when inner_lbp < parent_lbp.
            // Equal lbp: left-assoc means `(a + b) + c` == `a + b + c`,
            // safe to strip ON THE LEFT. ON THE RIGHT, `a + (b + c)`
            // re-parses to `a + b + c` (same value for `+`) but for
            // non-commutative ops would change semantics. We over-keep
            // parens in this case for safety.
            inner_lbp > parent_lbp
        }
        _ => false,
    }
}

/// Verbatim raw text re-emit — used for ERROR_NODE / unknown subtrees so
/// we never accidentally swallow source. Inserts no extra trivia.
fn re_emit_raw(w: &mut FormatWriter, node: &SyntaxNode) {
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Token(tok) if !tok.kind().is_trivia() => {
                w.write(tok.text());
            }
            NodeOrToken::Node(n) => re_emit_raw(w, &n),
            _ => {}
        }
    }
}

/// Iterate non-trivia tokens of `node` (top-level, not recursive).
pub(crate) fn iter_tokens(node: &SyntaxNode) -> impl Iterator<Item = fsm_parser::SyntaxToken> + '_ {
    node.children_with_tokens().filter_map(|c| match c {
        NodeOrToken::Token(t) if !t.kind().is_trivia() => Some(t),
        _ => None,
    })
}

/// Find the first non-trivia keyword/ident token. Used by callers that
/// only need the surface text (e.g. enum names).
pub(crate) fn first_ident(node: &SyntaxNode) -> Option<String> {
    iter_tokens(node)
        .find(|t| {
            matches!(
                t.kind(),
                SyntaxKind::Ident | SyntaxKind::KwTrue | SyntaxKind::KwFalse
            )
        })
        .map(|t| t.text().to_string())
}

/// Helper exported for the state module — the after/every timer's value
/// expression. Always a single `CONST_EXPR` child.
pub(crate) fn emit_const_expr(w: &mut FormatWriter, node: &SyntaxNode) {
    for child in node.children_with_tokens() {
        if let NodeOrToken::Node(n) = child {
            emit_with_parent_bp(w, &n, 0, false);
        }
    }
}

/// Type ref also exists outside expressions; re-exported entry.
pub(crate) fn emit_type_ref_public(w: &mut FormatWriter, node: &SyntaxNode) {
    emit_type_ref(w, node);
}

// Tiny unit tests for the precedence helpers — these don't require a
// parser hookup.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_lbp_matches_doc_04_8_7_1() {
        assert_eq!(binary_lbp_for_op("||"), 0);
        assert_eq!(binary_lbp_for_op("&&"), 1);
        assert_eq!(binary_lbp_for_op("=="), 2);
        assert_eq!(binary_lbp_for_op("&"), 3);
        assert_eq!(binary_lbp_for_op("<<"), 4);
        assert_eq!(binary_lbp_for_op("+"), 5);
        assert_eq!(binary_lbp_for_op("*"), 6);
    }
}
