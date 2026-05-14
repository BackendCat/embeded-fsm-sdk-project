//! Machine declaration + machine-body formatter.
//!
//! `machine_decl = [ "export" ] , "machine" , identifier , "{" , { machine_item } , "}"`
//! per Doc 04 §4.
//!
//! Machine body items per Doc 04 §4:
//! - `CONTEXT_BLOCK`
//! - `EVENTS_BLOCK`
//! - `QUEUE_BLOCK`
//! - `TARGET_BLOCK`
//! - `INITIAL_DECL`
//! - State / region / pseudo-states / extern decls
//!
//! Doc 19 §4 mandates a section order but also says "formatter does NOT
//! reorder states". We preserve source order globally and only insert
//! blank-line separators between groups of different *kinds*.

use fsm_parser::{SyntaxKind, SyntaxNode};
use rowan::NodeOrToken;

use super::expr::{emit_type_ref_public, iter_tokens};
use super::state::emit_state_item;
use super::writer::FormatWriter;
use crate::options::FormatOptions;

pub fn emit_machine_decl(w: &mut FormatWriter, node: &SyntaxNode, opts: &FormatOptions) {
    // Optional `export` keyword.
    let has_export = iter_tokens(node).any(|t| t.kind() == SyntaxKind::KwExport);
    if has_export {
        w.write("export ");
    }
    w.write("machine ");
    let name = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    w.write(&name);
    w.write(" {");
    w.newline();
    w.indent();
    emit_machine_body(w, node, opts);
    w.dedent();
    w.write("}");
}

fn emit_machine_body(w: &mut FormatWriter, node: &SyntaxNode, opts: &FormatOptions) {
    let events: Vec<super::trivia::BodyEvent> = super::trivia::body_events_with_trailing(node);
    let mut last_kind: Option<SyntaxKind> = None;
    let mut pending_stable_id: Option<SyntaxNode> = None;

    let mut pos = 0;
    while pos < events.len() {
        match &events[pos] {
            super::trivia::BodyEvent::Trivia(_) => {
                let mut run_end = pos;
                while run_end < events.len()
                    && matches!(events[run_end], super::trivia::BodyEvent::Trivia(_))
                {
                    run_end += 1;
                }
                let run: Vec<fsm_parser::SyntaxToken> = (pos..run_end)
                    .filter_map(|i| match &events[i] {
                        super::trivia::BodyEvent::Trivia(t) => Some(t.clone()),
                        _ => None,
                    })
                    .collect();
                emit_trivia_run(w, &run);
                pos = run_end;
                continue;
            }
            super::trivia::BodyEvent::Node(child) => {
                let child = child.clone();
                if child.kind() == SyntaxKind::STABLE_ID_ANNOT {
                    pending_stable_id = Some(child);
                    pos += 1;
                    continue;
                }
                if child.kind() == SyntaxKind::ERROR_NODE {
                    pos += 1;
                    continue;
                }

                if let Some(prev) = last_kind {
                    if opts.blank_line_between_decls
                        && needs_blank_between_machine(prev, child.kind())
                        && !w.buf().ends_with("\n\n")
                    {
                        w.blank_line();
                    }
                }

                if let Some(sid) = pending_stable_id.take() {
                    super::state::emit_state_item(w, &sid, opts);
                    w.newline();
                }

                match child.kind() {
                    SyntaxKind::CONTEXT_BLOCK => emit_context_block(w, &child),
                    SyntaxKind::EVENTS_BLOCK => emit_events_block(w, &child, opts),
                    SyntaxKind::QUEUE_BLOCK => emit_config_block(w, &child, "queue", None),
                    SyntaxKind::TARGET_BLOCK => emit_target_block(w, &child),
                    SyntaxKind::EXTERN_DECL => emit_extern_decl(w, &child),
                    _ => emit_state_item(w, &child, opts),
                }
                w.newline();
                last_kind = Some(child.kind());
                pos += 1;
            }
        }
    }
}

fn emit_trivia_run(w: &mut FormatWriter, trivia: &[fsm_parser::SyntaxToken]) {
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
    if newlines_before >= 2 && !w.buf().is_empty() && !w.buf().ends_with("\n\n") {
        w.blank_line();
    }
}

fn needs_blank_between_machine(prev: SyntaxKind, next: SyntaxKind) -> bool {
    fn family(k: SyntaxKind) -> u8 {
        match k {
            SyntaxKind::CONTEXT_BLOCK => 1,
            SyntaxKind::EVENTS_BLOCK => 2,
            SyntaxKind::QUEUE_BLOCK | SyntaxKind::TARGET_BLOCK => 3,
            SyntaxKind::EXTERN_DECL => 4,
            SyntaxKind::INITIAL_DECL => 5,
            SyntaxKind::STATE_DECL
            | SyntaxKind::REGION_DECL
            | SyntaxKind::CHOICE_DECL
            | SyntaxKind::JUNCTION_DECL
            | SyntaxKind::FORK_DECL
            | SyntaxKind::JOIN_DECL
            | SyntaxKind::SHALLOW_HISTORY_DECL
            | SyntaxKind::DEEP_HISTORY_DECL
            | SyntaxKind::FINAL_DECL => 6,
            _ => 9,
        }
    }
    // Always blank between states at the machine level (Doc 19 §4).
    if matches!(
        prev,
        SyntaxKind::STATE_DECL | SyntaxKind::REGION_DECL | SyntaxKind::CHOICE_DECL
    ) || matches!(
        next,
        SyntaxKind::STATE_DECL | SyntaxKind::REGION_DECL | SyntaxKind::CHOICE_DECL
    ) {
        return true;
    }
    family(prev) != family(next)
}

// ─── Context ────────────────────────────────────────────────────────────

fn emit_context_block(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("context {");
    let fields: Vec<SyntaxNode> = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::FIELD_DECL)
        .collect();
    if fields.is_empty() {
        w.write(" }");
        return;
    }
    w.newline();
    w.indent();

    // Doc 19 §5: align colons + equals across the block.
    let parts: Vec<FieldRender> = fields.iter().map(render_field).collect();
    let max_name = parts.iter().map(|p| p.name.len()).max().unwrap_or(0);
    let max_type = parts.iter().map(|p| p.ty.len()).max().unwrap_or(0);

    for (i, p) in parts.iter().enumerate() {
        let _ = i;
        // `name: type = default` (no trailing `;` — Doc 04 §4.1 EBNF
        // never specifies one; Doc 19 §5 inserts one but VALIDATION_REPORT
        // flagged that as a Doc 19 vs grammar disagreement).
        w.write(&p.name);
        w.pad_spaces(max_name.saturating_sub(p.name.len()));
        w.write(": ");
        w.write(&p.ty);
        if let Some(default) = &p.default {
            w.pad_spaces(max_type.saturating_sub(p.ty.len()));
            w.write(" = ");
            w.write(default);
        }
        w.newline();
    }
    w.dedent();
    w.write("}");
}

struct FieldRender {
    name: String,
    ty: String,
    default: Option<String>,
}

fn render_field(field: &SyntaxNode) -> FieldRender {
    let name = iter_tokens(field)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();

    let opts = FormatOptions::default();
    let mut type_buf = FormatWriter::new(&opts);
    let mut default_buf = FormatWriter::new(&opts);

    let mut saw_eq = false;
    for child in field.children_with_tokens() {
        match child {
            NodeOrToken::Node(n)
                if n.kind() == SyntaxKind::TYPE_REF || n.kind() == SyntaxKind::OPAQUE_TYPE_REF =>
            {
                emit_type_ref_public(&mut type_buf, &n);
            }
            NodeOrToken::Node(n) if n.kind() == SyntaxKind::CONST_EXPR && saw_eq => {
                super::expr::emit_const_expr(&mut default_buf, &n);
            }
            NodeOrToken::Token(t) if t.kind() == SyntaxKind::Eq => saw_eq = true,
            _ => {}
        }
    }
    let default_s = default_buf.buf().to_string();
    FieldRender {
        name,
        ty: type_buf.buf().to_string(),
        default: if default_s.is_empty() {
            None
        } else {
            Some(default_s)
        },
    }
}

// ─── Events ─────────────────────────────────────────────────────────────

fn emit_events_block(w: &mut FormatWriter, node: &SyntaxNode, opts: &FormatOptions) {
    let _ = opts;
    w.write("events {");
    let events: Vec<SyntaxNode> = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::EVENT_DECL)
        .collect();
    if events.is_empty() {
        w.write(" }");
        return;
    }
    w.newline();
    w.indent();
    // Track pending stable-id annotations attached to the next event.
    let mut pending_sid: Option<SyntaxNode> = None;
    for child in node.children() {
        match child.kind() {
            SyntaxKind::STABLE_ID_ANNOT => pending_sid = Some(child.clone()),
            SyntaxKind::EVENT_DECL => {
                if let Some(sid) = pending_sid.take() {
                    super::state::emit_state_item(w, &sid, opts);
                    w.newline();
                }
                emit_event_decl(w, &child);
                w.newline();
            }
            _ => {}
        }
    }
    w.dedent();
    w.write("}");
}

fn emit_event_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    let name = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    w.write(&name);
    if let Some(payload) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::PAYLOAD_LIST)
    {
        emit_payload_list(w, &payload);
    }
}

fn emit_payload_list(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("(");
    let fields: Vec<SyntaxNode> = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::PAYLOAD_FIELD)
        .collect();
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        let name = iter_tokens(f)
            .find(|t| t.kind() == SyntaxKind::Ident)
            .map(|t| t.text().to_string())
            .unwrap_or_default();
        w.write(&name);
        w.write(": ");
        if let Some(ty) = f
            .children()
            .find(|n| n.kind() == SyntaxKind::TYPE_REF || n.kind() == SyntaxKind::OPAQUE_TYPE_REF)
        {
            emit_type_ref_public(w, &ty);
        }
    }
    w.write(")");
}

// ─── Queue / Target ──────────────────────────────────────────────────────

fn emit_config_block(w: &mut FormatWriter, node: &SyntaxNode, kw: &str, name: Option<&str>) {
    w.write(kw);
    if let Some(n) = name {
        w.space();
        w.write(n);
    }
    w.write(" {");
    let entries: Vec<SyntaxNode> = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::CONFIG_ENTRY)
        .collect();
    if entries.is_empty() {
        w.write(" }");
        return;
    }
    w.newline();
    w.indent();
    // Align `=` columns within the block.
    let max_key = entries
        .iter()
        .filter_map(config_key)
        .map(|s| s.len())
        .max()
        .unwrap_or(0);
    for e in &entries {
        let key = config_key(e).unwrap_or_default();
        w.write(&key);
        w.pad_spaces(max_key.saturating_sub(key.len()));
        w.write(" = ");
        let val = config_value(e).unwrap_or_default();
        w.write(&val);
        w.newline();
    }
    w.dedent();
    w.write("}");
}

fn config_key(node: &SyntaxNode) -> Option<String> {
    iter_tokens(node)
        .next()
        .filter(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
}

fn config_value(node: &SyntaxNode) -> Option<String> {
    // Two tokens emitted: key, then `=`, then value-token (int / true /
    // false / ident). Pick the first non-ident-and-non-eq token after the
    // `=` token.
    let mut tokens = iter_tokens(node);
    tokens.next(); // key
    for t in tokens {
        match t.kind() {
            SyntaxKind::Eq => continue,
            _ => return Some(t.text().to_string()),
        }
    }
    None
}

fn emit_target_block(w: &mut FormatWriter, node: &SyntaxNode) {
    let name = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    emit_config_block(w, node, "target", Some(&name));
}

// ─── Extern ──────────────────────────────────────────────────────────────

fn emit_extern_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    let is_pure = iter_tokens(node).any(|t| t.kind() == SyntaxKind::KwPure);
    if is_pure {
        w.write("pure ");
    }
    w.write("extern ");
    let name = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    w.write(&name);
    if let Some(params) = node.children().find(|n| n.kind() == SyntaxKind::PARAM_LIST) {
        w.write("(");
        emit_param_list(w, &params);
        w.write(")");
    } else {
        w.write("()");
    }
    if let Some(ret) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::TYPE_REF || n.kind() == SyntaxKind::OPAQUE_TYPE_REF)
    {
        w.write(" : ");
        emit_type_ref_public(w, &ret);
    }
}

fn emit_param_list(w: &mut FormatWriter, node: &SyntaxNode) {
    let params: Vec<SyntaxNode> = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::PARAM)
        .collect();
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        emit_param(w, p);
    }
}

fn emit_param(w: &mut FormatWriter, node: &SyntaxNode) {
    // Doc 04 §2.5 form: `type identifier`. The Doc 04 example in
    // §2.5 also shows the alternative `ctx` shorthand (one ident, no
    // type). Emit whatever the parser captured.
    let mut wrote = false;
    let ty = node
        .children()
        .find(|n| n.kind() == SyntaxKind::TYPE_REF || n.kind() == SyntaxKind::OPAQUE_TYPE_REF);
    if let Some(t) = ty {
        emit_type_ref_public(w, &t);
        wrote = true;
    }
    for tok in iter_tokens(node) {
        if tok.kind() == SyntaxKind::Ident {
            if wrote {
                w.space();
            }
            w.write(tok.text());
            return;
        }
    }
}
