//! `fsm generate` — full compile + C99 emit.
//!
//! Pipeline:
//! 1. Read source.
//! 2. Parse → analyze; abort on any error-severity diagnostic.
//! 3. Build a [`CodegenConfig`] from CLI flags merged over `fsm.toml`.
//! 4. Run `fsm_codegen_c::emit`.
//! 5. Write every [`EmittedFile`] to `--out`.
//! 6. If `--report-memory`, print the [`MemoryBudget`] for each machine.
//!
//! Exit codes: 0 ok / 1 diagnostics / 2 codegen ICE / 3 file not found.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fsm_analyzer::analyze_with_source;
use fsm_codegen_c::{compute_budget, emit, CodegenConfig, DispatchStrategy};
use fsm_parser::parse;

use crate::cli::GenerateArgs;
use crate::config;
use crate::diagnostics;
use crate::import_header::{parse_header_file, ImportedHeader};

pub fn run(args: GenerateArgs) -> ExitCode {
    // v1.0 only ships C99 (cpp17 is on the roadmap but not in fsm-cli yet).
    if args.target != "c99" {
        eprintln!(
            "error: unknown target `{}` — v1.0 only ships `c99`",
            args.target
        );
        return ExitCode::from(2);
    }

    // Load fsm.toml from the parent directory of the first input file. The
    // walk-upwards loader matches `cargo`'s manifest discovery semantics
    // (Doc 18 §6.1).
    let first = args
        .files
        .first()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));
    let search_dir: &Path = first.parent().unwrap_or_else(|| Path::new("."));
    let toml_cfg = match config::load(search_dir) {
        Ok(Some((_, cfg))) => Some(cfg),
        Ok(None) => None,
        Err(e) => {
            eprintln!("error: fsm.toml: {}", e);
            return ExitCode::from(4);
        }
    };

    // Construct CodegenConfig: TOML provides the defaults, CLI overrides.
    let codegen_cfg = match build_codegen_config(&args, toml_cfg.as_ref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::from(4);
        }
    };

    // Collect `--import-header` paths + `fsm.toml [generate] import_headers`
    // and parse them ONCE. The resulting externs are injected into every
    // machine's IR below, before codegen, so an imported extern is
    // indistinguishable downstream from a `.fsm`-declared one (Doc 18 §5).
    // A header read/parse failure is exit-3 (file not found) like a missing
    // `.fsm` — the user asked for that header explicitly.
    let imported = match collect_imported_externs(&args, toml_cfg.as_ref(), search_dir) {
        Ok(i) => i,
        Err(code) => return code,
    };

    // Make sure --out exists before we start emitting.
    if let Err(e) = fs::create_dir_all(&args.out) {
        eprintln!(
            "error: cannot create output dir {}: {}",
            args.out.display(),
            e
        );
        return ExitCode::from(2);
    }

    // Emit the import notes (skipped constructs + cross-header dupes) ONCE,
    // not per input file. FSM-W level — advisory, never blocks codegen.
    for note in &imported.skipped {
        eprintln!(
            "note: [import-header] skipped `{}` — {}",
            note.fragment, note.reason
        );
    }

    let mut any_error = false;
    for path in &args.files {
        let raw = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {}: {}", path.display(), e);
                return ExitCode::from(3);
            }
        };
        let label = path.to_string_lossy().into_owned();

        // Splice the imported externs into the source as DSL `extern`
        // declarations BEFORE parsing, so they flow through the *entire*
        // normal pipeline (parser → symbol table → name resolution →
        // lowering → codegen) and are indistinguishable from hand-written
        // `extern`s — including for the analyzer's `FSM-E0102` unknown-
        // extern check, which queries the AST symbol table, not the IR.
        //
        // Conflict rule: an extern already declared in the `.fsm` WINS — we
        // do not re-emit a synthesized line for that name (re-emitting
        // would also trip the analyzer's own duplicate-extern `FSM-E0024`).
        // This is the conservative choice: the DSL is the user's explicit,
        // in-repo source of truth and may carry `pure` (a header cannot
        // tell us purity); letting a header silently override it could
        // change guard eligibility.
        let existing = existing_extern_names(&raw);
        let (extern_block, shadowed) = imported.to_dsl_externs(&existing);
        for name in &shadowed {
            eprintln!(
                "note: [import-header] `{}` is also declared as an `extern` \
                 in {} — the .fsm declaration wins; the imported one is \
                 ignored",
                name, label
            );
        }
        let src = splice_extern_block(&raw, &extern_block);

        let pr = parse(&src);
        let result = analyze_with_source(&pr, &label, &src);
        let diags = result.diagnostics;
        if !diags.is_empty() {
            diagnostics::render_human(&diags, &src, &label);
        }
        if diagnostics::any_errors(&diags) {
            any_error = true;
            continue;
        }
        let Some(ir) = result.ir else {
            eprintln!("error: analyzer returned no IR for {}", path.display());
            any_error = true;
            continue;
        };

        let emitted = match emit(&ir, &codegen_cfg) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("error: codegen-c: {}", e);
                return ExitCode::from(2);
            }
        };

        for f in &emitted.files {
            let dest = args.out.join(&f.path);
            if let Some(parent) = dest.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    eprintln!("error: cannot create directory {}: {}", parent.display(), e);
                    return ExitCode::from(2);
                }
            }
            if let Err(e) = fs::write(&dest, &f.content) {
                eprintln!("error: cannot write {}: {}", dest.display(), e);
                return ExitCode::from(2);
            }
            eprintln!("wrote {}", dest.display());
        }

        if args.emit_ir {
            let ir_path = args.out.join(format!(
                "{}.ir.json",
                ir.machines
                    .first()
                    .map(|m| m.name.as_str())
                    .unwrap_or("machine")
            ));
            match fsm_ir::to_json(&ir) {
                Ok(s) => {
                    if let Err(e) = fs::write(&ir_path, s) {
                        eprintln!("error: cannot write {}: {}", ir_path.display(), e);
                        return ExitCode::from(2);
                    }
                    eprintln!("wrote {}", ir_path.display());
                }
                Err(e) => {
                    eprintln!("error: ir serialise: {}", e);
                    return ExitCode::from(2);
                }
            }
        }

        if args.report_memory {
            // `compute_budget` reports against the first machine in the IR
            // document. Multi-machine IRs in v1.0 are an edge case; per-
            // machine reporting can ride a future flag.
            if !ir.machines.is_empty() {
                // Audit P1-8: `compute_budget` now returns Result so that a
                // >255-state machine surfaces as a clean CLI error instead
                // of a `panic!` + backtrace.
                let budget = match compute_budget(&ir, &codegen_cfg) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("error: codegen-c: {}", e);
                        return ExitCode::from(2);
                    }
                };
                let name = ir.machines[0].name.as_str();
                println!("Memory budget for machine `{}`:", name);
                println!("  sizeof(Machine_t)     = {} bytes", budget.sizeof_machine);
                println!("    context fields      = {}", budget.sizeof_context);
                println!("    event queue         = {}", budget.queue_bytes);
                println!("    history slots       = {}", budget.history_bytes);
                println!("    timer slots         = {}", budget.timer_bytes);
                println!("  total RAM             = {}", budget.total_ram_bytes);
                println!("  estimated ROM (text)  = {}", budget.estimated_rom_bytes);
                println!("  max completion depth  = {}", budget.max_completion_depth);
            }
        }
    }

    if any_error {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Resolve `--import-header` + `fsm.toml [generate] import_headers` paths,
/// read + parse each, and return the merged set of importable externs plus
/// every skip-note (surfaced to the user once, not per-machine).
///
/// Path resolution: an absolute path is used as-is; a relative path is
/// resolved against `search_dir` (the first `.fsm`'s directory — the same
/// anchor `fsm.toml` discovery uses), then falls back to the process CWD so
/// `fsm generate --import-header ./driver.h foo.fsm` works from either.
/// A missing/unreadable header is exit-3 (the user named it explicitly).
fn collect_imported_externs(
    args: &GenerateArgs,
    toml_cfg: Option<&config::FsmToml>,
    search_dir: &Path,
) -> Result<ImportedHeader, ExitCode> {
    // TOML list first (project-wide), then CLI (invocation-specific) — both
    // contribute; later entries that name a duplicate function lose to
    // earlier ones in `inject` (first-wins among imports), but a name
    // appearing twice across headers is itself worth a note.
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(cfg) = toml_cfg {
        paths.extend(cfg.generate.import_headers.iter().cloned());
    }
    paths.extend(args.import_header.iter().cloned());

    let mut merged = ImportedHeader::default();
    for p in &paths {
        let resolved = resolve_header_path(p, search_dir);
        let parsed = match parse_header_file(&resolved) {
            Ok(h) => h,
            Err(e) => {
                eprintln!(
                    "error: cannot read --import-header {}: {}",
                    resolved.display(),
                    e
                );
                return Err(ExitCode::from(3));
            }
        };
        for ext in parsed.externs {
            if merged.externs.iter().any(|x| x.name == ext.name) {
                merged.skipped.push(crate::import_header::SkipNote {
                    fragment: ext.name.clone(),
                    reason: format!(
                        "duplicate import: `{}` declared in more than one \
                         imported header — keeping the first",
                        ext.name
                    ),
                });
            } else {
                merged.externs.push(ext);
            }
        }
        merged.skipped.extend(parsed.skipped);
    }
    Ok(merged)
}

/// Resolve one header path: absolute → as-is; relative → try `search_dir`
/// then the process CWD.
fn resolve_header_path(p: &Path, search_dir: &Path) -> PathBuf {
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let anchored = search_dir.join(p);
    if anchored.is_file() {
        return anchored;
    }
    if p.is_file() {
        return p.to_path_buf();
    }
    // Neither exists — return the anchored form so the error message points
    // at the path the user most likely meant.
    anchored
}

/// Extern names already declared in the `.fsm` source (file-level + every
/// machine + submachine). Used so the synthesized import block does NOT
/// re-declare a name the user already wrote (DSL-wins conflict rule), which
/// would also trip the analyzer's duplicate-extern `FSM-E0024`.
///
/// We parse the *original* source (cheap; the parser is allocation-light
/// and error-tolerant) and walk the typed AST rather than string-scanning
/// for `extern`, so a commented-out or string-embedded `extern` is not
/// mistaken for a real declaration.
fn existing_extern_names(src: &str) -> Vec<String> {
    let pr = parse(src);
    let file = pr.ast();
    let mut names: Vec<String> = file.externs().filter_map(|e| e.name()).collect();
    for m in file.machines() {
        names.extend(m.externs().filter_map(|e| e.name()));
    }
    for sm in file.submachines() {
        // `SubmachineDecl` reuses the machine-item accessors (Doc 04 §15).
        names.extend(sm.externs().filter_map(|e| e.name()));
    }
    names.sort();
    names.dedup();
    names
}

/// Splice a synthesized DSL `extern` block into the source.
///
/// Top-level `extern` declarations are legal anywhere before/among other
/// file-scope items (Doc 04 §2.5 / grammar `file = { … | extern_decl }`).
/// We insert the block immediately AFTER the `language fsm X.Y` header line
/// if present (so the version pragma stays first, as the lexer/grammar
/// expect), otherwise at the very top. A trailing blank line keeps the
/// emitted-into source readable if a human ever inspects it. Empty block ⇒
/// source returned unchanged (zero behavioural change when no headers).
fn splice_extern_block(src: &str, extern_block: &str) -> String {
    if extern_block.trim().is_empty() {
        return src.to_string();
    }
    // Find the end of the `language` line (first non-blank, non-comment
    // line is the `language` pragma per Doc 04 §1). Insert right after it.
    if let Some(lang_idx) = src.find("language") {
        // Splice after the newline that terminates the language line.
        if let Some(nl) = src[lang_idx..].find('\n') {
            let cut = lang_idx + nl + 1;
            let mut out = String::with_capacity(src.len() + extern_block.len() + 64);
            out.push_str(&src[..cut]);
            out.push_str("\n// --- externs imported via --import-header ---\n");
            out.push_str(extern_block);
            out.push('\n');
            out.push_str(&src[cut..]);
            return out;
        }
    }
    // No `language` line found (analyzer will diagnose that separately);
    // prepend the block so we don't lose the imports.
    format!(
        "// --- externs imported via --import-header ---\n{}\n{}",
        extern_block, src
    )
}

/// Merge TOML + CLI into a final [`CodegenConfig`]. CLI flags ALWAYS win;
/// TOML supplies fallbacks; the codegen default supplies fallbacks for
/// keys neither source mentions (Doc 18 §6.1 last-writer-wins).
fn build_codegen_config(
    args: &GenerateArgs,
    toml_cfg: Option<&config::FsmToml>,
) -> Result<CodegenConfig, String> {
    let mut cfg = CodegenConfig::default();
    // Strategy: TOML first, then CLI.
    let cli_strategy = args.strategy.as_str();
    let toml_strategy = toml_cfg.and_then(|c| c.generate.strategy.as_deref());
    let strategy_str = if cli_strategy == "auto" {
        // The default — fall back to TOML if it sets something else.
        toml_strategy.unwrap_or("auto")
    } else {
        cli_strategy
    };
    cfg.strategy = match strategy_str {
        "switch" | "switch_based" => DispatchStrategy::Switch,
        "table" | "table_driven" => DispatchStrategy::Table,
        "auto" => DispatchStrategy::Auto,
        other => return Err(format!("unknown strategy `{}`", other)),
    };

    // Queue capacity.
    let toml_qs = toml_cfg.and_then(|c| c.generate.queue_size);
    if let Some(q) = args.queue_size.or(toml_qs) {
        cfg.queue_capacity = q;
    }

    // License: CLI first, TOML fallback, default MIT (Doc 00 §10.4).
    let toml_license = toml_cfg.and_then(|c| c.generate.license.as_deref());
    cfg.license_spdx = if args.license != "MIT" {
        args.license.clone()
    } else {
        toml_license.unwrap_or("MIT").to_owned()
    };

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_config_defaults_to_auto_strategy() {
        let args = GenerateArgs {
            target: "c99".into(),
            out: PathBuf::from("/tmp"),
            strategy: "auto".into(),
            queue_size: None,
            license: "MIT".into(),
            report_memory: false,
            emit_ir: false,
            import_header: vec![],
            files: vec![],
        };
        let cfg = build_codegen_config(&args, None).unwrap();
        assert_eq!(cfg.strategy, DispatchStrategy::Auto);
        assert_eq!(cfg.license_spdx, "MIT");
    }

    #[test]
    fn build_config_cli_license_wins_over_toml() {
        let mut t = config::FsmToml::default();
        t.generate.license = Some("Apache-2.0".into());
        let args = GenerateArgs {
            target: "c99".into(),
            out: PathBuf::from("/tmp"),
            strategy: "auto".into(),
            queue_size: None,
            license: "BSD-2-Clause".into(),
            report_memory: false,
            emit_ir: false,
            import_header: vec![],
            files: vec![],
        };
        let cfg = build_codegen_config(&args, Some(&t)).unwrap();
        assert_eq!(cfg.license_spdx, "BSD-2-Clause");
    }

    #[test]
    fn build_config_toml_fills_when_cli_is_default() {
        let mut t = config::FsmToml::default();
        t.generate.license = Some("Apache-2.0".into());
        let args = GenerateArgs {
            target: "c99".into(),
            out: PathBuf::from("/tmp"),
            strategy: "auto".into(),
            queue_size: None,
            license: "MIT".into(), // default — TOML should win
            report_memory: false,
            emit_ir: false,
            import_header: vec![],
            files: vec![],
        };
        let cfg = build_codegen_config(&args, Some(&t)).unwrap();
        assert_eq!(cfg.license_spdx, "Apache-2.0");
    }
}
