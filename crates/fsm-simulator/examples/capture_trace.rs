//! Scratch helper used by `phase1.17/trace-conformance-refresh` to capture the
//! actual `StepRecord` stream a `.trace` file produces under the current
//! simulator semantics, then print it as pretty-printed JSON suitable for
//! pasting back into the trace file's `expected:` block.
//!
//! Usage: `cargo run --example capture_trace -p fsm-simulator -- <path.fsm> <path.trace>`
//!
//! Kept in-tree (not in `target/`) so the audit step is reproducible from git.

use std::env;
use std::fs;
use std::process::ExitCode;

use fsm_analyzer::analyze_with_source;
use fsm_diagnostics::Severity;
use fsm_parser::parse;
use fsm_simulator::{execute_trace, parse_trace_yaml};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: capture_trace <path.fsm> <path.trace>");
        return ExitCode::from(2);
    }
    let fsm_path = &args[0];
    let trace_path = &args[1];

    let src = match fs::read_to_string(fsm_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("read .fsm: {}", e);
            return ExitCode::from(2);
        }
    };
    let trace_raw = match fs::read_to_string(trace_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("read .trace: {}", e);
            return ExitCode::from(2);
        }
    };
    let mut trace = match parse_trace_yaml(&trace_raw) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("parse trace: {}", e);
            return ExitCode::from(2);
        }
    };
    // Strip any pre-existing `expected` block so we capture the actual run
    // without the simulator short-circuiting on a mismatch.
    trace.expected.clear();

    let pr = parse(&src);
    let result = analyze_with_source(&pr, fsm_path, &src);
    let err_count = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    if err_count > 0 {
        eprintln!("analyzer reported {} error(s) in {}", err_count, fsm_path);
        return ExitCode::from(2);
    }
    let Some(ir) = result.ir else {
        eprintln!("analyzer returned no IR");
        return ExitCode::from(2);
    };

    let outcome = match execute_trace(&ir, &trace) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("simulator: {}", e);
            return ExitCode::from(2);
        }
    };

    match serde_json::to_string_pretty(&outcome.actual) {
        Ok(s) => println!("{}", s),
        Err(e) => {
            eprintln!("serialize records: {}", e);
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
