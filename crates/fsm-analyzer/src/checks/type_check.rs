//! Lightweight type checking pass — guards must be boolean-valued, integer
//! literals must fit, assignments must agree in shape.
//!
//! v1.0 scope per the brief is conservative: we surface the codes that the
//! conformance suite exercises rather than building a full HM-style
//! inference engine. The checks here are structural and fall back to "trust
//! the lowerer" for forms the parser cannot yet shape.

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
// **W0 / Doc 29 §3.4 (R-1 + R-2 + R-3 leave-and-explain).** Retains
// `fsm_parser::cst` for: (R-1) `ty_primitive_name` resolving the type token
// from the type-ref node — sibling of the OPAQUE-BUG-1 parent-node
// resolution; adding a `TypeOrOpaque` typed accessor is a new public type
// on the most behaviourally-critical seam, deferred to a dedicated
// parser-API wave (removing the coupling here risks the P0-1-class
// silent-data-loss regression); (R-2) the document-pre-order
// `machine.syntax().descendants()` STMT_ASSIGN/GUARD_CLAUSE dispatch — the
// unsorted diagnostic vector makes traversal order byte-load-bearing for
// the W0 §4.2 gate, no typed whole-subtree-preorder iterator exists; (R-3)
// `check_assign`/`check_guard`/`resolve_simple_literal` over the
// deliberately-shallow `Expr` CST. A typed accessor layer would relocate —
// not eliminate — these walks and add a large parser surface in a
// 0-new-API-intended wave. Folding any worsens clarity / risks the P0-1
// regression class for zero behaviour gain — Doc 00 §11.44/§11.49, the
// DRIFT-2 `LineIndex` precedent. Left-and-explained.
use fsm_parser::cst::{SyntaxKind, SyntaxNode};

use crate::scope::Scope;
use crate::symbol_table::SymbolTable;
use crate::util::{parse_int_literal_i128, span_of};

/// Run type-check pass.
pub fn check(file: &ast::File, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for (m_idx, machine) in file.machines().enumerate() {
        let scope = Scope::for_machine(m_idx);
        check_context_defaults(&machine, st, &scope, out);
        for descendant in machine.syntax().descendants() {
            match descendant.kind() {
                SyntaxKind::STMT_ASSIGN => check_assign(&descendant, &scope, st, out),
                SyntaxKind::GUARD_CLAUSE => check_guard(&descendant, &scope, st, out),
                _ => {}
            }
        }
    }
}

/// Default values on context fields must agree with the field's declared
/// type — Doc 10 §6 (E0201) / Doc 10 §E0208 for negative-into-unsigned.
fn check_context_defaults(
    m: &ast::MachineDecl,
    _st: &SymbolTable,
    _scope: &Scope,
    out: &mut Vec<Diagnostic>,
) {
    let Some(ctx) = m.context() else { return };
    for field in ctx.fields() {
        let Some(default) = field.default() else {
            continue;
        };
        let Some(ty_text) = field.ty().and_then(|t| ty_primitive_name(t.syntax())) else {
            continue;
        };
        let Some(value) = resolve_simple_literal(default.syntax()) else {
            continue;
        };
        if is_unsigned(&ty_text) && value < 0 {
            out.push(
                Diagnostic::new(DiagnosticCode::E0208, span_of(default.syntax())).with_message(
                    format!(
                        "negative value {value} cannot be assigned to unsigned context field '{}': {ty_text}",
                        field.name().unwrap_or_default()
                    ),
                ),
            );
            continue;
        }
        if let Some(max) = max_for_primitive(&ty_text) {
            if value > max {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0206, span_of(default.syntax()))
                        .with_message(format!("default value {value} overflows {ty_text}")),
                );
            }
        }
    }
}

/// Field assignment shape: LHS must be `ctx.field`. Payload fields are
/// read-only — Doc 10 §E0204.
fn check_assign(node: &SyntaxNode, scope: &Scope, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    // First expression child = LHS, second = RHS (parser keeps them in this
    // order). LHS must be EXPR_FIELD_REF starting with `ctx`.
    let children: Vec<_> = node.children().collect();
    if children.is_empty() {
        return;
    }
    let lhs = &children[0];
    if lhs.kind() != SyntaxKind::EXPR_FIELD_REF {
        out.push(
            Diagnostic::new(DiagnosticCode::E0205, span_of(node))
                .with_message("invalid left-hand side of assignment — expected `ctx.field`"),
        );
        return;
    }
    // Lhs shape: <prefix-node> . Ident — prefix-node is EXPR_NAME_REF holding
    // `ctx`/`payload`/etc. The trailing direct Ident token is the field name.
    let lhs_prefix = lhs
        .children()
        .next()
        .and_then(|child| {
            child
                .descendants_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| t.kind() == SyntaxKind::Ident)
                .map(|t| t.text().to_string())
        })
        .unwrap_or_default();
    let lhs_field = lhs
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    let idents = if !lhs_prefix.is_empty() && !lhs_field.is_empty() {
        vec![lhs_prefix, lhs_field]
    } else {
        Vec::<String>::new()
    };
    if idents.first().map(String::as_str) == Some("payload") {
        out.push(
            Diagnostic::new(DiagnosticCode::E0204, span_of(node))
                .with_message("payload fields are read-only"),
        );
        return;
    }
    if idents.first().map(String::as_str) == Some("ctx") {
        if let Some(field_name) = idents.get(1) {
            // Field existence already checked in name resolution; we
            // additionally narrow type compatibility for literal RHS.
            // `_field`-bound only to confirm the field exists; the body
            // looks up its type via `field_type_text` below.
            if let (Some(rhs), Some(_field)) =
                (children.get(1), st.resolve_context_field(field_name, scope))
            {
                let Some(rhs_val) = resolve_simple_literal(rhs) else {
                    return;
                };
                let Some(ty) = field_type_text(scope, st, field_name) else {
                    return;
                };
                if is_unsigned(&ty) && rhs_val < 0 {
                    out.push(
                        Diagnostic::new(DiagnosticCode::E0201, span_of(node)).with_message(
                            format!(
                                "cannot assign negative literal {rhs_val} to unsigned field '{field_name}': {ty}"
                            ),
                        ),
                    );
                }
                if let Some(max) = max_for_primitive(&ty) {
                    if rhs_val > max {
                        out.push(
                            Diagnostic::new(DiagnosticCode::E0201, span_of(node)).with_message(
                                format!("literal {rhs_val} overflows field '{field_name}': {ty}"),
                            ),
                        );
                    }
                }
            }
        }
    }
}

/// Guards are limited to comparison/logical expressions producing `bool`.
/// We catch the most obvious shape error: a guard whose top-level expression
/// is a bare arithmetic operation (no comparison) — E0200.
fn check_guard(node: &SyntaxNode, _scope: &Scope, _st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    let Some(g) = ast::GuardClause::cast(node.clone()) else {
        return;
    };
    let Some(expr) = g.expr() else { return };
    // `else` catch-all is always permitted.
    if matches!(expr, ast::Expr::GuardElse(_)) {
        return;
    }
    // A bare integer literal is not boolean.
    if let ast::Expr::Literal(lit) = &expr {
        // Allow `true` / `false` keywords (TokenKind::KwTrue/KwFalse) which
        // are stored as EXPR_LITERAL too. Check the inner token.
        let has_int = lit
            .syntax()
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .any(|t| matches!(t.kind(), SyntaxKind::IntLiteral | SyntaxKind::FloatLiteral));
        if has_int {
            out.push(
                Diagnostic::new(DiagnosticCode::E0200, span_of(node))
                    .with_message("guard expression must be boolean, found numeric literal"),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ty_primitive_name(ty_ref: &SyntaxNode) -> Option<String> {
    ty_ref
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| {
            matches!(
                t.kind(),
                SyntaxKind::KwBool
                    | SyntaxKind::KwU8
                    | SyntaxKind::KwU16
                    | SyntaxKind::KwU32
                    | SyntaxKind::KwU64
                    | SyntaxKind::KwI8
                    | SyntaxKind::KwI16
                    | SyntaxKind::KwI32
                    | SyntaxKind::KwI64
                    | SyntaxKind::KwF32
                    | SyntaxKind::KwF64
                    | SyntaxKind::Ident
            )
        })
        .map(|t| t.text().to_string())
}

fn field_type_text(scope: &Scope, st: &SymbolTable, name: &str) -> Option<String> {
    st.context_field_type(name, scope).map(String::from)
}

fn resolve_simple_literal(expr: &SyntaxNode) -> Option<i128> {
    // Walk through EXPR_LITERAL / EXPR_UNARY / EXPR_PAREN — same recursion
    // shape as `timer::resolve_expr_value` but returns i128 to defer overflow
    // decisions to the caller.
    let kind = expr.kind();
    match kind {
        SyntaxKind::EXPR_LITERAL => {
            let tok = expr
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| matches!(t.kind(), SyntaxKind::IntLiteral | SyntaxKind::FloatLiteral))?;
            parse_int_literal_i128(tok.text())
        }
        SyntaxKind::EXPR_UNARY => {
            let op = expr
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| matches!(t.kind(), SyntaxKind::Minus | SyntaxKind::Plus))?;
            let inner = expr.children().next()?;
            let v = resolve_simple_literal(&inner)?;
            match op.kind() {
                SyntaxKind::Minus => Some(-v),
                _ => Some(v),
            }
        }
        SyntaxKind::EXPR_PAREN => {
            let inner = expr.children().next()?;
            resolve_simple_literal(&inner)
        }
        SyntaxKind::CONST_EXPR => {
            let inner = expr.children().next()?;
            resolve_simple_literal(&inner)
        }
        _ => None,
    }
}

fn is_unsigned(ty: &str) -> bool {
    matches!(ty, "u8" | "u16" | "u32" | "u64")
}

fn max_for_primitive(ty: &str) -> Option<i128> {
    Some(match ty {
        "u8" => u8::MAX as i128,
        "u16" => u16::MAX as i128,
        "u32" => u32::MAX as i128,
        "u64" => u64::MAX as i128,
        "i8" => i8::MAX as i128,
        "i16" => i16::MAX as i128,
        "i32" => i32::MAX as i128,
        "i64" => i64::MAX as i128,
        _ => return None,
    })
}
