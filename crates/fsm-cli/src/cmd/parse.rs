//! `fsm parse` — tokenize + parse + dump CST/AST as JSON.
//!
//! Doc 18 §5 "fsm parse" notes this is for syntax-level debugging — it does
//! NOT run semantic analysis. Diagnostics from the parser are reported in
//! human form on stderr (a JSON-flag for this command was not specified in
//! Doc 18 §5 so we omit it for v1.0).
//!
//! With neither `--emit-cst` nor `--emit-ast`, the command behaves like a
//! quick "did this even parse?" check — silent on success, diagnostics on
//! failure.

use std::process::ExitCode;

use fsm_parser::{parse, SyntaxNode};
use serde_json::{json, Value};

use crate::cli::ParseArgs;
use crate::diagnostics;

pub(crate) fn run(args: ParseArgs) -> ExitCode {
    let mut had_errors = false;
    for path in &args.files {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {}: {}", path.display(), e);
                return ExitCode::from(3);
            }
        };
        let pr = parse(&src);
        if !pr.errors.is_empty() {
            had_errors |= diagnostics::any_errors(&pr.errors);
            diagnostics::render_human(&pr.errors, &src, &path.to_string_lossy());
        }

        if args.emit_cst {
            let syntax = pr.syntax();
            let v = cst_to_json(&syntax);
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        if args.emit_ast {
            // The parser exposes a typed AST view but it is a thin wrapper
            // around the rowan CST. For machine-readable output, the CST
            // dump is the right shape — the typed AST is a lossy view. We
            // emit a higher-level summary (file → machines → states) so
            // tests can introspect without learning the rowan kind table.
            let v = ast_to_json(&pr.ast());
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        }
    }
    if had_errors {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Recursive JSON dump of the rowan CST. Each node is `{"kind": …, "text":
/// raw, "children": […]}` and each leaf token is `{"kind": …, "text":
/// raw}`. Layout is intentionally verbose — this is a debug tool.
fn cst_to_json(node: &SyntaxNode) -> Value {
    let mut children = Vec::new();
    for ch in node.children_with_tokens() {
        // `children_with_tokens` returns rowan's NodeOrToken variant. We
        // discriminate via the typed accessors rather than naming the
        // rowan path directly — fsm-parser does not re-export `rowan`.
        if let Some(n) = ch.as_node() {
            children.push(cst_to_json(n));
        } else if let Some(t) = ch.as_token() {
            children.push(json!({
                "kind": format!("{:?}", t.kind()),
                "text": t.text().to_string(),
            }));
        }
    }
    json!({
        "kind": format!("{:?}", node.kind()),
        "range": [u32::from(node.text_range().start()), u32::from(node.text_range().end())],
        "children": children,
    })
}

/// Lightweight AST summary — just enough for tests + machine-readable
/// tooling to introspect the file shape. NOT a full structural mirror;
/// callers needing every detail should use `--emit-cst`.
fn ast_to_json(file: &fsm_parser::ast::File) -> Value {
    let machines: Vec<_> = file
        .machines()
        .map(|m| {
            json!({
                "name": m.name().unwrap_or_default(),
            })
        })
        .collect();
    json!({
        "kind": "File",
        "machines": machines,
    })
}
