//! `fsm check` — parse + analyze + report diagnostics.
//!
//! Doc 23 §9 normative behaviour:
//!   - `fsm check motor.fsm`  → exit 0, no output (clean file).
//!   - `fsm check broken.fsm` → exit 1, Rust-style error with `--> file:line:col`.
//!
//! Per Doc 18 §3 the rendering format is the Rust-style block with caret;
//! the JSON formatter is opt-in via `--json` and writes to stdout.

use std::process::ExitCode;

use fsm_analyzer::analyze_with_source;
use fsm_parser::parse;

use crate::cli::CheckArgs;
use crate::diagnostics;

pub fn run(args: CheckArgs) -> ExitCode {
    let mut any_error = false;
    let mut all_diags = Vec::new();
    for path in &args.files {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {}: {}", path.display(), e);
                return ExitCode::from(3);
            }
        };
        let pr = parse(&src);
        let label = path.to_string_lossy().into_owned();
        let result = analyze_with_source(&pr, &label, &src);
        let mut diags = result.diagnostics;
        if args.warn_as_error {
            diagnostics::promote_warnings(&mut diags);
        }
        any_error |= diagnostics::any_errors(&diags);

        if args.json {
            // JSON is an aggregate document — collect across files and emit
            // once at the end so the array is well-formed.
            for d in diags {
                all_diags.push((d, src.clone(), label.clone()));
            }
        } else if !diags.is_empty() {
            diagnostics::render_human(&diags, &src, &label);
        }
    }

    if args.json {
        emit_json_aggregate(&all_diags);
    }

    if any_error {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Emits one well-formed JSON array containing every diagnostic across
/// every input file. Format per Doc 18 §3.
fn emit_json_aggregate(entries: &[(fsm_diagnostics::Diagnostic, String, String)]) {
    // We re-use render_json's serialiser by flattening into a single pass.
    // To keep render_json's signature stable (per-file), we do the work
    // inline here — the surface area is small enough that duplication is
    // cleaner than threading slices through.
    use serde_json::Value;
    let mut out: Vec<Value> = Vec::with_capacity(entries.len());
    for (d, src, path) in entries {
        let (l, c) = line_col(src, d.span.start);
        let (el, ec) = line_col(src, d.span.end);
        let mut obj = serde_json::Map::new();
        obj.insert("code".into(), Value::String(format!("{}", d.code)));
        obj.insert(
            "severity".into(),
            Value::String(match d.severity {
                fsm_diagnostics::Severity::Error => "error".into(),
                fsm_diagnostics::Severity::Warning => "warning".into(),
                fsm_diagnostics::Severity::Info => "info".into(),
                fsm_diagnostics::Severity::Hint => "hint".into(),
            }),
        );
        obj.insert("message".into(), Value::String(d.message.clone()));
        obj.insert("file".into(), Value::String(path.clone()));
        obj.insert("line".into(), Value::Number(l.into()));
        obj.insert("col".into(), Value::Number(c.into()));
        obj.insert("endLine".into(), Value::Number(el.into()));
        obj.insert("endCol".into(), Value::Number(ec.into()));
        if !d.related.is_empty() {
            let rels: Vec<Value> = d
                .related
                .iter()
                .map(|r| {
                    let (rl, rc) = line_col(src, r.span.start);
                    let (rel_l, rel_c) = line_col(src, r.span.end);
                    let mut m = serde_json::Map::new();
                    m.insert("file".into(), Value::String(path.clone()));
                    m.insert("line".into(), Value::Number(rl.into()));
                    m.insert("col".into(), Value::Number(rc.into()));
                    m.insert("endLine".into(), Value::Number(rel_l.into()));
                    m.insert("endCol".into(), Value::Number(rel_c.into()));
                    m.insert("message".into(), Value::String(r.message.clone()));
                    Value::Object(m)
                })
                .collect();
            obj.insert("relatedLocs".into(), Value::Array(rels));
        }
        obj.insert("fixable".into(), Value::Bool(false));
        out.push(Value::Object(obj));
    }
    let s = serde_json::to_string_pretty(&out).unwrap_or_else(|_| "[]".to_string());
    println!("{}", s);
}

fn line_col(src: &str, byte: usize) -> (u32, u32) {
    let mut line: u32 = 1;
    let mut col: u32 = 1;
    for (i, ch) in src.char_indices() {
        if i >= byte {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}
