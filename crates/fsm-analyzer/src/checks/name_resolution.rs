//! Name-resolution checks — Doc 10 §5 (FSM-E0100..FSM-E0109).
//!
//! Walks every transition / completion / fork-join / initial / history-default
//! / send / raise / defer and verifies each referenced identifier resolves to
//! a symbol declared in the current scope.
//!
//! Per Doc 00 G-11, the corrected codes are:
//! - `FSM-E0100` Unknown state reference  (transition target, initial)
//! - `FSM-E0101` Unknown event reference  (trigger, raise, send, defer)
//! - `FSM-E0102` Unknown extern reference (call in action / guard)
//! - `FSM-E0103` Unknown machine reference (cross-machine `send`)
//! - `FSM-E0104` Unknown context field reference (ctx.X)
//! - `FSM-E0106` Non-pure extern used as guard
//! - `FSM-E0107` No initial declaration
//! - `FSM-E0108` Multiple initial declarations
//! - `FSM-E0109` History default references non-existent state

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
// **W0 / Doc 29 §3.4 (R-2 + R-3 leave-and-explain, generalized from the
// lowering-side residuals to this check).** This file deliberately retains
// `fsm_parser::cst` for two reasons that make folding it worse-EV:
//
// 1. **Document-order dispatch (R-2 class).** `check_machine` walks
//    `m.syntax().descendants()` in rowan **document pre-order** and
//    `check_node` dispatches by kind. The emitted diagnostic vector is
//    **unsorted** (`fsm_analyzer::checks::run_all` appends in pass order;
//    `fsm-cli`'s `emit_json_aggregate` emits in collected order — neither
//    sorts), so the *traversal order across heterogeneous node kinds is
//    byte-load-bearing* for the diagnostics stream the W0 §4.2 gate pins.
//    The typed AST exposes per-kind iterators but no single ordered
//    heterogeneous-child / whole-subtree-preorder iterator; reproducing
//    `descendants()` order through typed accessors would need the typed
//    `enum StateChild` ordered iterator Doc 29 §3.4-R-2 explicitly rejects
//    as the refactor-to-number trap (a large new parser API whose only
//    consumers are these document-order walks).
// 2. **Shallow-AST expression name resolution (R-3 class).**
//    `check_expr` / `call_name` / `check_stmt_*` walk the *deliberately
//    shallow* `Expr`/`Stmt` CST (matching `EXPR_FIELD_REF`/`EXPR_CALL`/…,
//    `Dot`/`Ident` tokens). A full typed accessor layer would relocate —
//    not eliminate — these walks into `fsm-parser` (the analyzer would
//    still depend on the shape) and add a large public surface in a
//    0-new-API-intended wave.
//
// Folding either would worsen clarity / risk the P0-1 byte-identity
// regression class for zero behaviour gain — Doc 00 §11.44/§11.49, the
// DRIFT-2 `LineIndex` precedent. Left-and-explained (a success of the C-1
// discipline, not a failure). The clean Archetype-A removals + B-clean
// accessor relocations land elsewhere in W0.
use fsm_parser::cst::{SyntaxKind, SyntaxNode};

use crate::scope::Scope;
use crate::symbol_table::SymbolTable;
use crate::util::span_of;

/// Run name-resolution checks against the AST.
pub fn check(file: &ast::File, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for (m_idx, machine) in file.machines().enumerate() {
        let scope = Scope::for_machine(m_idx);
        check_machine(&machine, &scope, st, out);
    }
}

fn check_machine(m: &ast::MachineDecl, scope: &Scope, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    // Initial declarations -------------------------------------------------
    // W0 (Doc 29 §3.3 "A-with-a-tiny-B-accessor"): typed `initials()`
    // iterator replaces the `children()+kind()` CST walk. `AstChildren<
    // InitialDecl>` is `children().filter_map(InitialDecl::cast)` — the
    // *same elements in the same source order* (rowan child order == source
    // order for same-kind siblings), so `.count()` and the enumerated
    // E0108-at-`span_of` emission are byte-identical. This sub-block is
    // order-safe (single kind); the surrounding descendants-dispatch +
    // expression walks stay the R-2/R-3 residual (see module header).
    let initials: Vec<_> = m.initials().collect();
    let initial_count = initials.len();
    if initial_count == 0 && m.states().count() > 0 {
        // Empty machines (no state) elide the initial — Doc 04 §6.
        out.push(Diagnostic::new(DiagnosticCode::E0107, span_of(m.syntax())));
    } else if initial_count > 1 {
        for (i, c) in initials.iter().enumerate() {
            if i > 0 {
                out.push(Diagnostic::new(DiagnosticCode::E0108, span_of(c.syntax())));
            }
        }
    }

    if let Some(init) = m.initial() {
        if let Some(target) = init.target() {
            if st.resolve_state(&target, scope).is_none() {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0100, span_of(init.syntax()))
                        .with_message(format!("unknown state '{target}' in initial declaration")),
                );
            }
        }
    }

    // Walk the entire machine subtree, dispatching per node kind.
    for child in m.syntax().descendants() {
        check_node(&child, scope, st, out);
    }
}

fn check_node(node: &SyntaxNode, scope: &Scope, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    use fsm_parser::cst::SyntaxKind as K;
    match node.kind() {
        K::INITIAL_DECL => {
            // Nested initial declarations get the same treatment.
            if let Some(init) = ast::InitialDecl::cast(node.clone()) {
                if let Some(target) = init.target() {
                    if st.resolve_state(&target, scope).is_none() {
                        out.push(
                            Diagnostic::new(DiagnosticCode::E0100, span_of(node)).with_message(
                                format!("unknown state '{target}' in initial declaration"),
                            ),
                        );
                    }
                }
            }
        }
        K::TRANSITION_DECL => {
            if let Some(t) = ast::TransitionDecl::cast(node.clone()) {
                check_transition(
                    scope,
                    st,
                    out,
                    t.trigger(),
                    t.target(),
                    t.guard(),
                    t.actions(),
                    node,
                );
            }
        }
        K::LOCAL_DECL => {
            if let Some(t) = ast::LocalDecl::cast(node.clone()) {
                check_transition(
                    scope,
                    st,
                    out,
                    t.trigger(),
                    t.target(),
                    t.guard(),
                    t.actions(),
                    node,
                );
            }
        }
        K::INTERNAL_DECL => {
            if let Some(t) = ast::InternalDecl::cast(node.clone()) {
                check_transition(
                    scope,
                    st,
                    out,
                    t.trigger(),
                    None, // internal has no explicit target
                    t.guard(),
                    t.actions(),
                    node,
                );
            }
        }
        K::COMPLETION_DECL => {
            if let Some(t) = ast::CompletionDecl::cast(node.clone()) {
                // No trigger event; target + optional guard + actions.
                if let Some(target) = t.target() {
                    if st.resolve_state(&target, scope).is_none() {
                        out.push(
                            Diagnostic::new(DiagnosticCode::E0100, span_of(node)).with_message(
                                format!("unknown state '{target}' in completion transition target"),
                            ),
                        );
                    }
                }
                if let Some(g) = t.guard() {
                    check_guard(g, scope, st, out);
                }
                if let Some(a) = t.actions() {
                    check_action_block(&a, scope, st, out);
                }
            }
        }
        K::AFTER_DECL | K::EVERY_DECL => {
            // Target ident is the only `Ident` token among children-with-
            // tokens (after the timer expression). Resolve via AST helper.
            let target = match node.kind() {
                K::AFTER_DECL => ast::AfterDecl::cast(node.clone()).and_then(|n| n.target()),
                K::EVERY_DECL => ast::EveryDecl::cast(node.clone()).and_then(|n| n.target()),
                _ => None,
            };
            if let Some(t) = target {
                if st.resolve_state(&t, scope).is_none() {
                    out.push(
                        Diagnostic::new(DiagnosticCode::E0100, span_of(node))
                            .with_message(format!("unknown state '{t}' in timer transition")),
                    );
                }
            }
            // Action block on the timer.
            for child in node.children() {
                if child.kind() == K::ACTION_BLOCK {
                    if let Some(ab) = ast::ActionBlock::cast(child) {
                        check_action_block(&ab, scope, st, out);
                    }
                }
            }
        }
        K::DEFER_DECL => {
            if let Some(d) = ast::DeferDecl::cast(node.clone()) {
                if let Some(ev) = d.event() {
                    if st.resolve_event(&ev, scope).is_none() {
                        out.push(
                            Diagnostic::new(DiagnosticCode::E0101, span_of(node))
                                .with_message(format!("unknown event '{ev}' in defer")),
                        );
                    }
                }
            }
        }
        K::FORK_DECL => {
            if let Some(f) = ast::ForkDecl::cast(node.clone()) {
                if let Some(targets) = f.targets() {
                    for t in targets.names() {
                        if st.resolve_state(&t, scope).is_none() {
                            out.push(
                                Diagnostic::new(DiagnosticCode::E0100, span_of(node))
                                    .with_message(format!("unknown state '{t}' in fork target")),
                            );
                        }
                    }
                }
            }
        }
        K::JOIN_DECL => {
            if let Some(j) = ast::JoinDecl::cast(node.clone()) {
                if let Some(sources) = j.sources() {
                    for s in sources.names() {
                        if st.resolve_state(&s, scope).is_none() {
                            out.push(
                                Diagnostic::new(DiagnosticCode::E0100, span_of(node))
                                    .with_message(format!("unknown state '{s}' in join source")),
                            );
                        }
                    }
                }
                // Join target ident is captured as the LAST ident token. We
                // rely on the ident-tokens helper inside JoinSources for the
                // sources; the target is appended after the `->`. Check via
                // CST.
                check_join_target(node, scope, st, out);
            }
        }
        K::SHALLOW_HISTORY_DECL | K::DEEP_HISTORY_DECL => {
            // history default — check default target exists per E0109.
            let default = match node.kind() {
                K::SHALLOW_HISTORY_DECL => {
                    ast::ShallowHistoryDecl::cast(node.clone()).and_then(|n| n.default())
                }
                K::DEEP_HISTORY_DECL => {
                    ast::DeepHistoryDecl::cast(node.clone()).and_then(|n| n.default())
                }
                _ => None,
            };
            if let Some(init) = default {
                if let Some(target) = init.target() {
                    if st.resolve_state(&target, scope).is_none() {
                        out.push(
                            Diagnostic::new(DiagnosticCode::E0109, span_of(init.syntax()))
                                .with_message(format!(
                                    "history default references unknown state '{target}'"
                                )),
                        );
                    }
                }
            }
        }
        K::CHOICE_DECL => {
            if let Some(c) = ast::ChoiceDecl::cast(node.clone()) {
                for branch in c.branches() {
                    if let Some(target) = branch.target() {
                        if st.resolve_state(&target, scope).is_none() {
                            out.push(
                                Diagnostic::new(DiagnosticCode::E0100, span_of(branch.syntax()))
                                    .with_message(format!(
                                        "unknown state '{target}' in choice branch"
                                    )),
                            );
                        }
                    }
                }
            }
        }
        K::JUNCTION_DECL => {
            if let Some(c) = ast::JunctionDecl::cast(node.clone()) {
                for branch in c.branches() {
                    if let Some(target) = branch.target() {
                        if st.resolve_state(&target, scope).is_none() {
                            out.push(
                                Diagnostic::new(DiagnosticCode::E0100, span_of(branch.syntax()))
                                    .with_message(format!(
                                        "unknown state '{target}' in junction branch"
                                    )),
                            );
                        }
                    }
                }
            }
        }
        K::ENTRY_POINT_DECL => {
            if let Some(e) = ast::EntryPointDecl::cast(node.clone()) {
                if let Some(target) = e.target() {
                    if st.resolve_state(&target, scope).is_none() {
                        out.push(
                            Diagnostic::new(DiagnosticCode::E0100, span_of(node)).with_message(
                                format!("unknown state '{target}' in entry_point target"),
                            ),
                        );
                    }
                }
            }
        }
        K::STMT_RAISE => check_stmt_raise(node, scope, st, out),
        K::STMT_SEND => check_stmt_send(node, scope, st, out),
        K::STMT_DEFER => check_stmt_defer(node, scope, st, out),
        K::STMT_CALL => check_stmt_call(node, scope, st, out),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn check_transition(
    scope: &Scope,
    st: &SymbolTable,
    out: &mut Vec<Diagnostic>,
    trigger: Option<String>,
    target: Option<String>,
    guard: Option<ast::GuardClause>,
    actions: Option<ast::ActionBlock>,
    node: &SyntaxNode,
) {
    if let Some(ev) = trigger {
        if st.resolve_event(&ev, scope).is_none() {
            out.push(
                Diagnostic::new(DiagnosticCode::E0101, span_of(node))
                    .with_message(format!("unknown event '{ev}' in trigger")),
            );
        }
    }
    if let Some(t) = target {
        if st.resolve_state(&t, scope).is_none() {
            out.push(
                Diagnostic::new(DiagnosticCode::E0100, span_of(node))
                    .with_message(format!("unknown state '{t}' in transition target")),
            );
        }
    }
    if let Some(g) = guard {
        check_guard(g, scope, st, out);
    }
    if let Some(a) = actions {
        check_action_block(&a, scope, st, out);
    }
}

fn check_guard(g: ast::GuardClause, scope: &Scope, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    let Some(expr) = g.expr() else { return };
    for sub in expr.syntax().descendants() {
        if matches!(
            sub.kind(),
            SyntaxKind::EXPR_FIELD_REF
                | SyntaxKind::EXPR_CALL
                | SyntaxKind::EXPR_NAME_REF
                | SyntaxKind::EXPR_QUALIFIED_NAME
        ) {
            check_expr(&sub, scope, st, out, /*in_guard=*/ true);
        }
    }
    // descendants() yields the root node first too — handle it directly so a
    // top-level field-ref is not missed.
    check_expr(expr.syntax(), scope, st, out, /*in_guard=*/ true);
}

fn check_action_block(
    ab: &ast::ActionBlock,
    scope: &Scope,
    st: &SymbolTable,
    out: &mut Vec<Diagnostic>,
) {
    for child in ab.syntax().descendants() {
        check_node(&child, scope, st, out);
        if matches!(
            child.kind(),
            SyntaxKind::EXPR_FIELD_REF
                | SyntaxKind::EXPR_CALL
                | SyntaxKind::EXPR_NAME_REF
                | SyntaxKind::EXPR_QUALIFIED_NAME
        ) {
            check_expr(&child, scope, st, out, /*in_guard=*/ false);
        }
    }
}

/// Validate every name reference inside an expression subtree.
fn check_expr(
    node: &SyntaxNode,
    scope: &Scope,
    st: &SymbolTable,
    out: &mut Vec<Diagnostic>,
    in_guard: bool,
) {
    // Field-ref AST shape per parser/expr.rs:
    //   EXPR_FIELD_REF
    //     <child node holding the LHS prefix>  (usually EXPR_NAME_REF, or
    //                                           a deeper EXPR_FIELD_REF for
    //                                           chained access)
    //     Dot
    //     Ident (the trailing field / variant name)
    if node.kind() == SyntaxKind::EXPR_FIELD_REF {
        // LHS is the first child node — we want the leftmost ident token
        // anywhere in that subtree.
        let lhs = node
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
        // RHS is the direct Ident-token child after the Dot.
        let rhs = node
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| t.kind() == SyntaxKind::Ident)
            .map(|t| t.text().to_string())
            .unwrap_or_default();
        if !lhs.is_empty() && !rhs.is_empty() {
            let (lhs, rhs) = (&lhs, &rhs);
            if lhs == "ctx" {
                if st.resolve_context_field(rhs, scope).is_none() {
                    out.push(
                        Diagnostic::new(DiagnosticCode::E0104, span_of(node))
                            .with_message(format!("unknown context field '{rhs}'")),
                    );
                }
            } else if lhs == "payload" {
                // Payload fields aren't tracked globally — each event owns
                // its own payload schema. We accept them silently here; type
                // checking handles deeper validation.
            } else if let Some(en) = st.file_enums.iter().find(|e| &e.name == lhs) {
                if !en.variants.iter().any(|v| v == rhs) {
                    out.push(
                        Diagnostic::new(DiagnosticCode::E0100, span_of(node))
                            .with_message(format!("unknown enum variant '{lhs}.{rhs}'")),
                    );
                }
            }
            // Otherwise: unknown qualifier — not a primary check target.
        }
    }
    // Function calls — verify extern exists and is `pure` if used in a guard.
    if node.kind() == SyntaxKind::EXPR_CALL {
        if let Some(callee) = call_name(node) {
            match st.resolve_extern(&callee, scope) {
                None => {
                    out.push(
                        Diagnostic::new(DiagnosticCode::E0102, span_of(node))
                            .with_message(format!("unknown extern '{callee}'")),
                    );
                }
                Some((_, is_pure)) if in_guard && !is_pure => {
                    out.push(
                        Diagnostic::new(DiagnosticCode::E0106, span_of(node))
                            .with_message(format!("non-pure extern '{callee}' used in guard")),
                    );
                }
                _ => {}
            }
        }
    }
}

/// First identifier child of an `EXPR_CALL` node — the callee name.
fn call_name(node: &SyntaxNode) -> Option<String> {
    // The callee is the first child node, which is an EXPR_NAME_REF holding
    // a single Ident token.
    for child in node.children() {
        if child.kind() == SyntaxKind::EXPR_NAME_REF {
            return child
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .find(|t| t.kind() == SyntaxKind::Ident)
                .map(|t| t.text().to_string());
        }
        if child.kind() == SyntaxKind::EXPR_FIELD_REF {
            // Submachine-style call `m.f(args)` — return the second ident as
            // the callee name. Rare; we keep it permissive.
            let idents: Vec<_> = child
                .children_with_tokens()
                .filter_map(|el| el.into_token())
                .filter(|t| t.kind() == SyntaxKind::Ident)
                .map(|t| t.text().to_string())
                .collect();
            return idents.last().cloned();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Statement-level checks
// ---------------------------------------------------------------------------

fn check_stmt_raise(node: &SyntaxNode, scope: &Scope, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    if let Some(ev) = first_ident(node) {
        if st.resolve_event(&ev, scope).is_none() {
            out.push(
                Diagnostic::new(DiagnosticCode::E0101, span_of(node))
                    .with_message(format!("unknown event '{ev}' in raise")),
            );
        }
    }
}

fn check_stmt_send(node: &SyntaxNode, scope: &Scope, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    let idents: Vec<String> = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .collect();
    // Grammar: `send EVENT [(args)] to MACHINE`
    if let Some(ev) = idents.first() {
        if st.resolve_event(ev, scope).is_none() {
            out.push(
                Diagnostic::new(DiagnosticCode::E0101, span_of(node))
                    .with_message(format!("unknown event '{ev}' in send")),
            );
        }
    }
    if let Some(target) = idents.last() {
        if idents.len() >= 2 && st.resolve_machine(target).is_none() {
            out.push(
                Diagnostic::new(DiagnosticCode::E0103, span_of(node))
                    .with_message(format!("unknown machine '{target}' in send target")),
            );
        }
    }
}

fn check_stmt_defer(node: &SyntaxNode, scope: &Scope, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    if let Some(ev) = first_ident(node) {
        if st.resolve_event(&ev, scope).is_none() {
            out.push(
                Diagnostic::new(DiagnosticCode::E0101, span_of(node))
                    .with_message(format!("unknown event '{ev}' in defer")),
            );
        }
    }
}

fn check_stmt_call(node: &SyntaxNode, scope: &Scope, st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    if let Some(callee) = first_ident(node) {
        if st.resolve_extern(&callee, scope).is_none() {
            out.push(
                Diagnostic::new(DiagnosticCode::E0102, span_of(node))
                    .with_message(format!("unknown extern '{callee}'")),
            );
        }
    }
}

fn first_ident(node: &SyntaxNode) -> Option<String> {
    node.children_with_tokens()
        .filter_map(|el| el.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
}

fn check_join_target(
    node: &SyntaxNode,
    scope: &Scope,
    st: &SymbolTable,
    out: &mut Vec<Diagnostic>,
) {
    // The join target is the last ident token at the JOIN_DECL level.
    if let Some(t) = node
        .children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .last()
        .map(|t| t.text().to_string())
    {
        if st.resolve_state(&t, scope).is_none() {
            out.push(
                Diagnostic::new(DiagnosticCode::E0100, span_of(node))
                    .with_message(format!("unknown state '{t}' in join target")),
            );
        }
    }
}
