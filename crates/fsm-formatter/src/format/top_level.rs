//! File-level emitter — language header, imports, top-level decls.
//!
//! Doc 04 §2:
//! ```ebnf
//! file = language_decl , { import_decl } , { top_level_decl } ;
//! top_level_decl = feature_decl | const_decl | enum_decl
//!                | extern_decl | machine_decl ;
//! ```
//!
//! Per Doc 19 §3 / §4 we emit:
//! - language header
//! - all imports (no blank line between consecutive imports — they form a group)
//! - blank line
//! - features (group)
//! - blank line
//! - other top-level decls, with a blank line between *kinds* and between
//!   adjacent machines.

use fsm_parser::{SyntaxKind, SyntaxNode};
use rowan::NodeOrToken;

use super::expr::{emit_const_expr, emit_type_ref_public, iter_tokens};
use super::machine::emit_machine_decl;
use super::writer::FormatWriter;
use crate::options::FormatOptions;

pub fn emit_file(w: &mut FormatWriter, file: &SyntaxNode, opts: &FormatOptions) {
    debug_assert_eq!(file.kind(), SyntaxKind::FILE);
    let events: Vec<super::trivia::BodyEvent> = super::trivia::body_events_with_trailing(file);
    let mut last_kind: Option<SyntaxKind> = None;
    let mut pending_sid: Option<SyntaxNode> = None;

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
                    pending_sid = Some(child);
                    pos += 1;
                    continue;
                }
                if child.kind() == SyntaxKind::ERROR_NODE {
                    pos += 1;
                    continue;
                }

                if let Some(prev) = last_kind {
                    if opts.blank_line_between_decls
                        && needs_blank_top(prev, child.kind())
                        && !w.buf().ends_with("\n\n")
                    {
                        w.blank_line();
                    }
                }

                if let Some(sid) = pending_sid.take() {
                    super::state::emit_state_item(w, &sid, opts);
                    w.newline();
                }

                match child.kind() {
                    SyntaxKind::LANGUAGE_DECL => emit_language_decl(w, &child),
                    SyntaxKind::IMPORT_DECL => emit_import_decl(w, &child),
                    SyntaxKind::FEATURE_DECL => emit_feature_decl(w, &child),
                    SyntaxKind::CONST_DECL => emit_const_decl(w, &child),
                    SyntaxKind::ENUM_DECL => emit_enum_decl(w, &child, opts),
                    SyntaxKind::EXTERN_DECL => emit_extern_top(w, &child),
                    SyntaxKind::MACHINE_DECL => emit_machine_decl(w, &child, opts),
                    _ => {
                        w.write(child.text().to_string().trim());
                    }
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
                if newlines_before >= 2 && !w.buf().ends_with("\n\n") && !w.buf().is_empty() {
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

fn needs_blank_top(prev: SyntaxKind, next: SyntaxKind) -> bool {
    fn fam(k: SyntaxKind) -> u8 {
        match k {
            SyntaxKind::LANGUAGE_DECL => 0,
            SyntaxKind::IMPORT_DECL => 1,
            SyntaxKind::FEATURE_DECL => 2,
            SyntaxKind::CONST_DECL => 3,
            SyntaxKind::ENUM_DECL => 4,
            SyntaxKind::EXTERN_DECL => 5,
            SyntaxKind::MACHINE_DECL => 6,
            _ => 9,
        }
    }
    // Always blank around machines.
    if prev == SyntaxKind::MACHINE_DECL || next == SyntaxKind::MACHINE_DECL {
        return true;
    }
    // Blank around enum.
    if prev == SyntaxKind::ENUM_DECL || next == SyntaxKind::ENUM_DECL {
        return true;
    }
    fam(prev) != fam(next)
}

fn emit_language_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("language fsm ");
    // Either FloatLiteral (`2.0`) or `int '.' int`. Emit verbatim.
    if let Some(ver) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::LANGUAGE_VERSION)
    {
        let txt: String = iter_tokens(&ver).map(|t| t.text().to_string()).collect();
        w.write(&txt);
    }
}

fn emit_import_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("import ");
    // First StringLiteral is the path.
    for child in node.children_with_tokens() {
        if let NodeOrToken::Token(t) = child {
            if t.kind() == SyntaxKind::StringLiteral {
                w.write(t.text());
                break;
            }
        }
    }
    if let Some(alias) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::IMPORT_ALIAS)
    {
        w.write(" as ");
        if let Some(id) = iter_tokens(&alias).find(|t| t.kind() == SyntaxKind::Ident) {
            w.write(id.text());
        }
    }
    if let Some(list) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::IMPORT_LIST)
    {
        w.write(" {");
        let items: Vec<String> = list
            .children()
            .filter(|n| n.kind() == SyntaxKind::IMPORT_ITEM)
            .filter_map(|n| {
                iter_tokens(&n)
                    .find(|t| t.kind() == SyntaxKind::Ident)
                    .map(|t| t.text().to_string())
            })
            .collect();
        if !items.is_empty() {
            w.write(" ");
            w.write(&items.join(", "));
            w.write(" ");
        }
        w.write("}");
    }
}

fn emit_feature_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("feature ");
    // Per parser, the feature name may be a keyword (e.g. `parallel`)
    // or an identifier. Take the first non-`feature` token.
    let mut iter = iter_tokens(node);
    iter.next(); // KwFeature
    if let Some(t) = iter.next() {
        w.write(t.text());
    }
}

fn emit_const_decl(w: &mut FormatWriter, node: &SyntaxNode) {
    w.write("const ");
    if let Some(name) = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
    {
        w.write(&name);
    }
    w.write(" = ");
    if let Some(expr) = node.children().find(|n| n.kind() == SyntaxKind::CONST_EXPR) {
        emit_const_expr(w, &expr);
    }
}

fn emit_enum_decl(w: &mut FormatWriter, node: &SyntaxNode, opts: &FormatOptions) {
    w.write("enum ");
    if let Some(name) = iter_tokens(node)
        .find(|t| t.kind() == SyntaxKind::Ident)
        .map(|t| t.text().to_string())
    {
        w.write(&name);
    }
    w.write(" {");
    let variants: Vec<SyntaxNode> = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::ENUM_VARIANT)
        .collect();
    if variants.is_empty() {
        w.write(" }");
        return;
    }
    w.newline();
    w.indent();

    // Align `=` columns across explicit-value variants.
    let parts: Vec<(String, Option<String>)> = variants
        .iter()
        .map(|v| {
            let mut toks = iter_tokens(v).map(|t| t.text().to_string());
            let name = toks.next().unwrap_or_default();
            // Skip `=` if present, take the following int literal.
            let value = v
                .children_with_tokens()
                .filter_map(|c| match c {
                    NodeOrToken::Token(t) if t.kind() == SyntaxKind::IntLiteral => {
                        Some(t.text().to_string())
                    }
                    _ => None,
                })
                .next();
            (name, value)
        })
        .collect();
    let max_name = parts.iter().map(|p| p.0.len()).max().unwrap_or(0);
    for (i, (name, value)) in parts.iter().enumerate() {
        w.write(name);
        if let Some(v) = value {
            w.pad_spaces(max_name.saturating_sub(name.len()));
            w.write(" = ");
            w.write(v);
        }
        if i + 1 < parts.len() || opts.trailing_comma {
            w.write(",");
        }
        w.newline();
    }
    w.dedent();
    w.write("}");
}

fn emit_extern_top(w: &mut FormatWriter, node: &SyntaxNode) {
    // Same shape as machine-scoped extern. We delegate to the same render
    // logic by re-using `emit_extern_decl` semantics inline.
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
    w.write("(");
    if let Some(params) = node.children().find(|n| n.kind() == SyntaxKind::PARAM_LIST) {
        let mut first = true;
        for p in params.children().filter(|n| n.kind() == SyntaxKind::PARAM) {
            if !first {
                w.write(", ");
            }
            first = false;
            let ty = p.children().find(|n| {
                n.kind() == SyntaxKind::TYPE_REF || n.kind() == SyntaxKind::OPAQUE_TYPE_REF
            });
            if let Some(t) = ty {
                emit_type_ref_public(w, &t);
                w.space();
            }
            if let Some(id) = iter_tokens(&p)
                .find(|t| t.kind() == SyntaxKind::Ident)
                .map(|t| t.text().to_string())
            {
                w.write(&id);
            }
        }
    }
    w.write(")");
    if let Some(ret) = node
        .children()
        .find(|n| n.kind() == SyntaxKind::TYPE_REF || n.kind() == SyntaxKind::OPAQUE_TYPE_REF)
    {
        w.write(" : ");
        emit_type_ref_public(w, &ret);
    }
}
