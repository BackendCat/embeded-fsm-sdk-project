//! `fsm baseline` — trace differential replay (v1.4-W3): the regression
//! oracle that makes the verification pipeline factory-trustable.
//!
//! ## What this is
//!
//! `fsm baseline --record <suite>` captures the execution trace of every
//! `.fsm` under `<suite>` — driven through the **shipped interpreter** —
//! into a frozen `fsm-trace/v1` baseline corpus. `fsm baseline --check
//! <suite>` re-runs every FSM on a later build and reports any **semantic
//! drift** (a divergence from the frozen baseline) with a precise
//! first-mismatch report. A refactor that silently changes runtime
//! semantics is therefore caught.
//!
//! ## Keystone discipline (the project's load-bearing invariant)
//!
//! This module **consumes** `fsm_simulator::execute_trace` (trace.rs) — the
//! same seam the conformance suite (`cmd::test`) and the verifier
//! (`fsm verify`) consume. It does **not** re-implement step comparison,
//! re-derive `StepRecord`s, or fork any execution/comparison semantics. The
//! interpreter remains the sole semantic oracle: capture is
//! `execute_trace(ir, trace).actual`; compare is `execute_trace(ir, trace)`
//! reading `first_mismatch` against the corpus's frozen `expected` list. A
//! regression tool that forked semantics would be worthless.
//!
//! ## CLI-surface choice (a disclosed judgment call)
//!
//! Doc 30 §4.2-W3 sketches `fsm test --baseline <dir>`. This ships instead
//! as a **dedicated sibling subcommand `fsm baseline`** (Doc 30 §4.x has
//! known closeout-batched staleness; the W3 brief explicitly delegates the
//! surface choice). Rationale: `fsm test` is already a two-mode runner
//! (MANIFEST vs trace-walk) with its **own** exit-code semantics
//! (1 = fixture-fail, 2 = walk-error, 3 = not-a-dir). W3 needs a *distinct*
//! exit-code contract that mirrors `fsm verify`'s 0/1/2/3/4 family with
//! **different** verdict meanings (1 = drift, 2 = inconclusive). Layering a
//! second exit-code contract onto `fsm test` would collide two contracts on
//! one subcommand and muddy its dispatch. A single-purpose subcommand keeps
//! the factory contract clean and a coherent sibling of `fsm verify`.
//!
//! ## Exit-code contract (DEFINED + DOCUMENTED here — load-bearing)
//!
//! Mirrors `fsm verify`'s family (see `cmd::verify` module docs); the
//! verdict meanings are W3-specific:
//!
//! | Exit | Meaning |
//! |------|---------|
//! | **0** | **no drift** — every FSM's replay matched its frozen baseline (or `--record` wrote the corpus successfully) |
//! | **1** | **drift detected** — at least one FSM diverged from its baseline; the first-mismatch is reported (human + `--json`) |
//! | **2** | **inconclusive** — the baseline corpus is absent / unreadable / not `fsm-trace/v1` when comparing, so drift could be neither confirmed nor denied. **Explicitly NOT "no drift".** |
//! | **3** | IO error (suite dir missing, corpus dir unwritable on `--record`, a corpus file unreadable mid-walk) |
//! | **4** | out-of-scope — a suite `.fsm` does not parse/analyze, or its driver/trace is malformed (cannot capture/compare what won't compile; kept distinct from a verdict so a factory script never confuses "bad input" with "semantics drifted") |
//!
//! The exit-code contract is itself part of the versioned surface and is
//! held stable across additive `fsm-trace/v1` / `fsm-trace-diff/v1`
//! revisions.
//!
//! ## `fsm-trace/v1` — the baseline-corpus file contract (factory artifact)
//!
//! One JSON file per captured FSM at `<corpus>/<sanitized-rel-path>.json`:
//!
//! ```jsonc
//! {
//!   "schema": "fsm-trace/v1",     // stability marker — see versioning policy
//!   "fsm": "motor/motor.fsm",     // suite-relative source path (provenance)
//!   "machine": "Motor",           // the machine the trace drove
//!   "trace": { /* fsm_simulator::TraceFile: init + steps + the captured
//!                 `expected` StepRecord list (the frozen oracle) */ }
//! }
//! ```
//!
//! The corpus is a *persisted artifact that must survive format evolution*,
//! so the marker is mandatory and checked on read. The captured
//! `StepRecord`s are byte-stable by construction (Doc 13 §11: every
//! payload/context map is a `BTreeMap`, every list a `Vec` in
//! interpreter-emission order, no `HashMap`), so capturing the same FSM
//! twice on the same build yields byte-identical corpus files (acceptance
//! (c)). The serializer is `serde_json::to_string_pretty`, deterministic
//! for this shape.
//!
//! ### Schema-versioning policy — `fsm-trace/vN` (mirrors `fsm-verify/vN`)
//!
//! Intentionally the **same policy shape** as `cmd::verify`'s
//! `fsm-verify/vN` so a factory integrator sees one coherent CLI contract
//! family:
//!
//! - **Additive changes keep the SAME major** (`fsm-trace/v1`). *Additive*
//!   = a NEW optional key on the corpus envelope, or a NEW
//!   `skip_serializing_if`-omitted field on `StepRecord` (the established
//!   append-only `StepRecord` discipline — e.g. the W2 `submachine` field,
//!   omitted when `None`, kept legacy traces byte-identical). A reader that
//!   only consumes known keys is unaffected.
//! - **A breaking change bumps the major** (`fsm-trace/v2`): the meaning /
//!   type / shape of an EXISTING key changes, a key is removed/renamed, or
//!   the `TraceFile`/`StepRecord` wire shape changes incompatibly. A
//!   `fsm-trace/v1` corpus is then re-recorded against the new build; a
//!   `--check` against a major-mismatched corpus is **inconclusive (exit
//!   2)**, never a false "no drift" and never a false "drift".
//! - The exit-code contract (0/1/2/3/4) is part of the versioned surface
//!   and is held stable across additive revisions.
//!
//! A consumer SHOULD branch on the major (`schema` prefix `fsm-trace/v1`)
//! and tolerate unknown keys rather than pinning a byte shape. This policy
//! is intentionally co-located with the contract it governs so a factory
//! integrator needs no other doc (the `fsm-verify/vN` precedent).
//!
//! ## `--json` — the `fsm-trace-diff/v1` machine-readable result
//!
//! A single deterministic JSON object on stdout (sorted keys via
//! `BTreeMap`; arrays in stable order). On drift it carries the
//! machine-consumable first-mismatch (which FSM, the step index, the
//! expected-vs-actual `StepRecord`):
//!
//! ```jsonc
//! {
//!   "schema": "fsm-trace-diff/v1",
//!   "mode": "check" | "record",
//!   "verdict": "no-drift" | "drift" | "inconclusive",
//!   "exitCode": 0,
//!   "corpus": "/abs/path/to/baselines",
//!   "fsms": [
//!     { "fsm": "motor/motor.fsm", "machine": "Motor",
//!       "result": "match" | "drift" | "inconclusive" | "recorded",
//!       // present iff result == "drift":
//!       "firstMismatch": {
//!         "step": 4,
//!         "expected": { /* StepRecord */ } | null,
//!         "actual":   { /* StepRecord */ } | null
//!       },
//!       // present iff result == "inconclusive":
//!       "reason": "baseline corpus file absent for this FSM"
//!     }
//!   ]
//! }
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fsm_analyzer::analyze_with_source;
use fsm_diagnostics::Severity;
use fsm_parser::parse;
use fsm_simulator::{execute_trace, StepRecord, TraceFile, TraceResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::cli::BaselineArgs;

/// The corpus-file schema marker. A breaking shape change bumps this.
const CORPUS_SCHEMA: &str = "fsm-trace/v1";
/// The `--json` result schema marker.
const DIFF_SCHEMA: &str = "fsm-trace-diff/v1";

/// On-disk `fsm-trace/v1` corpus envelope (one file per captured FSM).
/// `#[serde(deny_unknown_fields)]` is intentionally NOT set: per the
/// versioning policy an additive future key must not break a v1 reader.
#[derive(Debug, Serialize, Deserialize)]
struct CorpusFile {
    /// Mandatory stability marker; checked on read (`fsm-trace/vN`).
    schema: String,
    /// Suite-relative source path — provenance, surfaced in reports.
    fsm: String,
    /// The machine the captured trace drove.
    machine: String,
    /// The frozen oracle: a `TraceFile` whose `expected` list is the
    /// captured `StepRecord`s. Compare = `execute_trace(ir, &this)`.
    trace: TraceFile,
}

/// Per-FSM outcome of a `--check` / `--record` walk.
enum FsmOutcome {
    /// `--check`: replay matched the frozen baseline exactly.
    Match,
    /// `--check`: replay diverged. Carries the first-mismatch records.
    Drift {
        step: usize,
        expected: Option<StepRecord>,
        actual: Option<StepRecord>,
    },
    /// `--check`: drift could be neither confirmed nor denied (no/!v1
    /// corpus file for this FSM). Maps to exit 2.
    Inconclusive { reason: String },
    /// `--record`: a fresh baseline was written for this FSM.
    Recorded,
}

struct FsmReport {
    fsm_rel: String,
    machine: String,
    outcome: FsmOutcome,
}

pub(crate) fn run(args: BaselineArgs) -> ExitCode {
    if !args.suite.is_dir() {
        eprintln!("error: not a directory: {}", args.suite.display());
        return ExitCode::from(3);
    }
    let corpus_dir = args
        .corpus
        .clone()
        .unwrap_or_else(|| args.suite.join("baselines"));

    let fsms = match collect_fsms(&args.suite) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: walking {}: {}", args.suite.display(), e);
            return ExitCode::from(3);
        }
    };
    if fsms.is_empty() {
        eprintln!(
            "error: no .fsm files under {} — nothing to baseline",
            args.suite.display()
        );
        return ExitCode::from(3);
    }

    let recording = args.record;
    if recording {
        if let Err(e) = std::fs::create_dir_all(&corpus_dir) {
            eprintln!(
                "error: cannot create corpus dir {}: {}",
                corpus_dir.display(),
                e
            );
            return ExitCode::from(3);
        }
    }

    let mut reports: Vec<FsmReport> = Vec::with_capacity(fsms.len());
    for fsm_path in &fsms {
        let rel = fsm_path
            .strip_prefix(&args.suite)
            .unwrap_or(fsm_path)
            .to_string_lossy()
            .replace('\\', "/");

        let res = if recording {
            record_one(fsm_path, &rel, &corpus_dir)
        } else {
            check_one(fsm_path, &rel, &corpus_dir)
        };
        match res {
            Ok(report) => reports.push(report),
            // A hard error (out-of-scope / IO) aborts the whole walk with
            // the matching exit code — a partial corpus / partial compare
            // is never silently "no drift" (the honest-bound discipline).
            Err(code) => return code,
        }
    }

    emit(&args, &corpus_dir, &reports)
}

/// Walk `root` and collect every `*.fsm` regular file (sorted, so the walk
/// — and any `--json` `fsms` array — is deterministic). Skips the corpus
/// dir itself if it lives under the suite.
fn collect_fsms(root: &Path) -> std::io::Result<Vec<PathBuf>> {
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
            if p.extension().and_then(|s| s.to_str()) == Some("fsm") {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Parse + analyze a suite `.fsm`. `Err(ExitCode::from(4))` (out-of-scope)
/// if it does not compile — capturing/comparing an un-analyzable model is
/// meaningless and kept distinct from a drift verdict.
fn analyze_fsm(fsm_path: &Path) -> Result<fsm_ir::Ir, ExitCode> {
    let src = match crate::safe_io::read_to_string_capped(fsm_path, 16 * 1024 * 1024) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", fsm_path.display(), e);
            return Err(ExitCode::from(3));
        }
    };
    let label = fsm_path.to_string_lossy().into_owned();
    let pr = parse(&src);
    let res = analyze_with_source(&pr, &label, &src);
    if res
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        eprintln!(
            "error: {} has analysis errors; cannot baseline a model that does not \
             compile (run `fsm check {}` to see them)",
            fsm_path.display(),
            fsm_path.display()
        );
        return Err(ExitCode::from(4));
    }
    res.ir.ok_or_else(|| {
        eprintln!(
            "error: analyzer produced no IR for {} (cannot baseline)",
            fsm_path.display()
        );
        ExitCode::from(4)
    })
}

/// Load the optional driver beside an `.fsm`: `<stem>.steps.json`, a
/// `TraceFile` whose `expected` is ignored on capture (it carries `init` +
/// `steps`). Absent ⇒ a default `TraceFile` (init-only capture — still a
/// real, drift-sensitive semantic snapshot of the entry path).
fn load_driver(fsm_path: &Path) -> Result<TraceFile, ExitCode> {
    let stem = fsm_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let driver_path = fsm_path.with_file_name(format!("{stem}.steps.json"));
    if !driver_path.is_file() {
        return Ok(TraceFile::default());
    }
    let raw = match std::fs::read_to_string(&driver_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: read driver {}: {}", driver_path.display(), e);
            return Err(ExitCode::from(3));
        }
    };
    match fsm_simulator::parse_trace_yaml(&raw) {
        Ok(t) => Ok(t),
        Err(e) => {
            // A malformed driver is a request error, not a drift verdict.
            eprintln!(
                "error: driver {} is malformed: {}",
                driver_path.display(),
                e
            );
            Err(ExitCode::from(4))
        }
    }
}

/// Stable corpus filename for a suite-relative FSM path:
/// `a/b/motor.fsm` → `a__b__motor.fsm.json`. The double-underscore join
/// keeps it a flat, collision-free, filesystem-safe single file.
fn corpus_file_name(rel: &str) -> String {
    format!("{}.json", rel.replace('/', "__"))
}

/// `--record`: drive the FSM through the shipped interpreter via the
/// driver, freeze the produced `StepRecord`s as the corpus `expected`.
fn record_one(fsm_path: &Path, rel: &str, corpus_dir: &Path) -> Result<FsmReport, ExitCode> {
    let ir = analyze_fsm(fsm_path)?;
    let mut driver = load_driver(fsm_path)?;
    let machine = driver
        .init
        .machine_name
        .clone()
        .or_else(|| ir.machines.first().map(|m| m.name.clone()))
        .unwrap_or_default();

    // CONSUME the keystone seam: the interpreter produces the records.
    let result = match execute_trace(&ir, &driver) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: interpreter could not run: {}", rel, e);
            return Err(ExitCode::from(4));
        }
    };
    // Freeze the captured records as the trace's `expected` oracle. Pin the
    // resolved machine name so a later `--check` is independent of IR
    // machine ordering.
    driver.init.machine_name = Some(machine.clone());
    driver.expected = result.actual;

    let corpus = CorpusFile {
        schema: CORPUS_SCHEMA.to_string(),
        fsm: rel.to_string(),
        machine: machine.clone(),
        trace: driver,
    };
    // Deterministic by construction (BTreeMap/Vec wire discipline,
    // Doc 13 §11) — capturing twice on one build is byte-identical.
    let body = match serde_json::to_string_pretty(&corpus) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: serialize corpus for {}: {}", rel, e);
            return Err(ExitCode::from(3));
        }
    };
    let out_path = corpus_dir.join(corpus_file_name(rel));
    if let Err(e) = std::fs::write(&out_path, body) {
        eprintln!("error: write corpus file {}: {}", out_path.display(), e);
        return Err(ExitCode::from(3));
    }
    Ok(FsmReport {
        fsm_rel: rel.to_string(),
        machine,
        outcome: FsmOutcome::Recorded,
    })
}

/// `--check`: load the frozen corpus file and re-run the FSM through the
/// shipped interpreter. Drift = `execute_trace`'s `first_mismatch`.
fn check_one(fsm_path: &Path, rel: &str, corpus_dir: &Path) -> Result<FsmReport, ExitCode> {
    let corpus_path = corpus_dir.join(corpus_file_name(rel));
    if !corpus_path.is_file() {
        // No baseline for this FSM ⇒ INCONCLUSIVE (exit 2): we can neither
        // confirm nor deny drift. NOT a false "no drift".
        return Ok(FsmReport {
            fsm_rel: rel.to_string(),
            machine: String::new(),
            outcome: FsmOutcome::Inconclusive {
                reason: format!(
                    "no baseline corpus file at {} — run `fsm baseline --record` first",
                    corpus_path.display()
                ),
            },
        });
    }
    let raw = match std::fs::read_to_string(&corpus_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: read corpus file {}: {}", corpus_path.display(), e);
            return Err(ExitCode::from(3));
        }
    };
    let corpus: CorpusFile = match serde_json::from_str(&raw) {
        Ok(c) => c,
        Err(_) => {
            // An unparseable / wrong-shape corpus file is inconclusive, not
            // "no drift" and not "drift".
            return Ok(FsmReport {
                fsm_rel: rel.to_string(),
                machine: String::new(),
                outcome: FsmOutcome::Inconclusive {
                    reason: format!(
                        "corpus file {} is not a readable {} envelope",
                        corpus_path.display(),
                        CORPUS_SCHEMA
                    ),
                },
            });
        }
    };
    // Major-version gate: a non-`fsm-trace/v1` corpus cannot be compared
    // against this build ⇒ inconclusive (the versioning-policy contract).
    if corpus_major(&corpus.schema) != corpus_major(CORPUS_SCHEMA) {
        return Ok(FsmReport {
            fsm_rel: rel.to_string(),
            machine: corpus.machine.clone(),
            outcome: FsmOutcome::Inconclusive {
                reason: format!(
                    "corpus schema {:?} is not compatible with {} (re-record the baseline)",
                    corpus.schema, CORPUS_SCHEMA
                ),
            },
        });
    }

    let ir = analyze_fsm(fsm_path)?;
    // CONSUME the keystone seam: the interpreter replays + the comparator
    // (`first_mismatch`) is the interpreter crate's, not re-implemented.
    let result: TraceResult = match execute_trace(&ir, &corpus.trace) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}: interpreter could not run: {}", rel, e);
            return Err(ExitCode::from(4));
        }
    };

    if result.matches_expected {
        return Ok(FsmReport {
            fsm_rel: rel.to_string(),
            machine: corpus.machine,
            outcome: FsmOutcome::Match,
        });
    }
    // Drift. `first_mismatch` is the interpreter's index; on a pure
    // length divergence with a common prefix it is the shorter length.
    let step = result.first_mismatch.unwrap_or(0);
    let expected = corpus.trace.expected.get(step).cloned();
    let actual = result.actual.get(step).cloned();
    Ok(FsmReport {
        fsm_rel: rel.to_string(),
        machine: corpus.machine,
        outcome: FsmOutcome::Drift {
            step,
            expected,
            actual,
        },
    })
}

/// Major token of a `fsm-trace/vN` schema string (`"fsm-trace/v1"` → `"v1"`;
/// an unrecognized string yields itself so it never accidentally compares
/// equal to a real major).
fn corpus_major(schema: &str) -> &str {
    schema.rsplit('/').next().unwrap_or(schema)
}

/// Render the verdict (human or `--json`) and return the contract exit code.
fn emit(args: &BaselineArgs, corpus_dir: &Path, reports: &[FsmReport]) -> ExitCode {
    let any_drift = reports
        .iter()
        .any(|r| matches!(r.outcome, FsmOutcome::Drift { .. }));
    let any_inconclusive = reports
        .iter()
        .any(|r| matches!(r.outcome, FsmOutcome::Inconclusive { .. }));

    // Precedence: drift (1) dominates inconclusive (2) dominates clean (0).
    // A real semantic drift is the most actionable signal; surface it even
    // if some other FSM lacked a baseline.
    let (verdict, exit): (&str, ExitCode) = if any_drift {
        ("drift", ExitCode::from(1))
    } else if any_inconclusive {
        ("inconclusive", ExitCode::from(2))
    } else {
        ("no-drift", ExitCode::SUCCESS)
    };
    let exit_num: u8 = match verdict {
        "no-drift" => 0,
        "drift" => 1,
        _ => 2,
    };
    let mode = if args.record { "record" } else { "check" };

    if args.json {
        let fsms: Vec<Value> = reports
            .iter()
            .map(|r| {
                let mut o = serde_json::Map::new();
                o.insert("fsm".into(), json!(r.fsm_rel));
                o.insert("machine".into(), json!(r.machine));
                match &r.outcome {
                    FsmOutcome::Match => {
                        o.insert("result".into(), json!("match"));
                    }
                    FsmOutcome::Recorded => {
                        o.insert("result".into(), json!("recorded"));
                    }
                    FsmOutcome::Inconclusive { reason } => {
                        o.insert("result".into(), json!("inconclusive"));
                        o.insert("reason".into(), json!(reason));
                    }
                    FsmOutcome::Drift {
                        step,
                        expected,
                        actual,
                    } => {
                        o.insert("result".into(), json!("drift"));
                        o.insert(
                            "firstMismatch".into(),
                            json!({
                                "step": step,
                                "expected": expected,
                                "actual": actual,
                            }),
                        );
                    }
                }
                Value::Object(o)
            })
            .collect();
        let root = json!({
            "schema": DIFF_SCHEMA,
            "mode": mode,
            "verdict": verdict,
            "exitCode": exit_num,
            "corpus": corpus_dir.display().to_string(),
            "fsms": fsms,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&root).unwrap_or_else(|_| "{}".into())
        );
        return exit;
    }

    // Human form.
    if args.record {
        for r in reports {
            println!("recorded: {} (machine {})", r.fsm_rel, r.machine);
        }
        println!(
            "\nfsm baseline: recorded {} FSM baseline(s) into {}",
            reports.len(),
            corpus_dir.display()
        );
        return exit;
    }

    let mut matched = 0usize;
    for r in reports {
        match &r.outcome {
            FsmOutcome::Match => {
                matched += 1;
            }
            FsmOutcome::Inconclusive { reason } => {
                println!("inconclusive: {}\n      {}", r.fsm_rel, reason);
            }
            FsmOutcome::Drift {
                step,
                expected,
                actual,
            } => {
                println!(
                    "DRIFT: {} (machine {}) — semantic divergence from baseline",
                    r.fsm_rel, r.machine
                );
                println!("  first mismatch at step #{step}");
                println!("  expected: {}", record_summary(expected.as_ref()));
                println!("  actual:   {}", record_summary(actual.as_ref()));
            }
            FsmOutcome::Recorded => {}
        }
    }
    println!(
        "\nfsm baseline: {} matched, {} drifted, {} inconclusive (of {} FSM(s)) — {}",
        matched,
        reports
            .iter()
            .filter(|r| matches!(r.outcome, FsmOutcome::Drift { .. }))
            .count(),
        reports
            .iter()
            .filter(|r| matches!(r.outcome, FsmOutcome::Inconclusive { .. }))
            .count(),
        reports.len(),
        verdict,
    );
    exit
}

/// One-line `StepRecord` summary for the human drift report (the full
/// record is in `--json`; this keeps the terminal diff actionable).
fn record_summary(rec: Option<&StepRecord>) -> String {
    let Some(rec) = rec else {
        return "(no record at this step — length divergence)".into();
    };
    let evt = rec
        .event_received
        .as_ref()
        .map(|e| e.name.as_str())
        .unwrap_or("-");
    format!(
        "traceId={} kind={:?} event={} configAfter={:?}",
        rec.trace_id, rec.kind, evt, rec.config_after
    )
}
