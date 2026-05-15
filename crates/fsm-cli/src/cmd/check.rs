//! `fsm check` — parse + analyze + report diagnostics.
//!
//! Doc 23 §9 normative behaviour:
//!   - `fsm check motor.fsm`  → exit 0, no output (clean file).
//!   - `fsm check broken.fsm` → exit 1, Rust-style error with `--> file:line:col`.
//!
//! Per Doc 18 §3 the rendering format is the Rust-style block with caret;
//! the JSON formatter is opt-in via `--json` and writes to stdout.
//!
//! ## Import-path security pass (Doc 00 §7.12 G-02 / audit P1-4)
//!
//! After parsing, every `import "..."` declaration is run through
//! [`fsm_parser::import_resolver::resolve_import`] using the workspace
//! root discovered via the `fsm.toml` walker. The shape check has already
//! happened in the parser; the resolver adds canonicalize-plus-prefix
//! containment so a workspace-relative symlink pointing outside the
//! workspace is rejected. Unresolvable imports are *not* a hard error
//! here — `fsm check` is run against a single file and the imported
//! sibling may not exist yet in author workflows. The escape check (out
//! of workspace) IS a hard error because it indicates an unsafe import
//! regardless of file presence.

use std::path::Path;
use std::process::ExitCode;

use fsm_analyzer::analyze_with_source;
use fsm_diagnostics::{Diagnostic, Span};
use fsm_parser::ast::{AstNode, File as AstFile};
use fsm_parser::import_resolver::{resolve_import, ImportError};
use fsm_parser::{parse, ParseResult};

use crate::cli::CheckArgs;
use crate::{config, diagnostics};

pub(crate) fn run(args: CheckArgs) -> ExitCode {
    // Load the project `fsm.toml` once so `[compiler] allow`/`deny`
    // (Doc 18 §6) can be applied at the diagnostic-finalization point
    // below, exactly as `cmd::generate` loads it for `[generate]`. The
    // search starts at the first input file's directory and walks up
    // (config.rs `load`). A malformed `fsm.toml` is a clean exit-4
    // config error — identical handling to `cmd::generate` (a file the
    // user clearly meant to be honoured but cannot be parsed must fail
    // loud, never be silently skipped). `allow`/`deny` default to empty
    // when there is no `fsm.toml` or no `[compiler]` block, so behaviour
    // is byte-identical to pre-FU#67 for every project that does not use
    // the feature.
    let search_dir = args
        .files
        .first()
        .and_then(|p| p.parent())
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| Path::new(".").to_path_buf());
    let compiler_cfg = match config::load(&search_dir) {
        Ok(Some((_, cfg))) => cfg.compiler,
        Ok(None) => config::CompilerSection::default(),
        Err(e) => {
            eprintln!("error: fsm.toml: {}", e);
            return ExitCode::from(4);
        }
    };

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
        // Resolve the workspace root once per file — the resolver walks
        // upwards from the file's parent looking for `fsm.toml`. A
        // missing `fsm.toml` falls back to the file's directory; that is
        // the most permissive position the resolver can hold without
        // exposing escape paths (everything outside that directory is
        // still rejected by `resolve_import`).
        //
        // SEC-P0-1 convergence: this is the SINGLE shared definition in
        // `safe_io`, used identically by `fsm generate`'s header-path
        // containment, so the boundary is the same for a DSL `import` and
        // an `fsm.toml import_headers` entry.
        let workspace_root = crate::safe_io::workspace_root_for(path);
        let mut import_diags = security_check_imports(&pr, path, &workspace_root);
        let result = analyze_with_source(&pr, &label, &src);
        let mut diags = result.diagnostics;
        // Surface import-security diagnostics alongside parse/analyze
        // diagnostics — they share the renderer.
        diags.append(&mut import_diags);
        // Apply `fsm.toml [compiler] allow`/`deny` (Doc 18 §6) at the
        // SAME finalization stage as `--warn-as-error`: after the full
        // diagnostic set for this file exists, before it influences the
        // renderer OR the exit code. `allow` removes suppressed codes;
        // `deny` elevates listed warnings to errors. An unknown code in
        // either list is a clean exit-4 config error (never a silent
        // ignore — the FU#67 cardinal-sin guard) and aborts before any
        // diagnostic is rendered, so the user sees only the config error.
        if let Err(e) =
            diagnostics::apply_allow_deny(&mut diags, &compiler_cfg.allow, &compiler_cfg.deny)
        {
            eprintln!("error: {}", e);
            return ExitCode::from(4);
        }
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

/// Run every `import "..."` declaration through `resolve_import` so any
/// workspace-relative symlink that escapes the workspace is rejected.
/// Returns diagnostics in source order. Unresolvable imports
/// (`ImportError::Unresolved`) are intentionally **not** flagged here:
/// `fsm check` is run on a single file in author workflows where sibling
/// `.fsm` files may not exist yet. The escape check (`OutsideWorkspace`)
/// is always a hard error; shape failures have already been caught at
/// parse time and produce a duplicate, suppressed by the dedupe at the
/// bottom of the loop.
fn security_check_imports(
    pr: &ParseResult,
    file_path: &Path,
    workspace_root: &Path,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let file = AstFile::cast(pr.syntax()).expect("root node is always FILE");
    for imp in file.imports() {
        let Some(raw) = imp.path() else { continue };
        // Recover the path span via the IMPORT_DECL subtree — first
        // StringLiteral token.
        let span = imp
            .syntax()
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| t.kind() == fsm_parser::SyntaxKind::StringLiteral)
            .map(|t| {
                let r = t.text_range();
                Span::new(usize::from(r.start()), usize::from(r.end()))
            })
            .unwrap_or_else(|| Span::new(0, 0));
        match resolve_import(workspace_root, file_path, &raw) {
            Ok(_) => {}
            Err(ImportError::BadShape { .. }) => {
                // Already diagnosed by the parser's shape check — silent
                // here to avoid duplicate noise in the report.
            }
            Err(ImportError::Unresolved) => {
                // Not flagged in `fsm check` — sibling file may not
                // exist yet. The build driver (`fsm generate`) is the
                // right place to fail-loud on unresolvable imports.
            }
            Err(err @ ImportError::OutsideWorkspace) => {
                out.push(err.into_diagnostic(span));
            }
        }
    }
    out
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
