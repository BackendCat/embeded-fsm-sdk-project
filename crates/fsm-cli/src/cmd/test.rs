//! `fsm test` — conformance test runner.
//!
//! Walks a directory tree, finds every `.trace` (or `.trace.json`) file, and
//! for each one:
//!   1. resolves the companion `.fsm` source — by default the file at the
//!      same stem with `.fsm` extension, optionally overridden by the trace
//!      file's `machineFile` field;
//!   2. parses + analyzes the source (any analyzer error fails the test);
//!   3. parses the trace JSON via `fsm_simulator::parse_trace_yaml`;
//!   4. invokes `fsm_simulator::execute_trace` and asserts `matches_expected`.
//!
//! v1.0 keeps the runner intentionally simple — no MANIFEST.json yet. The
//! tree walk + naming convention is enough to drive the codegen
//! equivalence smoke tests and any user-authored traces.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fsm_analyzer::analyze_with_source;
use fsm_parser::parse;
use fsm_simulator::{execute_trace, parse_trace_yaml};

use crate::cli::TestArgs;

pub fn run(args: TestArgs) -> ExitCode {
    if !args.dir.is_dir() {
        eprintln!("error: not a directory: {}", args.dir.display());
        return ExitCode::from(3);
    }

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
    for trace_path in &traces {
        match run_single(trace_path) {
            Ok(()) => {
                if !args.failing_only {
                    println!("pass: {}", trace_path.display());
                }
                passed += 1;
            }
            Err(why) => {
                println!("fail: {}\n      {}", trace_path.display(), why);
                failed += 1;
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

/// Execute one trace file. Returns `Ok(())` on full match, `Err(reason)` on
/// any failure — read/parse/analyse/execute/mismatch.
fn run_single(trace_path: &Path) -> Result<(), String> {
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
        .any(|d| d.severity == fsm_diagnostics::Severity::Error);
    if has_err {
        return Err(format!(
            "analyzer reported {} error(s) in {}",
            result
                .diagnostics
                .iter()
                .filter(|d| d.severity == fsm_diagnostics::Severity::Error)
                .count(),
            src_path.display()
        ));
    }
    let Some(ir) = result.ir else {
        return Err("analyzer returned no IR".into());
    };
    let outcome = execute_trace(&ir, &trace).map_err(|e| format!("simulator: {}", e))?;
    if !outcome.matches_expected {
        return Err(match outcome.first_mismatch {
            Some(idx) => format!("trace mismatch at step #{}", idx),
            None => "trace mismatch (length differs)".into(),
        });
    }
    Ok(())
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
