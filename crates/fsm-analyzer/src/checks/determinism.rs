//! Determinism analysis — Doc 10 §7 / Doc 08 §4.2.
//!
//! Two transitions from the same source state with the same trigger event
//! must be mutually exclusive (statically). This pass:
//!
//! - groups transitions by `(source_state, trigger_event)`;
//! - if a group has ≥2 unguarded transitions, emits `FSM-E0300`;
//! - if a group has ≥2 guarded transitions whose guards are not provably
//!   disjoint, emits `FSM-E0300` unless every transition carries an
//!   explicit `priority` clause (in which case `FSM-W0300` is emitted
//!   instead);
//! - if exactly one transition is `[else]`, every other guard is considered
//!   sufficient and no E0300 fires;
//! - if a transition has a guard that is the literal `true` or `false`,
//!   `FSM-W0603` is emitted (constant guard).
//!
//! Exact static disjointness is undecidable in general; we model "obviously
//! exclusive" forms: `[A == X]` vs `[A == Y]` where `X != Y`, and `[A < N]`
//! vs `[A >= N]`. Anything else we conservatively flag as overlapping.

use std::collections::HashMap;

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
use fsm_parser::cst::{SyntaxKind, SyntaxNode};

use crate::symbol_table::SymbolTable;
use crate::util::{parse_int_literal_i128, span_of, walk_all_states};

/// Run determinism analysis. Operates on the AST directly so each emitted
/// diagnostic points at the offending transition span.
pub fn check(file: &ast::File, _st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for m in file.machines() {
        for state in walk_all_states(&m) {
            let groups = collect_transitions(&state);
            for transitions in groups.values() {
                if transitions.len() < 2 {
                    if let Some(t) = transitions.first() {
                        warn_constant_guard(t, out);
                    }
                    continue;
                }
                analyze_group(transitions, out);
            }
            for transitions in groups.values() {
                for t in transitions {
                    warn_constant_guard(t, out);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Trans {
    span: fsm_diagnostics::Span,
    guard: Option<GuardShape>,
    priority: Option<i64>,
    /// Original syntax node — kept for future related-info reporting.
    #[allow(dead_code)]
    node: SyntaxNode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum GuardShape {
    /// Literal `true` / `false`.
    Const(bool),
    /// `[else]` catch-all.
    Else,
    /// Field-equality `ctx.X == literal`.
    Eq { field: String, value: String },
    /// Field-inequality `ctx.X != literal`.
    Ne { field: String, value: String },
    /// Field-comparison `ctx.X < literal` (etc.).
    Cmp {
        field: String,
        op: CmpOp,
        value: i128,
    },
    /// Anything else.
    Opaque,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
}

fn collect_transitions(state: &ast::StateDecl) -> HashMap<(String, String), Vec<Trans>> {
    let mut out: HashMap<(String, String), Vec<Trans>> = HashMap::new();
    let src = state.name().unwrap_or_default();
    for t in state.transitions() {
        if let Some(ev) = t.trigger() {
            out.entry((src.clone(), ev)).or_default().push(make_trans(
                t.guard(),
                t.priority(),
                t.syntax(),
            ));
        }
    }
    for t in state.internal_transitions() {
        if let Some(ev) = t.trigger() {
            out.entry((src.clone(), ev)).or_default().push(make_trans(
                t.guard(),
                t.priority(),
                t.syntax(),
            ));
        }
    }
    for t in state.local_transitions() {
        if let Some(ev) = t.trigger() {
            out.entry((src.clone(), ev)).or_default().push(make_trans(
                t.guard(),
                t.priority(),
                t.syntax(),
            ));
        }
    }
    out
}

fn make_trans(
    guard: Option<ast::GuardClause>,
    priority: Option<ast::PriorityClause>,
    node: &SyntaxNode,
) -> Trans {
    Trans {
        span: span_of(node),
        guard: guard.map(|g| classify_guard(&g)),
        priority: priority.and_then(|p| extract_priority(p.syntax())),
        node: node.clone(),
    }
}

fn classify_guard(g: &ast::GuardClause) -> GuardShape {
    let Some(expr) = g.expr() else {
        return GuardShape::Opaque;
    };
    match expr {
        ast::Expr::GuardElse(_) => GuardShape::Else,
        ast::Expr::Literal(lit) => {
            // Find true/false keyword tokens.
            for el in lit.syntax().children_with_tokens() {
                if let Some(t) = el.into_token() {
                    if t.kind() == SyntaxKind::KwTrue {
                        return GuardShape::Const(true);
                    }
                    if t.kind() == SyntaxKind::KwFalse {
                        return GuardShape::Const(false);
                    }
                }
            }
            GuardShape::Opaque
        }
        ast::Expr::Binary(b) => classify_binary(b.syntax()),
        _ => GuardShape::Opaque,
    }
}

fn classify_binary(bin: &SyntaxNode) -> GuardShape {
    // Children: lhs node + op token + rhs node.
    let mut child_nodes = bin.children();
    let lhs = child_nodes.next();
    let rhs = child_nodes.next();
    let op = bin
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| is_op_token(t.kind()));
    let (Some(lhs), Some(rhs), Some(op)) = (lhs, rhs, op) else {
        return GuardShape::Opaque;
    };
    let field = lhs_field(&lhs);
    let lit = literal_value(&rhs);
    match (field, lit, op.kind()) {
        (Some(field), Some(LitVal::Int(value)), SyntaxKind::EqEq) => GuardShape::Eq {
            field,
            value: value.to_string(),
        },
        (Some(field), Some(LitVal::Int(value)), SyntaxKind::BangEq) => GuardShape::Ne {
            field,
            value: value.to_string(),
        },
        (Some(field), Some(LitVal::Int(value)), SyntaxKind::Lt) => GuardShape::Cmp {
            field,
            op: CmpOp::Lt,
            value,
        },
        (Some(field), Some(LitVal::Int(value)), SyntaxKind::Le) => GuardShape::Cmp {
            field,
            op: CmpOp::Le,
            value,
        },
        (Some(field), Some(LitVal::Int(value)), SyntaxKind::Gt) => GuardShape::Cmp {
            field,
            op: CmpOp::Gt,
            value,
        },
        (Some(field), Some(LitVal::Int(value)), SyntaxKind::Ge) => GuardShape::Cmp {
            field,
            op: CmpOp::Ge,
            value,
        },
        (Some(field), Some(LitVal::Ident(v)), SyntaxKind::EqEq) => {
            GuardShape::Eq { field, value: v }
        }
        (Some(field), Some(LitVal::Ident(v)), SyntaxKind::BangEq) => {
            GuardShape::Ne { field, value: v }
        }
        _ => GuardShape::Opaque,
    }
}

fn is_op_token(k: SyntaxKind) -> bool {
    matches!(
        k,
        SyntaxKind::EqEq
            | SyntaxKind::BangEq
            | SyntaxKind::Lt
            | SyntaxKind::Le
            | SyntaxKind::Gt
            | SyntaxKind::Ge
            | SyntaxKind::AmpAmp
            | SyntaxKind::PipePipe
    )
}

fn lhs_field(node: &SyntaxNode) -> Option<String> {
    if node.kind() == SyntaxKind::EXPR_FIELD_REF {
        // LHS lives inside a child node (EXPR_NAME_REF); the trailing Ident
        // token is the field name.
        let lhs = node.children().next().and_then(|child| {
            child
                .descendants_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| t.kind() == SyntaxKind::Ident)
                .map(|t| t.text().to_string())
        })?;
        let rhs = node
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| t.kind() == SyntaxKind::Ident)?
            .text()
            .to_string();
        return Some(format!("{lhs}.{rhs}"));
    }
    None
}

enum LitVal {
    Int(i128),
    Ident(String),
}

fn literal_value(node: &SyntaxNode) -> Option<LitVal> {
    match node.kind() {
        SyntaxKind::EXPR_LITERAL => {
            let tok = node
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| matches!(t.kind(), SyntaxKind::IntLiteral | SyntaxKind::FloatLiteral))?;
            let v = parse_int_literal_i128(tok.text())?;
            Some(LitVal::Int(v))
        }
        SyntaxKind::EXPR_NAME_REF => {
            let tok = node
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| t.kind() == SyntaxKind::Ident)?;
            Some(LitVal::Ident(tok.text().to_string()))
        }
        SyntaxKind::EXPR_FIELD_REF => {
            // EnumName.Variant — encode as `EnumName.Variant`.
            let lhs = node.children().next().and_then(|child| {
                child
                    .descendants_with_tokens()
                    .filter_map(|el| el.into_token())
                    .find(|t| t.kind() == SyntaxKind::Ident)
                    .map(|t| t.text().to_string())
            })?;
            let rhs = node
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| t.kind() == SyntaxKind::Ident)?
                .text()
                .to_string();
            Some(LitVal::Ident(format!("{lhs}.{rhs}")))
        }
        _ => None,
    }
}

fn extract_priority(node: &SyntaxNode) -> Option<i64> {
    let tok = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::IntLiteral)?;
    let s: String = tok.text().chars().filter(|c| *c != '_').collect();
    s.parse().ok()
}

fn analyze_group(transitions: &[Trans], out: &mut Vec<Diagnostic>) {
    // Distinct priorities => W0300 acknowledgement, no E0300.
    let all_have_priority = transitions.iter().all(|t| t.priority.is_some());
    let distinct_priorities = {
        let mut ps: Vec<_> = transitions.iter().filter_map(|t| t.priority).collect();
        ps.sort_unstable();
        ps.dedup();
        ps.len() == transitions.iter().filter(|t| t.priority.is_some()).count()
    };
    let has_else = transitions
        .iter()
        .any(|t| matches!(t.guard, Some(GuardShape::Else)));

    if all_have_priority && distinct_priorities {
        // Acknowledge the priority resolution but warn.
        for t in transitions {
            out.push(
                Diagnostic::new(DiagnosticCode::W0300, t.span)
                    .with_message("transition conflict resolved by priority"),
            );
        }
        return;
    }

    // Determine which transition pairs overlap.
    let unguarded_count = transitions
        .iter()
        .filter(|t| t.guard.is_none() || matches!(t.guard, Some(GuardShape::Const(true))))
        .count();
    if unguarded_count > 1 {
        for t in transitions
            .iter()
            .filter(|t| t.guard.is_none() || matches!(t.guard, Some(GuardShape::Const(true))))
            .skip(1)
        {
            out.push(
                Diagnostic::new(DiagnosticCode::E0300, t.span)
                    .with_message("multiple unguarded transitions on the same event"),
            );
        }
        return;
    }

    for i in 0..transitions.len() {
        for j in (i + 1)..transitions.len() {
            let a = &transitions[i];
            let b = &transitions[j];
            if has_else
                && (matches!(a.guard, Some(GuardShape::Else))
                    || matches!(b.guard, Some(GuardShape::Else)))
            {
                continue;
            }
            if !guards_disjoint(&a.guard, &b.guard) {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0300, b.span)
                        .with_message("nondeterministic transition conflict — guards may overlap"),
                );
            }
        }
    }
}

fn guards_disjoint(a: &Option<GuardShape>, b: &Option<GuardShape>) -> bool {
    use GuardShape::*;
    match (a, b) {
        (Some(Const(false)), _) | (_, Some(Const(false))) => true,
        (Some(Else), _) | (_, Some(Else)) => true,
        (
            Some(Eq {
                field: fa,
                value: va,
            }),
            Some(Eq {
                field: fb,
                value: vb,
            }),
        ) if fa == fb => va != vb,
        (
            Some(Eq {
                field: fa,
                value: va,
            }),
            Some(Ne {
                field: fb,
                value: vb,
            }),
        ) if fa == fb => va == vb,
        (
            Some(Ne {
                field: fa,
                value: va,
            }),
            Some(Eq {
                field: fb,
                value: vb,
            }),
        ) if fa == fb => va == vb,
        (
            Some(Cmp {
                field: fa,
                op: oa,
                value: va,
            }),
            Some(Cmp {
                field: fb,
                op: ob,
                value: vb,
            }),
        ) if fa == fb => disjoint_intervals(*oa, *va, *ob, *vb),
        _ => false,
    }
}

fn disjoint_intervals(oa: CmpOp, va: i128, ob: CmpOp, vb: i128) -> bool {
    // Translate each constraint to an interval and check non-overlap.
    let (la, ra) = bounds_of(oa, va);
    let (lb, rb) = bounds_of(ob, vb);
    // disjoint if a's right < b's left or vice versa (strict).
    ra < lb || rb < la
}

fn bounds_of(op: CmpOp, value: i128) -> (i128, i128) {
    match op {
        CmpOp::Lt => (i128::MIN, value - 1),
        CmpOp::Le => (i128::MIN, value),
        CmpOp::Gt => (value + 1, i128::MAX),
        CmpOp::Ge => (value, i128::MAX),
    }
}

fn warn_constant_guard(t: &Trans, out: &mut Vec<Diagnostic>) {
    match &t.guard {
        Some(GuardShape::Const(true)) => out.push(
            Diagnostic::new(DiagnosticCode::W0603, t.span)
                .with_message("guard always evaluates to true"),
        ),
        Some(GuardShape::Const(false)) => out.push(
            Diagnostic::new(DiagnosticCode::W0603, t.span)
                .with_message("guard always evaluates to false"),
        ),
        _ => {}
    }
}

// Walker logic moved to `crate::util::walk_all_states` (R2.1 dedup,
// 2026-05-15).
