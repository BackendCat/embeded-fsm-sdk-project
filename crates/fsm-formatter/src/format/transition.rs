//! Transition + timer formatter, plus arrow-column alignment across a
//! sibling group of transitions.
//!
//! Doc 04 §8 transition forms (with optional doc-comment and stable-id
//! prefixes that the parser stores as trivia / a sibling STABLE_ID_ANNOT
//! node):
//!
//! - **External**       `TRANSITION_DECL` — `on EVENT [g] -> T : action`
//! - **Internal**       `INTERNAL_DECL`   — `on EVENT [g] : action`   (no `->`)
//! - **Local**          `LOCAL_DECL`      — `on EVENT [g] ~> T : action`
//! - **Completion**     `COMPLETION_DECL` — `done [g] -> T : action`
//! - **One-shot timer** `AFTER_DECL`      — `after N ms -> T : action`
//! - **Periodic timer** `EVERY_DECL`      — `every N ms -> T : action`
//! - **Internal timer** `EVERY_INTERNAL_DECL` — `every N ms : action`
//!
//! ## Alignment
//!
//! Doc 19 §10 specifies that consecutive transitions in a sibling group
//! get vertically aligned `->` columns. We compute the alignment by:
//!
//! 1. For each sibling transition, compute the pre-arrow segment length
//!    (`on EVENT [guard]` or `after N ms`).
//! 2. Take `max(pre_arrow_len)` across the group.
//! 3. Pad each transition's pre-arrow segment up to that column with
//!    spaces before the `->`.
//!
//! Internal transitions (no `->`) are skipped for alignment because
//! mixing `:`-aligned and `->`-aligned columns degrades readability.

use fsm_parser::{SyntaxKind, SyntaxNode};
use rowan::NodeOrToken;

use super::expr::{emit_const_expr, emit_expr, iter_tokens};
use super::stmt::emit_inline_action_list;
use super::writer::FormatWriter;
use crate::options::FormatOptions;

/// Format one transition-like node, given an arrow target column for
/// alignment (or `None` to skip alignment). The opening writer position
/// is the start of the transition's line (caller emits indentation).
pub(crate) fn emit_transition(
    w: &mut FormatWriter,
    node: &SyntaxNode,
    align_arrow_col: Option<usize>,
    _opts: &FormatOptions,
) {
    match node.kind() {
        SyntaxKind::TRANSITION_DECL => emit_on_arrow(w, node, "->", align_arrow_col),
        SyntaxKind::LOCAL_DECL => emit_on_arrow(w, node, "~>", align_arrow_col),
        SyntaxKind::INTERNAL_DECL => emit_internal(w, node),
        SyntaxKind::COMPLETION_DECL => emit_completion(w, node, align_arrow_col),
        SyntaxKind::AFTER_DECL => emit_after(w, node, align_arrow_col),
        SyntaxKind::EVERY_DECL => emit_every(w, node, align_arrow_col),
        SyntaxKind::EVERY_INTERNAL_DECL => emit_every_internal(w, node),
        _ => {
            // Defensive — should not be reached.
            w.write(node.text().to_string().trim());
        }
    }
}

/// Compute the *pre-arrow* width (chars from start-of-line to where the
/// `->` should go) for one transition. Used by the group-alignment pass.
pub(crate) fn pre_arrow_width(node: &SyntaxNode) -> Option<usize> {
    let s = render_pre_arrow_into_string(node)?;
    Some(s.len())
}

fn render_pre_arrow_into_string(node: &SyntaxNode) -> Option<String> {
    // ignore self when measuring
    let tmp_opts = FormatOptions {
        align_arrows: false,
        ..Default::default()
    };
    let mut buf = FormatWriter::new(&tmp_opts);
    match node.kind() {
        SyntaxKind::TRANSITION_DECL | SyntaxKind::LOCAL_DECL => {
            emit_on_prefix(&mut buf, node);
        }
        SyntaxKind::COMPLETION_DECL => {
            emit_done_prefix(&mut buf, node);
        }
        SyntaxKind::AFTER_DECL => {
            emit_after_prefix(&mut buf, node);
        }
        SyntaxKind::EVERY_DECL => {
            emit_every_prefix(&mut buf, node);
        }
        _ => return None,
    }
    // The buffer doesn't end with a newline — read it as-is.
    Some(buf.buf().to_string())
}

// ─── Pre-arrow renderers ─────────────────────────────────────────────────

fn emit_on_prefix(w: &mut FormatWriter, node: &SyntaxNode) {
    // `on EVENT [guard] [priority N]`
    w.write("on ");
    let mut wrote_event = false;
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Token(t) if t.kind() == SyntaxKind::Ident && !wrote_event => {
                w.write(t.text());
                wrote_event = true;
            }
            NodeOrToken::Node(n) if n.kind() == SyntaxKind::GUARD_CLAUSE => {
                w.space();
                emit_guard_clause(w, &n);
            }
            NodeOrToken::Node(n) if n.kind() == SyntaxKind::PRIORITY_CLAUSE => {
                w.space();
                emit_priority_clause(w, &n);
            }
            _ => {}
        }
    }
}

fn emit_done_prefix(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("done");
    for child in node.children() {
        match child.kind() {
            SyntaxKind::GUARD_CLAUSE => {
                w.space();
                emit_guard_clause(w, &child);
            }
            SyntaxKind::PRIORITY_CLAUSE => {
                w.space();
                emit_priority_clause(w, &child);
            }
            _ => {}
        }
    }
}

fn emit_after_prefix(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("after ");
    if let Some(c) = node.children().find(|n| n.kind() == SyntaxKind::CONST_EXPR) {
        emit_const_expr(w, &c);
    }
    w.write(" ms");
}

fn emit_every_prefix(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("every ");
    if let Some(c) = node.children().find(|n| n.kind() == SyntaxKind::CONST_EXPR) {
        emit_const_expr(w, &c);
    }
    w.write(" ms");
}

// ─── Full-form renderers ────────────────────────────────────────────────

fn emit_on_arrow(
    w: &mut FormatWriter,
    node: &SyntaxNode,
    arrow: &str,
    align_arrow_col: Option<usize>,
) {
    let pre_start_col = w.column();
    emit_on_prefix(w, node);
    pad_then_arrow(w, pre_start_col, align_arrow_col, arrow);
    emit_target_and_action(w, node);
}

fn emit_completion(w: &mut FormatWriter, node: &SyntaxNode, align_arrow_col: Option<usize>) {
    let pre_start_col = w.column();
    emit_done_prefix(w, node);
    pad_then_arrow(w, pre_start_col, align_arrow_col, "->");
    emit_target_and_action(w, node);
}

fn emit_after(w: &mut FormatWriter, node: &SyntaxNode, align_arrow_col: Option<usize>) {
    let pre_start_col = w.column();
    emit_after_prefix(w, node);
    pad_then_arrow(w, pre_start_col, align_arrow_col, "->");
    emit_target_and_action(w, node);
}

fn emit_every(w: &mut FormatWriter, node: &SyntaxNode, align_arrow_col: Option<usize>) {
    let pre_start_col = w.column();
    emit_every_prefix(w, node);
    pad_then_arrow(w, pre_start_col, align_arrow_col, "->");
    emit_target_and_action(w, node);
}

fn emit_every_internal(w: &mut FormatWriter, node: &SyntaxNode) {
    emit_every_prefix(w, node);
    if let Some(action) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
    {
        w.write(" : ");
        emit_inline_action_list(w, &action);
    }
}

fn emit_internal(w: &mut FormatWriter, node: &SyntaxNode) {
    emit_on_prefix(w, node);
    if let Some(action) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
    {
        w.write(" : ");
        emit_inline_action_list(w, &action);
    }
}

/// Emit the target identifier (if any) and optional `: action`.
fn emit_target_and_action(w: &mut FormatWriter, node: &SyntaxNode) {
    // The target is the *last* Ident token that is a direct child token of
    // `node` — not inside guard or priority subnodes.
    let target = iter_tokens(node)
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .last();
    if let Some(t) = target {
        // Exactly one space — bypass `space()` so a preceding pad_spaces
        // run doesn't suppress this separator.
        w.write(" ");
        w.write(t.text());
    }
    if let Some(action) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
    {
        // Doc 04 §8 transition forms spell the action as `: action_list`
        // — a flat, semicolon-separated sequence with **no** surrounding
        // braces (Doc 19 §10's `{ … }` example is one of the formatter-
        // vs-grammar disagreements catalogued in lib.rs). Emit inline.
        w.write(" : ");
        emit_inline_action_list(w, &action);
    }
}

/// Pad to alignment + emit ` arrow `. Always inserts exactly one space
/// after the (possibly padded) pre-arrow text and exactly one space after
/// the arrow. Without alignment, the pre-arrow padding is zero.
fn pad_then_arrow(
    w: &mut FormatWriter,
    line_start_col: usize,
    align_arrow_col: Option<usize>,
    arrow: &str,
) {
    if let Some(target_pre_len) = align_arrow_col {
        let current = w.column();
        let pre_len = current.saturating_sub(line_start_col);
        if target_pre_len > pre_len {
            w.pad_spaces(target_pre_len - pre_len);
        }
    }
    // Exactly one space, then arrow. We emit via `write` to bypass
    // `space()`'s "merge adjacent spaces" behaviour.
    w.write(" ");
    w.write(arrow);
}

// ─── Guard / priority ───────────────────────────────────────────────────

pub(crate) fn emit_guard_clause(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("[");
    // The guard body is a single expression (possibly `else`).
    if let Some(inner) = node.children().next() {
        emit_expr(w, &inner);
    }
    w.write("]");
}

fn emit_priority_clause(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("priority ");
    for child in node.children_with_tokens() {
        if let NodeOrToken::Token(t) = child {
            if t.kind() == SyntaxKind::IntLiteral {
                w.write(t.text());
                break;
            }
        }
    }
}

// ─── Group-alignment helper ─────────────────────────────────────────────

/// Decide whether `kind` belongs to the alignment family (has an arrow).
pub(crate) fn is_arrow_transition(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::TRANSITION_DECL
            | SyntaxKind::LOCAL_DECL
            | SyntaxKind::COMPLETION_DECL
            | SyntaxKind::AFTER_DECL
            | SyntaxKind::EVERY_DECL
    )
}

/// Decide whether `kind` is a transition family member at all (with or
/// without arrow).
pub(crate) fn is_transition_kind(kind: SyntaxKind) -> bool {
    is_arrow_transition(kind)
        || matches!(
            kind,
            SyntaxKind::INTERNAL_DECL | SyntaxKind::EVERY_INTERNAL_DECL
        )
}
