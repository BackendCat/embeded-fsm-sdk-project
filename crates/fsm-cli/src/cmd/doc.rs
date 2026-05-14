//! `fsm doc` — Markdown documentation generator.
//!
//! v1.0 ships a one-page "Machine: NAME / States: …" summary. Full doc
//! comment extraction + state-diagram art lands in v1.1 once the LSP hover
//! pipeline ships a structured doc-comment reader.

use std::io::Write;
use std::process::ExitCode;

use fsm_analyzer::analyze_with_source;
use fsm_ir::{Ir, StateNode};
use fsm_parser::parse;

use crate::cli::DocArgs;
use crate::diagnostics;

pub fn run(args: DocArgs) -> ExitCode {
    let src = match std::fs::read_to_string(&args.file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", args.file.display(), e);
            return ExitCode::from(3);
        }
    };
    let pr = parse(&src);
    let label = args.file.to_string_lossy().into_owned();
    let res = analyze_with_source(&pr, &label, &src);
    if diagnostics::any_errors(&res.diagnostics) {
        diagnostics::render_human(&res.diagnostics, &src, &label);
        return ExitCode::from(1);
    }
    let Some(ir) = res.ir else {
        eprintln!("error: analyzer returned no IR");
        return ExitCode::from(2);
    };
    let body = render_markdown(&ir);
    match &args.out {
        Some(path) => {
            if let Err(e) = std::fs::write(path, body) {
                eprintln!("error: cannot write {}: {}", path.display(), e);
                return ExitCode::from(2);
            }
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            let _ = stdout.write_all(body.as_bytes());
        }
    }
    ExitCode::SUCCESS
}

fn render_markdown(ir: &Ir) -> String {
    let mut out = String::new();
    out.push_str("# FSM Documentation\n\n");
    for m in &ir.machines {
        out.push_str(&format!("## Machine: {}\n\n", m.name));
        out.push_str("### States\n\n");
        collect_states(&m.root.states, &mut out);
        out.push('\n');
        if !m.events.is_empty() {
            out.push_str("### Events\n\n");
            for e in &m.events {
                out.push_str(&format!("- `{}`\n", e.name));
            }
            out.push('\n');
        }
        if !m.externs.is_empty() {
            out.push_str("### Externs\n\n");
            for x in &m.externs {
                out.push_str(&format!("- `{}`\n", x.name));
            }
            out.push('\n');
        }
    }
    out
}

fn collect_states(states: &[StateNode], out: &mut String) {
    for s in states {
        match s {
            StateNode::Simple(ss) => out.push_str(&format!("- `{}` (simple)\n", ss.name)),
            StateNode::Composite(cs) => {
                out.push_str(&format!("- `{}` (composite)\n", cs.name));
                for r in &cs.regions {
                    collect_states(&r.states, out);
                }
            }
            StateNode::Parallel(ps) => {
                out.push_str(&format!("- `{}` (parallel)\n", ps.name));
                for r in &ps.regions {
                    collect_states(&r.states, out);
                }
            }
            StateNode::Final(f) => out.push_str(&format!("- `{}` (final)\n", f.id)),
            _ => {}
        }
    }
}
