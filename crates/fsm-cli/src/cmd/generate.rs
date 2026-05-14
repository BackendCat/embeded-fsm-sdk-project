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

    // Make sure --out exists before we start emitting.
    if let Err(e) = fs::create_dir_all(&args.out) {
        eprintln!(
            "error: cannot create output dir {}: {}",
            args.out.display(),
            e
        );
        return ExitCode::from(2);
    }

    let mut any_error = false;
    for path in &args.files {
        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {}: {}", path.display(), e);
                return ExitCode::from(3);
            }
        };
        let pr = parse(&src);
        let label = path.to_string_lossy().into_owned();
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
                let budget = compute_budget(&ir, &codegen_cfg);
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
            files: vec![],
        };
        let cfg = build_codegen_config(&args, Some(&t)).unwrap();
        assert_eq!(cfg.license_spdx, "Apache-2.0");
    }
}
