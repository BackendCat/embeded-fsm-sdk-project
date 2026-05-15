//! `fsm test` — conformance test runner.
//!
//! Two modes depending on the layout under the test directory:
//!
//! 1. **MANIFEST.json mode.** If `MANIFEST.json` exists at the dir root, the
//!    runner parses it (per Doc 15 §4 plus the v1.0-pragmatic shape used by
//!    `tests/conformance/`) and walks every fixture by category:
//!    - `parser` / `validator` / `semantic` positive: parse + analyze must
//!      report zero errors.
//!    - `parser` / `validator` / `semantic` negative: parse + analyze MUST
//!      emit each diagnostic code listed in `expectedCodes`.
//!    - `codegen-c`: `fsm generate --target c99` must succeed, and every
//!      file in the fixture's `expected/` directory must be a substring of
//!      the matching output (snippet check).
//!    - `formatter` idempotent: `fsm fmt --check` exits 0.
//!    - `formatter` transform: input + expected — read both, format the
//!      input, byte-compare against expected.
//!
//! 2. **Trace mode (legacy / examples).** If no MANIFEST.json is found,
//!    the runner walks the dir tree, finds every `.trace` / `.trace.json`
//!    file, and executes it through the in-process simulator.
//!
//! Doc 15 §6 specifies `.fsm.test` files as the negative-test schema; this
//! v1.0 runner uses per-directory `source.fsm` + `expected.json` instead —
//! a flatter shape that keeps the .fsm extension consistent with everything
//! else in the SDK and avoids a separate "test parser." The MANIFEST.json
//! is authoritative; on-disk structure is incidental.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fsm_analyzer::analyze_with_source;
use fsm_codegen_c::{emit, CodegenConfig};
use fsm_diagnostics::{Diagnostic, DiagnosticCode, Severity};
use fsm_formatter::{format_string, FormatOptions};
use fsm_parser::parse;
use fsm_simulator::{execute_trace, parse_trace_yaml};
use serde::Deserialize;

use crate::cli::TestArgs;

pub(crate) fn run(args: TestArgs) -> ExitCode {
    if !args.dir.is_dir() {
        eprintln!("error: not a directory: {}", args.dir.display());
        return ExitCode::from(3);
    }

    let manifest_path = args.dir.join("MANIFEST.json");
    if manifest_path.is_file() {
        run_manifest(&args.dir, &manifest_path, args.failing_only)
    } else {
        run_traces(&args)
    }
}

// ---------------------------------------------------------------------------
// Trace mode (legacy / examples)
// ---------------------------------------------------------------------------

fn run_traces(args: &TestArgs) -> ExitCode {
    let traces = match collect_traces(&args.dir) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: walking {}: {}", args.dir.display(), e);
            return ExitCode::from(2);
        }
    };

    if traces.is_empty() {
        eprintln!(
            "warning: no .trace or .trace.json files under {}",
            args.dir.display()
        );
        return ExitCode::SUCCESS;
    }

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    for trace_path in &traces {
        match run_single_trace(trace_path) {
            Ok(TraceOutcome::Verified) => {
                if !args.failing_only {
                    println!("pass: {}", trace_path.display());
                }
                passed += 1;
            }
            Ok(TraceOutcome::EmptyExpected) => {
                // A trace that ships without an `expected` block can't verify
                // anything — under the new behavior it's a hard fail by
                // default. `--allow-empty-expected` opts back into the legacy
                // capture-mode pass so trace authors can iterate.
                if args.allow_empty_expected {
                    if !args.failing_only {
                        println!(
                            "skip: {} (no expected block; --allow-empty-expected)",
                            trace_path.display()
                        );
                    }
                    skipped += 1;
                } else {
                    println!(
                        "fail: {}\n      \
                         trace has no `expected` block; nothing to verify. \
                         Re-run with `--allow-empty-expected` only if you are \
                         iterating on step sequences and have not yet recorded \
                         the expected output.",
                        trace_path.display()
                    );
                    failed += 1;
                }
            }
            Err(why) => {
                println!("fail: {}\n      {}", trace_path.display(), why);
                failed += 1;
            }
        }
    }

    let total = passed + failed + skipped;
    if skipped > 0 {
        println!(
            "\nfsm test: {} passed, {} failed, {} skipped (of {} total)",
            passed, failed, skipped, total
        );
    } else {
        println!(
            "\nfsm test: {} passed, {} failed (of {} total)",
            passed, failed, total
        );
    }

    if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Walk `root` and collect every `*.trace` / `*.trace.json` regular file.
fn collect_traces(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let name = match p.file_name().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };
            // Accept either ".trace" or ".trace.json" — both are JSON
            // documents matching the simulator's `TraceFile` shape per
            // `fsm_simulator::trace` module docs.
            if name.ends_with(".trace") || name.ends_with(".trace.json") {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Outcome of running one trace through the in-process simulator. `Verified`
/// means the actual records matched the trace's `expected` list (or no
/// `expected` was provided in opt-in mode — see [`TestArgs::allow_empty_expected`]).
/// `EmptyExpected` is surfaced separately so the caller can either treat the
/// trace as a skipped-by-design (legacy) or a hard fail (current default).
enum TraceOutcome {
    Verified,
    EmptyExpected,
}

/// Execute one trace file. Returns `Ok(Verified)` on full match,
/// `Ok(EmptyExpected)` when the trace lacks an `expected` block (caller
/// decides whether that's a fail or a skip), or `Err(reason)` on any other
/// failure — read/parse/analyse/execute/mismatch.
fn run_single_trace(trace_path: &Path) -> Result<TraceOutcome, String> {
    let trace_raw =
        std::fs::read_to_string(trace_path).map_err(|e| format!("read trace: {}", e))?;
    let trace = parse_trace_yaml(&trace_raw).map_err(|e| format!("parse trace: {}", e))?;

    // Resolve the source .fsm file. The trace may name it explicitly via
    // `machineFile`; otherwise we strip the trace extension and append
    // `.fsm`.
    let src_path = if let Some(mf) = &trace.machine_file {
        let candidate = trace_path
            .parent()
            .map(|p| p.join(mf))
            .unwrap_or_else(|| PathBuf::from(mf));
        candidate
    } else {
        infer_src_path(trace_path)
    };

    let src = std::fs::read_to_string(&src_path)
        .map_err(|e| format!("read .fsm at {}: {}", src_path.display(), e))?;
    let pr = parse(&src);
    let label = src_path.to_string_lossy().into_owned();
    let result = analyze_with_source(&pr, &label, &src);
    let has_err = result
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error);
    if has_err {
        return Err(format!(
            "analyzer reported {} error(s) in {}",
            result
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count(),
            src_path.display()
        ));
    }
    let Some(ir) = result.ir else {
        return Err("analyzer returned no IR".into());
    };
    let outcome = execute_trace(&ir, &trace).map_err(|e| format!("simulator: {}", e))?;
    if !outcome.matches_expected {
        return Err(format_trace_mismatch(&outcome, &trace.expected));
    }
    // An empty `expected` list means the caller wants to know but should not
    // be treated as a real verification — surface that distinction.
    if trace.expected.is_empty() {
        return Ok(TraceOutcome::EmptyExpected);
    }
    Ok(TraceOutcome::Verified)
}

/// Render a human-readable diff between `actual` and `expected` records for
/// the failure path. Includes the index of the first mismatching record plus
/// a single-record summary on each side so the diff is actionable without
/// dumping the whole trace.
fn format_trace_mismatch(
    outcome: &fsm_simulator::TraceResult,
    expected: &[fsm_simulator::StepRecord],
) -> String {
    let idx = match outcome.first_mismatch {
        Some(i) => i,
        None => {
            return format!(
                "trace mismatch (length differs): actual={}, expected={}",
                outcome.actual.len(),
                expected.len()
            );
        }
    };
    let actual_snippet = outcome
        .actual
        .get(idx)
        .map(format_record_summary)
        .unwrap_or_else(|| "(no actual record at this index)".into());
    let expected_snippet = expected
        .get(idx)
        .map(format_record_summary)
        .unwrap_or_else(|| "(no expected record at this index)".into());
    format!(
        "trace mismatch at step #{}\n        expected: {}\n        actual:   {}",
        idx, expected_snippet, actual_snippet,
    )
}

fn format_record_summary(rec: &fsm_simulator::StepRecord) -> String {
    let kind: &str = match rec.kind {
        fsm_simulator::StepKind::Init => "init",
        fsm_simulator::StepKind::Dispatched => "dispatched",
        fsm_simulator::StepKind::Raised => "raised",
        fsm_simulator::StepKind::TimerFired => "timer_fired",
        fsm_simulator::StepKind::Completion => "completion",
        fsm_simulator::StepKind::Discarded => "discarded",
        fsm_simulator::StepKind::EventDeferred => "event_deferred",
        fsm_simulator::StepKind::EventRedispatched => "event_redispatched",
        fsm_simulator::StepKind::SubmachineEntered => "submachine_entered",
        fsm_simulator::StepKind::SubmachineEventDelegated => "submachine_event_delegated",
        fsm_simulator::StepKind::SubmachineCompleted => "submachine_completed",
    };
    let evt = rec
        .event_received
        .as_ref()
        .map(|e| e.name.as_str())
        .unwrap_or("-");
    format!(
        "kind={} event={} config_after={:?}",
        kind, evt, rec.config_after
    )
}

fn infer_src_path(trace_path: &Path) -> PathBuf {
    let stem = trace_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    // For `foo.trace.json`, file_stem gives `foo.trace` — strip the inner
    // `.trace` segment as well.
    let stem = stem.trim_end_matches(".trace");
    trace_path.with_file_name(format!("{}.fsm", stem))
}

// ---------------------------------------------------------------------------
// MANIFEST.json mode
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    #[allow(dead_code)]
    version: String,
    categories: Vec<ManifestCategory>,
}

#[derive(Deserialize)]
struct ManifestCategory {
    name: String,
    fixtures: Vec<ManifestFixture>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFixture {
    id: String,
    /// `positive` | `negative` | `golden` | `idempotent` | `transform`.
    kind: String,
    path: String,
    #[serde(default)]
    expected_codes: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    description: String,
    #[allow(dead_code)]
    #[serde(default)]
    normative: bool,
    #[allow(dead_code)]
    #[serde(default)]
    tags: Vec<String>,
}

/// Per-directory negative-fixture expectations.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedDiag {
    code: String,
    #[serde(default)]
    message_contains: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedJson {
    #[allow(dead_code)]
    #[serde(default)]
    exit_code: Option<i32>,
    #[serde(default)]
    expected_diagnostics: Vec<ExpectedDiag>,
}

fn run_manifest(root: &Path, manifest_path: &Path, failing_only: bool) -> ExitCode {
    let raw = match std::fs::read_to_string(manifest_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: read {}: {}", manifest_path.display(), e);
            return ExitCode::from(2);
        }
    };
    let manifest: Manifest = match serde_json::from_str(&raw) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: parse {}: {}", manifest_path.display(), e);
            return ExitCode::from(2);
        }
    };

    let mut passed = 0usize;
    let mut failed = 0usize;
    for cat in &manifest.categories {
        for fixture in &cat.fixtures {
            let outcome = run_fixture(root, &cat.name, fixture);
            match outcome {
                Ok(()) => {
                    if !failing_only {
                        println!("pass: {} ({} / {})", fixture.id, cat.name, fixture.path);
                    }
                    passed += 1;
                }
                Err(why) => {
                    println!(
                        "fail: {} ({} / {})\n      {}",
                        fixture.id, cat.name, fixture.path, why
                    );
                    failed += 1;
                }
            }
        }
    }
    println!(
        "\nfsm test: {} passed, {} failed (of {} total)",
        passed,
        failed,
        passed + failed
    );
    if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn run_fixture(root: &Path, category: &str, fx: &ManifestFixture) -> Result<(), String> {
    let path = root.join(&fx.path);
    match (category, fx.kind.as_str()) {
        ("parser" | "validator" | "semantic", "positive") => run_pos(&path),
        ("parser" | "validator" | "semantic", "negative") => run_neg(&path, &fx.expected_codes),
        ("codegen-c", "golden") => run_codegen_golden(&path),
        ("formatter", "idempotent") => run_fmt_idempotent(&path),
        ("formatter", "transform") => run_fmt_transform(&path),
        _ => Err(format!("unknown category/kind: {}/{}", category, fx.kind)),
    }
}

/// Positive fixture: parse + analyze, MUST emit zero errors.
fn run_pos(src_path: &Path) -> Result<(), String> {
    let src = std::fs::read_to_string(src_path)
        .map_err(|e| format!("read {}: {}", src_path.display(), e))?;
    let pr = parse(&src);
    let label = src_path.to_string_lossy().into_owned();
    let result = analyze_with_source(&pr, &label, &src);
    let errors: Vec<&Diagnostic> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(format!(
            "expected clean parse + analyze, got {} error(s): {:?}",
            errors.len(),
            errors
                .iter()
                .map(|e| e.code.to_string())
                .collect::<Vec<_>>()
        ));
    }
    Ok(())
}

/// Negative fixture: a directory containing `source.fsm` (+ optional
/// `expected.json`). The MANIFEST's `expectedCodes` is authoritative; the
/// per-fixture `expected.json` is a richer companion that may also be read
/// when present (we surface its codes if the manifest list was empty).
fn run_neg(dir_path: &Path, expected_codes_manifest: &[String]) -> Result<(), String> {
    let src_path = dir_path.join("source.fsm");
    let src = std::fs::read_to_string(&src_path)
        .map_err(|e| format!("read {}: {}", src_path.display(), e))?;
    let pr = parse(&src);
    let label = src_path.to_string_lossy().into_owned();
    let result = analyze_with_source(&pr, &label, &src);

    let expected_codes: Vec<String> = if !expected_codes_manifest.is_empty() {
        expected_codes_manifest.to_vec()
    } else {
        // Fall back to expected.json if it exists.
        let exp_path = dir_path.join("expected.json");
        if exp_path.is_file() {
            let raw = std::fs::read_to_string(&exp_path)
                .map_err(|e| format!("read {}: {}", exp_path.display(), e))?;
            let parsed: ExpectedJson = serde_json::from_str(&raw)
                .map_err(|e| format!("parse {}: {}", exp_path.display(), e))?;
            parsed
                .expected_diagnostics
                .iter()
                .map(|d| d.code.clone())
                .collect()
        } else {
            Vec::new()
        }
    };

    if expected_codes.is_empty() {
        return Err("negative fixture has no expected codes".into());
    }

    // Combine parse errors + analyze diagnostics; both carry DiagnosticCode.
    let mut emitted_codes: Vec<DiagnosticCode> = pr.errors.iter().map(|d| d.code).collect();
    emitted_codes.extend(result.diagnostics.iter().map(|d| d.code));

    let missing: Vec<&String> = expected_codes
        .iter()
        .filter(|exp| !emitted_codes.iter().any(|c| c.to_string() == **exp))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "missing expected diagnostic code(s): {:?}; got: {:?}",
            missing,
            emitted_codes
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
        ));
    }

    // Optional message-substring check from expected.json.
    let exp_path = dir_path.join("expected.json");
    if exp_path.is_file() {
        let raw = std::fs::read_to_string(&exp_path).unwrap_or_default();
        if let Ok(parsed) = serde_json::from_str::<ExpectedJson>(&raw) {
            for ed in &parsed.expected_diagnostics {
                if let Some(needle) = &ed.message_contains {
                    let needle_lc = needle.to_ascii_lowercase();
                    let matched = result
                        .diagnostics
                        .iter()
                        .chain(pr.errors.iter())
                        .filter(|d| d.code.to_string() == ed.code)
                        .any(|d| d.message.to_ascii_lowercase().contains(&needle_lc));
                    if !matched {
                        return Err(format!(
                            "no diagnostic for {} contained substring '{}'",
                            ed.code, needle
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

/// Codegen golden: `source.fsm` + `expected/` directory. The runner
/// generates output to a tempdir, then asserts each `expected/<name>.contains`
/// file's lines are substrings of the corresponding output file.
fn run_codegen_golden(dir_path: &Path) -> Result<(), String> {
    let src_path = dir_path.join("source.fsm");
    let src = std::fs::read_to_string(&src_path)
        .map_err(|e| format!("read {}: {}", src_path.display(), e))?;
    let pr = parse(&src);
    let label = src_path.to_string_lossy().into_owned();
    let result = analyze_with_source(&pr, &label, &src);
    let errors: Vec<&Diagnostic> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(format!(
            "analyzer rejected source.fsm with {} error(s)",
            errors.len()
        ));
    }
    let ir = result.ir.ok_or_else(|| String::from("no IR produced"))?;
    let cfg = CodegenConfig::default();
    let emitted = emit(&ir, &cfg).map_err(|e| format!("codegen-c emit: {:?}", e))?;
    // Build a name -> content map.
    let by_name: std::collections::HashMap<String, String> = emitted
        .files
        .iter()
        .map(|f| (f.path.clone(), f.content.clone()))
        .collect();

    // For each `expected/NAME.contains` (or `expected/NAME`) verify each
    // non-empty line is a substring of the generated file `NAME`.
    let expected_dir = dir_path.join("expected");
    if !expected_dir.is_dir() {
        return Err(format!(
            "missing expected/ dir at {}",
            expected_dir.display()
        ));
    }
    for entry in std::fs::read_dir(&expected_dir).map_err(|e| format!("read expected/: {}", e))? {
        let entry = entry.map_err(|e| format!("expected/ entry: {}", e))?;
        let path = entry.path();
        let fname = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        // Strip the `.contains` suffix to find the source name.
        let stem = fname
            .strip_suffix(".contains")
            .unwrap_or(&fname)
            .to_string();
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("read {}: {}", path.display(), e))?;
        let content = by_name
            .get(&stem)
            .ok_or_else(|| format!("codegen did not emit a file named {}", stem))?;
        for needle in raw.lines() {
            let n = needle.trim();
            if n.is_empty() {
                continue;
            }
            if !content.contains(n) {
                return Err(format!(
                    "expected/{} requires snippet '{}' in generated {} (not found)",
                    fname, n, stem
                ));
            }
        }
    }
    Ok(())
}

fn run_fmt_idempotent(src_path: &Path) -> Result<(), String> {
    let src = std::fs::read_to_string(src_path)
        .map_err(|e| format!("read {}: {}", src_path.display(), e))?;
    let formatted = format_string(&src, &FormatOptions::default())
        .map_err(|e| format!("format error: {:?}", e))?;
    if formatted != src {
        return Err(format!(
            "fixture is not canonical — `fmt --check` would fail at {}",
            src_path.display()
        ));
    }
    // Idempotency: format(format(s)) == format(s)
    let formatted_twice = format_string(&formatted, &FormatOptions::default())
        .map_err(|e| format!("format error (2nd pass): {:?}", e))?;
    if formatted_twice != formatted {
        return Err("formatter is not idempotent on this fixture".into());
    }
    Ok(())
}

fn run_fmt_transform(dir_path: &Path) -> Result<(), String> {
    let input_path = dir_path.join("input.fsm");
    let expected_path = dir_path.join("expected.fsm");
    let input = std::fs::read_to_string(&input_path)
        .map_err(|e| format!("read {}: {}", input_path.display(), e))?;
    let expected = std::fs::read_to_string(&expected_path)
        .map_err(|e| format!("read {}: {}", expected_path.display(), e))?;
    let formatted = format_string(&input, &FormatOptions::default())
        .map_err(|e| format!("format error: {:?}", e))?;
    if formatted.trim_end() != expected.trim_end() {
        return Err(format!(
            "transform mismatch — formatted input does not equal expected ({} vs {})",
            formatted.len(),
            expected.len()
        ));
    }
    Ok(())
}
