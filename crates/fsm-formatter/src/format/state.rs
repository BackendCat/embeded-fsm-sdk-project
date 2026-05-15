//! State + pseudo-state formatter.
//!
//! `STATE_DECL` is the catch-all node for both *simple* and *composite*
//! states (Doc 04 §5; per Doc 00 §B-13 there is no `composite` keyword in
//! the grammar — a state is composite iff its body contains nested state
//! decls or regions). The formatter doesn't classify; it walks the body
//! emitting each child verbatim.
//!
//! Pseudo-state nodes covered here:
//! - `REGION_DECL` (Doc 04 §6)
//! - `SHALLOW_HISTORY_DECL` / `DEEP_HISTORY_DECL` (§7.1 / §7.2)
//! - `CHOICE_DECL` / `JUNCTION_DECL` (§7.3 / §7.4)
//! - `FORK_DECL` / `JOIN_DECL` (§7.5 / §7.6)
//! - `INITIAL_DECL` / `FINAL_DECL` / `ENTRY_POINT_DECL` /
//!   `EXIT_POINT_DECL` (§4.5 / §5.2 / §5.3)
//! - `ENTRY_DECL` / `EXIT_DECL` (§5.1)
//! - All transition kinds — delegated to `transition.rs`.
//! - `DEFER_DECL` (§9.4)
//! - Nested `STATE_DECL` (recursion).

use fsm_parser::{SyntaxKind, SyntaxNode};
use rowan::NodeOrToken;

use super::expr::{first_ident, iter_tokens};
use super::transition::{
    emit_transition, is_arrow_transition, is_transition_kind, pre_arrow_width,
};
use super::trivia;
use super::writer::FormatWriter;
use crate::options::FormatOptions;

/// Format a `STATE_DECL` (top-level call site: at the writer's current
/// indent, no leading newline).
pub fn emit_state_decl(w: &mut FormatWriter, node: &SyntaxNode, opts: &FormatOptions) {
    // Optional [export] prefix.
    let has_export = iter_tokens(node).any(|t| t.kind() == SyntaxKind::KwExport);
    if has_export {
        w.write("export ");
    }
    w.write("state ");
    let name = state_name(node).unwrap_or_default();
    w.write(&name);

    if has_children_body(node) {
        w.write(" {");
        w.newline();
        w.indent();
        emit_state_body(w, node, opts);
        w.dedent();
        w.write("}");
    } else {
        w.write(" { }");
    }
}

/// Pseudo-state-ish nodes that the machine body also accepts as items.
pub fn emit_state_item(w: &mut FormatWriter, node: &SyntaxNode, opts: &FormatOptions) {
    match node.kind() {
        SyntaxKind::STATE_DECL => emit_state_decl(w, node, opts),
        SyntaxKind::REGION_DECL => emit_region_decl(w, node, opts),
        SyntaxKind::SHALLOW_HISTORY_DECL => emit_history_decl(w, node, "shallow_history"),
        SyntaxKind::DEEP_HISTORY_DECL => emit_history_decl(w, node, "deep_history"),
        SyntaxKind::CHOICE_DECL => emit_choice_decl(w, node, "choice", opts),
        SyntaxKind::JUNCTION_DECL => emit_choice_decl(w, node, "junction", opts),
        SyntaxKind::FORK_DECL => emit_fork_decl(w, node, opts),
        SyntaxKind::JOIN_DECL => emit_join_decl(w, node, opts),
        SyntaxKind::INITIAL_DECL => emit_initial_decl(w, node),
        SyntaxKind::FINAL_DECL => emit_final_decl(w, node),
        SyntaxKind::ENTRY_POINT_DECL => emit_entry_point_decl(w, node),
        SyntaxKind::EXIT_POINT_DECL => emit_exit_point_decl(w, node),
        SyntaxKind::ENTRY_DECL => emit_entry_or_exit(w, node, "entry"),
        SyntaxKind::EXIT_DECL => emit_entry_or_exit(w, node, "exit"),
        SyntaxKind::DEFER_DECL => emit_defer_decl(w, node),
        SyntaxKind::STABLE_ID_ANNOT => emit_stable_id_annot(w, node),
        k if is_transition_kind(k) => emit_transition(w, node, /* align = */ None, opts),
        _ => {
            // ERROR_NODE or unknown — raw re-emit.
            w.write(node.text().to_string().trim());
        }
    }
}

fn state_name(node: &SyntaxNode) -> Option<String> {
    iter_tokens(node)
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .next()
}

fn has_children_body(node: &SyntaxNode) -> bool {
    // True iff the body contains at least one non-trivia child node
    // (entry/exit/transition/nested state/etc.).
    node.children().any(|c| {
        !matches!(
            c.kind(),
            SyntaxKind::STABLE_ID_ANNOT | SyntaxKind::ERROR_NODE
        )
    })
}

/// Emit children of a state-like body. Trivia is preserved (Doc 19 §14):
/// comments survive in their relative position; user-authored blank
/// lines between groups are kept (collapsed to one); blank lines between
/// declarations of *different* kinds are inserted.
///
/// Source order is preserved exactly — Doc 19 §8 "section order" is
/// described prescriptively but Doc 19 §1.5 ("minimal diff") wins in
/// conflict: we never reorder declarations.
pub fn emit_state_body(w: &mut FormatWriter, parent: &SyntaxNode, opts: &FormatOptions) {
    let events: Vec<trivia::BodyEvent> = trivia::body_events_with_trailing(parent);

    // Two-pass: collect indices of node events alongside their kinds so we
    // can detect arrow-transition runs and decide blank-line breaks.
    let node_positions: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            trivia::BodyEvent::Node(_) => Some(i),
            _ => None,
        })
        .collect();

    let mut last_emitted_kind: Option<SyntaxKind> = None;
    let mut pending_stable_id: Option<SyntaxNode> = None;

    // Single-pass walk over the body. `pos` indexes into `events`.
    let mut pos = 0;
    while pos < events.len() {
        match &events[pos] {
            trivia::BodyEvent::Trivia(_) => {
                // Find this trivia's neighbours: it sits between
                // previous-emitted-node (if any) and the next node.
                let next_node_idx = next_node_position(&node_positions, pos);
                // Collect the contiguous trivia run before next node.
                let mut run_end = pos;
                while run_end < events.len()
                    && matches!(events[run_end], trivia::BodyEvent::Trivia(_))
                {
                    run_end += 1;
                }
                // Render the trivia run (counts blank-line newlines and
                // emits any comments).
                let trivia_run: Vec<fsm_parser::SyntaxToken> = (pos..run_end)
                    .filter_map(|i| match &events[i] {
                        trivia::BodyEvent::Trivia(t) => Some(t.clone()),
                        _ => None,
                    })
                    .collect();
                emit_body_trivia(
                    w,
                    &trivia_run,
                    last_emitted_kind.is_some(),
                    next_node_idx.is_some(),
                );
                pos = run_end;
                continue;
            }
            trivia::BodyEvent::Node(node) => {
                let node = node.clone();
                match node.kind() {
                    SyntaxKind::STABLE_ID_ANNOT => {
                        pending_stable_id = Some(node);
                        pos += 1;
                        continue;
                    }
                    SyntaxKind::ERROR_NODE => {
                        pos += 1;
                        continue;
                    }
                    _ => {}
                }

                // Decide blank-line break between kinds.
                if let Some(prev) = last_emitted_kind {
                    if opts.blank_line_between_decls && needs_blank_between(prev, node.kind()) {
                        // Only inject if we haven't already (the trivia run
                        // before may have already emitted blank_line()).
                        if !w.buf().ends_with("\n\n") {
                            w.blank_line();
                        }
                    }
                }

                if let Some(sid) = pending_stable_id.take() {
                    emit_stable_id_annot(w, &sid);
                    w.newline();
                }

                // Detect a run of arrow-aligned transitions starting at
                // this position.
                if is_arrow_transition(node.kind()) && opts.align_arrows {
                    let (run_end_pos, run_nodes) =
                        arrow_run(&events, pos, /* same_group_only = */ true);
                    let widths: Vec<usize> = run_nodes.iter().filter_map(pre_arrow_width).collect();
                    let max_w = widths.iter().copied().max().unwrap_or(0);
                    for n in &run_nodes {
                        emit_transition(w, n, Some(max_w), opts);
                        w.newline();
                        last_emitted_kind = Some(n.kind());
                    }
                    pos = run_end_pos;
                    continue;
                }

                emit_state_item(w, &node, opts);
                w.newline();
                last_emitted_kind = Some(node.kind());
                pos += 1;
            }
        }
    }
}

fn next_node_position(node_positions: &[usize], from: usize) -> Option<usize> {
    node_positions.iter().copied().find(|&i| i >= from)
}

/// Emit a trivia run (whitespace/newlines/comments) that sits between two
/// declarations in a body. `has_previous_decl` tells us whether there's
/// already something emitted (so we can decide to start with a blank
/// line). `has_next_decl` is currently informational.
fn emit_body_trivia(
    w: &mut FormatWriter,
    trivia: &[fsm_parser::SyntaxToken],
    _has_previous_decl: bool,
    _has_next_decl: bool,
) {
    // We separate the trivia run into newlines + comments. We emit each
    // comment at the current indent, and insert at most a single blank
    // line before / between comments if the user had >=2 newlines.
    let mut newlines_before: usize = 0;
    for tok in trivia {
        match tok.kind() {
            SyntaxKind::Newline => newlines_before += 1,
            SyntaxKind::Whitespace => {
                newlines_before += tok.text().bytes().filter(|b| *b == b'\n').count();
            }
            SyntaxKind::LineComment | SyntaxKind::DocComment | SyntaxKind::BlockComment => {
                if newlines_before >= 2 && !w.buf().ends_with("\n\n") {
                    w.blank_line();
                }
                super::trivia::emit_comment(w, tok);
                newlines_before = 0;
            }
            _ => {}
        }
    }
    // If the trivia run ends with 2+ newlines (a user-authored blank
    // line) and there is a next declaration, mark a pending blank line.
    if newlines_before >= 2 && !w.buf().ends_with("\n\n") && !w.buf().is_empty() {
        w.blank_line();
    }
}

/// Find a run of arrow-aligned transitions starting at `start_pos` in
/// `events`. Returns the index *after* the last consumed event, plus the
/// list of nodes in the run.
///
/// Boundaries:
/// - Non-trivia tokens (shouldn't happen at this level — already filtered).
/// - Non-arrow-transition nodes.
/// - A blank line (>=2 newlines) in the trivia run between two transitions.
/// - A comment in the trivia run.
fn arrow_run(
    events: &[trivia::BodyEvent],
    start_pos: usize,
    _same_group_only: bool,
) -> (usize, Vec<SyntaxNode>) {
    let mut nodes = Vec::new();
    let mut pos = start_pos;
    let mut prev_was_transition = false;

    while pos < events.len() {
        match &events[pos] {
            trivia::BodyEvent::Trivia(_t) => {
                if prev_was_transition {
                    // Look ahead — if blank line or comment, end the run.
                    let mut nl = 0usize;
                    let mut saw_comment = false;
                    let mut probe = pos;
                    while probe < events.len() {
                        match &events[probe] {
                            trivia::BodyEvent::Trivia(t2) => match t2.kind() {
                                SyntaxKind::Newline => nl += 1,
                                SyntaxKind::Whitespace => {
                                    nl += t2.text().bytes().filter(|b| *b == b'\n').count();
                                }
                                SyntaxKind::LineComment
                                | SyntaxKind::BlockComment
                                | SyntaxKind::DocComment => {
                                    saw_comment = true;
                                    break;
                                }
                                _ => {}
                            },
                            trivia::BodyEvent::Node(_) => break,
                        }
                        probe += 1;
                    }
                    if nl >= 2 || saw_comment {
                        return (pos, nodes);
                    }
                }
                pos += 1;
                continue;
            }
            trivia::BodyEvent::Node(n) => {
                if is_arrow_transition(n.kind()) {
                    nodes.push(n.clone());
                    prev_was_transition = true;
                    pos += 1;
                    continue;
                }
                if n.kind() == SyntaxKind::STABLE_ID_ANNOT {
                    // Stable-id breaks the run (attaches to next decl,
                    // not part of the current transition).
                    return (pos, nodes);
                }
                return (pos, nodes);
            }
        }
    }
    (pos, nodes)
}

/// Does the change of kind from `prev` to `next` deserve a blank line?
fn needs_blank_between(prev: SyntaxKind, next: SyntaxKind) -> bool {
    fn family(k: SyntaxKind) -> u8 {
        match k {
            SyntaxKind::ENTRY_DECL | SyntaxKind::EXIT_DECL => 1,
            k if is_transition_kind(k) => 2,
            SyntaxKind::DEFER_DECL => 3,
            SyntaxKind::INITIAL_DECL
            | SyntaxKind::FINAL_DECL
            | SyntaxKind::ENTRY_POINT_DECL
            | SyntaxKind::EXIT_POINT_DECL => 4,
            SyntaxKind::SHALLOW_HISTORY_DECL | SyntaxKind::DEEP_HISTORY_DECL => 5,
            SyntaxKind::CHOICE_DECL | SyntaxKind::JUNCTION_DECL => 6,
            SyntaxKind::FORK_DECL | SyntaxKind::JOIN_DECL => 7,
            SyntaxKind::STATE_DECL | SyntaxKind::REGION_DECL => 8,
            _ => 9,
        }
    }
    family(prev) != family(next)
}

// ─── Concrete emitters ──────────────────────────────────────────────────

fn emit_entry_or_exit(w: &mut FormatWriter, node: &SyntaxNode, kw: &str) {
    w.write(kw);
    w.write(" : ");
    if let Some(action) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
    {
        // Doc 04 §5.1 declares `entry : action_list` — there is NO
        // surrounding `{ }` in the grammar. The CST nonetheless wraps
        // the statement(s) in an ACTION_BLOCK node. We emit the body
        // inline (no braces) for single-statement action lists per Doc
        // 04 EBNF; multi-statement bodies wrap one per line with a
        // separator `;`.
        //
        // The Doc 19 §9 example `entry: { startTimer(); }` uses braces,
        // but per VALIDATION_REPORT 2.21 those braces aren't part of
        // Doc 04's grammar. We follow the grammar.
        super::stmt::emit_inline_action_list(w, &action);
    }
}

fn emit_initial_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("initial ");
    if let Some(id) = first_ident(node) {
        w.write(&id);
    }
}

fn emit_final_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("final ");
    if let Some(id) = first_ident(node) {
        w.write(&id);
    }
}

fn emit_entry_point_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    let ids: Vec<String> = iter_tokens(node)
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .collect();
    w.write("entry_point ");
    if let Some(name) = ids.first() {
        w.write(name);
    }
    w.write(" -> ");
    if let Some(t) = ids.get(1) {
        w.write(t);
    }
}

fn emit_exit_point_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("exit_point ");
    if let Some(id) = first_ident(node) {
        w.write(&id);
    }
}

fn emit_defer_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("defer ");
    if let Some(id) = first_ident(node) {
        w.write(&id);
    }
}

fn emit_region_decl(w: &mut FormatWriter, node: &SyntaxNode, opts: &FormatOptions) {
    w.write("region ");
    let name = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    w.write(&name);
    if has_children_body(node) {
        w.write(" {");
        w.newline();
        w.indent();
        emit_state_body(w, node, opts);
        w.dedent();
        w.write("}");
    } else {
        w.write(" { }");
    }
}

fn emit_history_decl(w: &mut FormatWriter, node: &SyntaxNode, kw: &str) {
    // Per Doc 04 §1.5 the single-token form. Doc 19 §12 wrote
    // `history shallow` (two words); not in grammar.
    w.write(kw);
    w.space();
    let name = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    w.write(&name);
    w.write(" {");
    if let Some(init) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::INITIAL_DECL)
    {
        w.space();
        emit_initial_decl(w, &init);
        w.space();
    } else {
        // No default-target initial — Doc 00 §B-14 mandates it; emit
        // empty braces (the analyzer raises E0111 separately).
        w.space();
    }
    w.write("}");
}

fn emit_choice_decl(w: &mut FormatWriter, node: &SyntaxNode, kw: &str, _opts: &FormatOptions) {
    w.write(kw);
    w.space();
    let name = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    w.write(&name);
    w.write(" {");
    w.newline();
    w.indent();

    // Branches: `[guard] -> Target [: action]`. Pad guards to align
    // arrows.
    let branches: Vec<SyntaxNode> = node
        .children()
        .filter(|n| {
            matches!(
                n.kind(),
                SyntaxKind::CHOICE_BRANCH | SyntaxKind::JUNCTION_BRANCH
            )
        })
        .collect();

    let guard_widths: Vec<usize> = branches.iter().map(guard_render_width).collect();
    let max_g = guard_widths.iter().copied().max().unwrap_or(0);

    for b in &branches {
        emit_choice_branch(w, b, max_g);
        w.newline();
    }
    w.dedent();
    w.write("}");
}

fn guard_render_width(branch: &SyntaxNode) -> usize {
    let guard = branch
        .children()
        .find(|n| n.kind() == SyntaxKind::GUARD_CLAUSE);
    match guard {
        None => 0,
        Some(g) => {
            // Render to a throwaway writer and count chars.
            let opts = FormatOptions::default();
            let mut buf = FormatWriter::new(&opts);
            super::transition::emit_guard_clause(&mut buf, &g);
            buf.buf().len()
        }
    }
}

fn emit_choice_branch(w: &mut FormatWriter, branch: &SyntaxNode, max_guard_w: usize) {
    if let Some(g) = branch
        .children()
        .find(|n| n.kind() == SyntaxKind::GUARD_CLAUSE)
    {
        let start = w.column();
        super::transition::emit_guard_clause(w, &g);
        let used = w.column() - start;
        if max_guard_w > used {
            w.pad_spaces(max_guard_w - used);
        }
    } else {
        // Guard missing — defensive; emit empty `[]`.
        w.write("[]");
        if max_guard_w > 2 {
            w.pad_spaces(max_guard_w - 2);
        }
    }
    w.write(" -> ");
    let target = iter_tokens(branch)
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .last();
    if let Some(t) = target {
        w.write(t.text());
    }
    if let Some(action) = branch
        .children()
        .find(|n| n.kind() == SyntaxKind::ACTION_BLOCK)
    {
        // Choice/junction branches share the transition action-list
        // shape (Doc 04 §7.3 → §8 action_list, no braces).
        w.write(" : ");
        super::stmt::emit_inline_action_list(w, &action);
    }
}

fn emit_fork_decl(w: &mut FormatWriter, node: &SyntaxNode, _opts: &FormatOptions) {
    w.write("fork ");
    let name = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    w.write(&name);
    w.write(" -> { ");
    if let Some(targets) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::FORK_TARGETS)
    {
        let ids: Vec<String> = iter_tokens(&targets)
            .filter(|t| t.kind() == SyntaxKind::Ident)
            .map(|t| t.text().to_string())
            .collect();
        w.write(&ids.join(", "));
    }
    w.write(" }");
}

fn emit_join_decl(w: &mut FormatWriter, node: &SyntaxNode, _opts: &FormatOptions) {
    w.write("join ");
    // First Ident before JOIN_SOURCES is the name.
    let name = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    w.write(&name);
    w.write(" { ");
    if let Some(sources) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::JOIN_SOURCES)
    {
        let ids: Vec<String> = iter_tokens(&sources)
            .filter(|t| t.kind() == SyntaxKind::Ident)
            .map(|t| t.text().to_string())
            .collect();
        w.write(&ids.join(", "));
    }
    w.write(" } -> ");
    // The target is the last top-level Ident token (after JOIN_SOURCES).
    // We walk top-level tokens explicitly because `iter_tokens` already
    // returns the name first.
    let target = node
        .children_with_tokens()
        .filter_map(|c| match c {
            NodeOrToken::Token(t) if t.kind() == SyntaxKind::Ident => Some(t.text().to_string()),
            _ => None,
        })
        .last();
    if let Some(t) = target {
        w.write(&t);
    }
}

fn emit_stable_id_annot(w: &mut FormatWriter, node: &SyntaxNode) {
    // Re-emit verbatim, normalised to `@id("…")` shape if possible. The
    // lexer can emit the whole annotation as a single `StableId` token
    // (`@ident`) or as a longer `@ Ident ( "…" )` form. We trust the
    // parser's tokens.
    let raw: String = iter_tokens(node).map(|t| t.text().to_string()).collect();
    w.write(&raw);
}
