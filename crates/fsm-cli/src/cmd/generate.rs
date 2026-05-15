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
use fsm_parser::import_resolver::{resolve_import, ImportError};
use fsm_parser::parse;

use crate::cli::GenerateArgs;
use crate::config;
use crate::diagnostics;
use crate::import_header::{parse_header_file, ImportedHeader};

pub(crate) fn run(args: GenerateArgs) -> ExitCode {
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
    let imported = match collect_imported_externs(&args, toml_cfg.as_ref(), search_dir, &first) {
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

    // W7: every top-level machine name actually generated, accumulated
    // across all input files so a project-wide `fsm.toml` carrying a
    // `[machine.X]` for a machine that lives in a file NOT passed this
    // invocation can be detected and WARNed about (not hard-failed —
    // multi-file projects legitimately share one fsm.toml; see Doc 00
    // §11.25). Submachine template names are intentionally excluded: a
    // submachine has no user-facing `[machine.X]` override surface (W7
    // makes submachines inherit the parent's dispatch family), so a
    // `[machine.<submachine>]` entry IS an unmatched name worth warning.
    let mut generated_machines: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();

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

        // W7: record the top-level machine names this file contributed so
        // a `[machine.X]` naming a never-generated machine can be WARNed
        // once after all files are processed.
        for m in &ir.machines {
            generated_machines.insert(m.name.clone());
        }

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

    // W7 (Doc 00 §11.25): a `[machine.X]` strategy override whose machine
    // was NOT generated this invocation is a WARN, not a hard error — a
    // multi-file project legitimately carries one project-wide fsm.toml,
    // and `fsm generate` on a subset of its `.fsm` files must still
    // succeed. Warning (not erroring) also catches the common typo case
    // helpfully without blocking an otherwise-valid build. Suppressed when
    // a real error already aborted a file (the machine set would be
    // misleadingly incomplete, producing false "unmatched" noise).
    if !any_error {
        for name in codegen_cfg.machine_strategy_overrides.keys() {
            if !generated_machines.contains(name) {
                eprintln!(
                    "warning: fsm.toml [machine.{}] sets a strategy override, \
                     but no machine named `{}` was generated by this invocation \
                     (the override had no effect here; this is expected for a \
                     project-wide fsm.toml generating a subset of its .fsm files)",
                    name, name
                );
            }
        }
    }

    if any_error {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Where a header path came from. This is a **trust-level** distinction,
/// not a cosmetic one — it decides whether workspace-root containment is
/// enforced (SEC-P0-1 deliberate decision; see [`resolve_header_path`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderSource {
    /// `fsm.toml [generate] import_headers`. The `fsm.toml` travels WITH
    /// the project tree, so its contents are **attacker-controlled** under
    /// the G-02 threat model (a crafted repo on shared CI). Containment is
    /// enforced to parity with a DSL `import "..."` unless the project
    /// explicitly opts out via `allow_unscoped_import_headers`.
    FsmToml,
    /// `--import-header` CLI flag. This is **invocation-supplied** — the
    /// same trust level as the `.fsm` path argument and `--out`. The G-02
    /// threat model scopes the danger to "a `.fsm` *source* (or project
    /// file) posted to a shared build host", NOT to the CI job's own
    /// argv. A vendored HAL at an absolute path is the *documented normal*
    /// use of this flag (the W5 example itself passes an absolute path), so
    /// hard-rejecting absolute paths here would be both wrong for the
    /// threat model and a functional regression. Still shape-sane + size-
    /// capped; just not workspace-contained.
    CliFlag,
}

/// Resolve `--import-header` + `fsm.toml [generate] import_headers` paths,
/// read + parse each, and return the merged set of importable externs plus
/// every skip-note (surfaced to the user once, not per-machine).
///
/// SECURITY (SEC-P0-1, Doc 00 §G-02 / Doc 18 §10): every path is routed
/// through [`resolve_header_path`], which converges on the SAME hardening
/// primitives the DSL `import "..."` path uses — there is no second copy of
/// the containment logic (it calls `fsm_parser::import_resolver::
/// resolve_import` directly). The size cap is enforced inside
/// `parse_header_file` via the shared bounded read. A missing/unreadable/
/// oversized header is exit-3 (the user named it explicitly — same as a
/// missing `.fsm`); a containment-escape is exit-1 (a validation error in
/// attacker-influenced input, mirroring how the DSL `import` escape maps).
fn collect_imported_externs(
    args: &GenerateArgs,
    toml_cfg: Option<&config::FsmToml>,
    search_dir: &Path,
    first_fsm: &Path,
) -> Result<ImportedHeader, ExitCode> {
    // TOML list first (project-wide), then CLI (invocation-specific) — both
    // contribute; later entries that name a duplicate function lose to
    // earlier ones in `inject` (first-wins among imports), but a name
    // appearing twice across headers is itself worth a note. Each path
    // carries its provenance so the trust-level (containment-or-not)
    // decision is explicit per entry.
    let mut paths: Vec<(PathBuf, HeaderSource)> = Vec::new();
    if let Some(cfg) = toml_cfg {
        for p in &cfg.generate.import_headers {
            paths.push((p.clone(), HeaderSource::FsmToml));
        }
    }
    for p in &args.import_header {
        paths.push((p.clone(), HeaderSource::CliFlag));
    }

    // Workspace root for the containment check — the SAME resolver
    // `fsm check` uses for DSL `import "..."`, so the boundary is identical
    // for both surfaces (one definition in `safe_io`, no drift).
    let workspace_root = crate::safe_io::workspace_root_for(first_fsm);
    let allow_unscoped = toml_cfg
        .map(|c| c.generate.allow_unscoped_import_headers)
        .unwrap_or(false);

    let mut merged = ImportedHeader::default();
    for (p, src) in &paths {
        let resolved = match resolve_header_path(
            p,
            *src,
            search_dir,
            &workspace_root,
            first_fsm,
            allow_unscoped,
        ) {
            Ok(r) => r,
            Err(code) => return Err(code),
        };
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

/// Resolve one header path with the trust-appropriate hardening
/// (SEC-P0-1 — the convergent fix).
///
/// **The containment logic is NOT re-implemented here.** For the
/// attacker-controlled surface this calls
/// `fsm_parser::import_resolver::resolve_import` — the *exact same*
/// primitive the DSL `import "..."` path uses (shape-reject `..`/NUL/
/// absolute/UNC → `canonicalize` → workspace-root prefix). There is
/// deliberately no third variant of that logic anywhere.
///
/// Trust model (the deliberate absolute-path decision):
///
/// - [`HeaderSource::FsmToml`] — attacker-controlled (the `fsm.toml`
///   travels with a possibly-hostile repo). Routed through `resolve_import`
///   for full DSL-`import` parity. The legitimate vendored-HAL-via-config
///   case is served by the **explicit, named, default-off** opt-in
///   `[generate] allow_unscoped_import_headers = true`, which downgrades it
///   to the trusted-invoker treatment below — never a silent allow, and the
///   DoS size cap still applies regardless.
/// - [`HeaderSource::CliFlag`] — trusted invocation input (same trust as
///   the `.fsm` path arg). Absolute vendored-HAL paths are the documented
///   normal use, so containment is intentionally NOT enforced; only a
///   defensive NUL-byte shape reject is applied (the size cap is enforced
///   downstream in `parse_header_file`). This is the behaviour the W5
///   example + tests already rely on.
///
/// Returns the resolved path on success, or a clean non-zero `ExitCode`
/// (1 = containment/shape violation; 3 = unresolvable) with a precise
/// diagnostic — never a panic.
fn resolve_header_path(
    p: &Path,
    source: HeaderSource,
    search_dir: &Path,
    workspace_root: &Path,
    first_fsm: &Path,
    allow_unscoped: bool,
) -> Result<PathBuf, ExitCode> {
    // Defensive shape reject applied to BOTH trust levels: a NUL byte in a
    // path is never legitimate and is cheap to reject up front (the lexer
    // does the same for DSL imports — defence in depth, Doc 18 §10).
    if p.as_os_str().as_encoded_bytes().contains(&0) {
        eprintln!(
            "error: import-header path {} contains a NUL byte (rejected)",
            p.display()
        );
        return Err(ExitCode::from(1));
    }

    let contained = match source {
        HeaderSource::FsmToml => !allow_unscoped,
        HeaderSource::CliFlag => false,
    };

    if contained {
        // ATTACKER-CONTROLLED surface → identical hardening to a DSL
        // `import "..."`. `resolve_import` resolves the path relative to
        // the importing file's directory (here: the first `.fsm`, the
        // same anchor `fsm.toml` discovery uses), canonicalizes it
        // (symlink-resolved), and asserts it stays under the workspace
        // root. We pass the raw textual path so the shape check sees
        // exactly what the user wrote (`../../../etc/shadow` is rejected
        // at the shape stage before any I/O).
        let raw = p.to_string_lossy();
        match resolve_import(workspace_root, first_fsm, &raw) {
            Ok(canonical) => Ok(canonical),
            Err(ImportError::BadShape { reason }) => {
                eprintln!(
                    "error: fsm.toml import_headers entry {:?} is not a safe \
                     path: {} (paths must stay inside the workspace; set \
                     [generate] allow_unscoped_import_headers = true to \
                     permit an out-of-tree vendored header)",
                    raw, reason
                );
                Err(ExitCode::from(1))
            }
            Err(ImportError::OutsideWorkspace) => {
                eprintln!(
                    "error: fsm.toml import_headers entry {:?} resolves \
                     outside the workspace root (rejected as a containment \
                     escape; set [generate] allow_unscoped_import_headers = \
                     true only if this out-of-tree header is trusted)",
                    raw
                );
                Err(ExitCode::from(1))
            }
            Err(ImportError::Unresolved) => {
                eprintln!(
                    "error: fsm.toml import_headers entry {:?} could not be \
                     resolved on the filesystem",
                    raw
                );
                Err(ExitCode::from(3))
            }
        }
    } else {
        // TRUSTED-INVOKER surface (CLI flag, or fsm.toml with the explicit
        // opt-in): preserve the established resolution — absolute → as-is;
        // relative → `search_dir` then the process CWD. The size cap still
        // applies in `parse_header_file`; only containment is relaxed.
        if p.is_absolute() {
            return Ok(p.to_path_buf());
        }
        let anchored = search_dir.join(p);
        if anchored.is_file() {
            return Ok(anchored);
        }
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
        // Neither exists — return the anchored form so the error message
        // points at the path the user most likely meant.
        Ok(anchored)
    }
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

/// Parse one dispatch-strategy string to the codegen enum. The accepted
/// spellings mirror the CLI `value_parser` plus the historical
/// `switch_based`/`table_driven` aliases the global path already honoured;
/// keeping ONE parser means a `[machine.M]` override and the global
/// `--strategy`/`[generate].strategy` reject identically (zero-legacy: no
/// second, drifting validation table). `where` names the config site so the
/// exit-4 message points the user at the exact offending key.
fn parse_strategy(value: &str, where_: &str) -> Result<DispatchStrategy, String> {
    match value {
        "switch" | "switch_based" => Ok(DispatchStrategy::Switch),
        "table" | "table_driven" => Ok(DispatchStrategy::Table),
        "auto" => Ok(DispatchStrategy::Auto),
        other => Err(format!(
            "unknown strategy `{}` in {} — expected one of: switch, table, auto",
            other, where_
        )),
    }
}

/// Merge TOML + CLI into a final [`CodegenConfig`]. CLI flags ALWAYS win;
/// TOML supplies fallbacks; the codegen default supplies fallbacks for
/// keys neither source mentions (Doc 18 §6.1 last-writer-wins).
///
/// Per-machine strategy (v1.1-W7, Doc 00 §11.25): each `[machine.<Name>]
/// strategy` value populates `CodegenConfig.machine_strategy_overrides`.
/// Effective precedence resolved *per machine* is fsm.toml
/// `[machine.M].strategy` (highest) > CLI `--strategy` > default `auto`.
/// The first tier lives in the override map; the lower two tiers are
/// exactly what `cfg.strategy` already encodes (CLI flag, else
/// `[generate].strategy`, else auto), and `CodegenConfig::strategy_for`
/// falls back to `cfg.strategy` for any machine WITHOUT an override — so a
/// single resolution point expresses the whole precedence chain.
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
    cfg.strategy = parse_strategy(strategy_str, "--strategy / [generate] strategy")?;

    // Per-machine overrides (W7). Highest-priority tier of the per-machine
    // precedence chain; an invalid value is a clean exit-4 config error
    // naming the offending machine, never a silent fallback (zero-legacy).
    if let Some(c) = toml_cfg {
        for (name, sec) in &c.machine {
            if let Some(s) = sec.strategy.as_deref() {
                let parsed = parse_strategy(s, &format!("[machine.{}] strategy", name))?;
                cfg.machine_strategy_overrides.insert(name.clone(), parsed);
            }
        }
    }

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

    fn args_with_strategy(s: &str) -> GenerateArgs {
        GenerateArgs {
            target: "c99".into(),
            out: PathBuf::from("/tmp"),
            strategy: s.into(),
            queue_size: None,
            license: "MIT".into(),
            report_memory: false,
            emit_ir: false,
            import_header: vec![],
            files: vec![],
        }
    }

    #[test]
    fn per_machine_override_beats_cli_flag_for_that_machine_only() {
        // CLI says `switch` globally; fsm.toml overrides machine B to
        // `table`. B must resolve table (fsm.toml wins); a machine with no
        // entry must follow the CLI flag (switch). This is the W7
        // precedence contract: [machine.M] > --strategy > default.
        let mut t = config::FsmToml::default();
        t.machine.insert(
            "Beta".into(),
            config::MachineSection {
                strategy: Some("table".into()),
            },
        );
        let cfg = build_codegen_config(&args_with_strategy("switch"), Some(&t)).unwrap();
        assert_eq!(cfg.strategy, DispatchStrategy::Switch);
        // Override registered for Beta only.
        assert_eq!(cfg.strategy_for("Beta"), DispatchStrategy::Table);
        // Alpha has no override → falls back to the global (CLI) strategy.
        assert_eq!(cfg.strategy_for("Alpha"), DispatchStrategy::Switch);
    }

    #[test]
    fn per_machine_override_beats_generate_section_strategy() {
        // No CLI flag (auto) → global resolves from [generate].strategy
        // (table). [machine.Alpha] still overrides to switch for Alpha.
        let mut t = config::FsmToml::default();
        t.generate.strategy = Some("table".into());
        t.machine.insert(
            "Alpha".into(),
            config::MachineSection {
                strategy: Some("switch".into()),
            },
        );
        let cfg = build_codegen_config(&args_with_strategy("auto"), Some(&t)).unwrap();
        assert_eq!(cfg.strategy, DispatchStrategy::Table);
        assert_eq!(cfg.strategy_for("Alpha"), DispatchStrategy::Switch);
        assert_eq!(cfg.strategy_for("Gamma"), DispatchStrategy::Table);
    }

    #[test]
    fn invalid_per_machine_strategy_is_clean_error_naming_the_machine() {
        let mut t = config::FsmToml::default();
        t.machine.insert(
            "Motor".into(),
            config::MachineSection {
                strategy: Some("nonsense".into()),
            },
        );
        let err = build_codegen_config(&args_with_strategy("auto"), Some(&t)).unwrap_err();
        assert!(
            err.contains("nonsense") && err.contains("[machine.Motor]"),
            "error must name the offending value AND machine, got: {err}"
        );
    }

    #[test]
    fn no_machine_section_leaves_override_map_empty() {
        // Zero-behavioural-change baseline: without [machine.*] the map is
        // empty so strategy_for() == the global strategy for every name.
        let cfg = build_codegen_config(&args_with_strategy("table"), None).unwrap();
        assert!(cfg.machine_strategy_overrides.is_empty());
        assert_eq!(cfg.strategy_for("Anything"), DispatchStrategy::Table);
    }
}
