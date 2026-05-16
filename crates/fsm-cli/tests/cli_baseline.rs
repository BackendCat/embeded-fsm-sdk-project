//! `fsm baseline` (v1.4-W3 trace differential replay) CLI end-to-end
//! acceptance — Doc 30 §4.2-W3 / §5.4. Behavioural, driven through the
//! **real binary** against the committed `w3_baseline_suite` /
//! `w3_baseline_corpus` factory fixtures. Symbol-presence is explicitly
//! NOT acceptance here: every test invokes the shipped harness and asserts
//! its exit code + stdout contract.
//!
//! Coverage maps directly to the W3 §5.4 (a)-(d) acceptance bar:
//!
//! - **(a) no false drift** — capture-from-this-build corpus + re-run
//!   compare on the unchanged build ⇒ exit 0, zero drift, clean `--json`.
//! - **(b) real drift caught** — a hand-mutated baseline `StepRecord` ⇒
//!   exit 1, correct `first_mismatch` step index, the `--json`
//!   first-mismatch payload names the right FSM/step with expected≠actual.
//! - **(c) determinism** — capture twice on the same build ⇒ byte-identical
//!   corpus files.
//! - **(d) inconclusive / IO** — absent baseline ⇒ exit 2 (not a panic, not
//!   a false "no drift"); not-a-directory suite ⇒ exit 3.
//!
//! Plus the documented exit-code contract, the `fsm-trace/v1` corpus
//! version marker, the `fsm-trace-diff/v1` `--json` schema, and the
//! schema-major-mismatch ⇒ inconclusive versioning gate.

use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use serde_json::Value;

/// The committed curated W3 suite (representative breadth: flat motor,
/// hierarchical+timer+history traffic-light, deferred, submachine,
/// composite, parallel).
fn suite_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/w3_baseline_suite")
}

/// The committed frozen baseline corpus captured from a known-good build.
fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/w3_baseline_corpus")
}

fn run_baseline(args: &[&str]) -> (i32, String, String) {
    let out = Command::cargo_bin("fsm")
        .unwrap()
        .arg("baseline")
        .args(args)
        .output()
        .expect("spawn fsm baseline");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// ---------------------------------------------------------------------------
// (a) No false drift: the committed corpus matches this build exactly.
// ---------------------------------------------------------------------------

#[test]
fn check_unchanged_build_reports_no_drift_exit_zero() {
    let (code, stdout, _err) = run_baseline(&[
        "--check",
        "--corpus",
        corpus_dir().to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(
        code, 0,
        "an unchanged build must produce zero drift (exit 0); stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("0 drifted") && stdout.contains("no-drift"),
        "human summary must report no drift; got:\n{stdout}"
    );
}

#[test]
fn check_unchanged_build_json_is_clean_and_schema_versioned() {
    let (code, stdout, _err) = run_baseline(&[
        "--check",
        "--json",
        "--corpus",
        corpus_dir().to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "clean check exits 0; stdout:\n{stdout}");
    let v: Value = serde_json::from_str(&stdout).expect("--json must be valid JSON");
    assert_eq!(
        v["schema"], "fsm-trace-diff/v1",
        "json must be schema-versioned"
    );
    assert_eq!(v["verdict"], "no-drift");
    assert_eq!(v["exitCode"], 0);
    assert_eq!(v["mode"], "check");
    let fsms = v["fsms"].as_array().expect("fsms array");
    assert_eq!(fsms.len(), 6, "the curated suite has 6 FSMs");
    assert!(
        fsms.iter().all(|f| f["result"] == "match"),
        "every FSM must match on an unchanged build; got:\n{stdout}"
    );
    // Deterministic walk order (sorted): the array is stable across runs.
    let (_c2, stdout2, _e2) = run_baseline(&[
        "--check",
        "--json",
        "--corpus",
        corpus_dir().to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(
        stdout, stdout2,
        "--json must be byte-deterministic across two identical runs"
    );
}

// ---------------------------------------------------------------------------
// (b) Real drift caught: a hand-mutated baseline StepRecord.
// ---------------------------------------------------------------------------

#[test]
fn mutated_baseline_record_is_caught_with_correct_first_mismatch() {
    let td = tempfile::tempdir().unwrap();
    let mutated_corpus = td.path().join("corpus");
    fs::create_dir_all(&mutated_corpus).unwrap();
    // Copy the frozen corpus, then mutate exactly one StepRecord field of
    // one FSM (the TICK-dispatched step's configAfter) — a real semantic
    // divergence the interpreter's own comparator must flag.
    for entry in fs::read_dir(corpus_dir()).unwrap() {
        let p = entry.unwrap().path();
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let raw = fs::read_to_string(&p).unwrap();
        if name == "composite__composite.fsm.json" {
            let mut doc: Value = serde_json::from_str(&raw).unwrap();
            // expected[1] is the `TICK` dispatched step.
            doc["trace"]["expected"][1]["configAfter"] =
                Value::Array(vec![Value::String("s-Composite-WRONG".into())]);
            fs::write(
                mutated_corpus.join(&name),
                serde_json::to_string_pretty(&doc).unwrap(),
            )
            .unwrap();
        } else {
            fs::write(mutated_corpus.join(&name), raw).unwrap();
        }
    }

    // Human form: exit 1, names the FSM + the step index.
    let (code, stdout, _err) = run_baseline(&[
        "--check",
        "--corpus",
        mutated_corpus.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(
        code, 1,
        "a mutated baseline must be detected as drift (exit 1); stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("DRIFT: composite/composite.fsm"),
        "drift report must name the diverged FSM; got:\n{stdout}"
    );
    assert!(
        stdout.contains("first mismatch at step #1"),
        "drift report must carry the correct first-mismatch index (#1); got:\n{stdout}"
    );
    assert!(
        stdout.contains("s-Composite-WRONG") && stdout.contains("s-Composite-Inner2"),
        "human diff must surface expected (wrong) vs actual (real) states; got:\n{stdout}"
    );
    assert!(
        stdout.contains("5 matched, 1 drifted"),
        "summary must count exactly one drift; got:\n{stdout}"
    );

    // --json form: machine-consumable first-mismatch payload.
    let (jcode, jstdout, _je) = run_baseline(&[
        "--check",
        "--json",
        "--corpus",
        mutated_corpus.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(jcode, 1, "json drift path must also exit 1");
    let v: Value = serde_json::from_str(&jstdout).expect("valid json");
    assert_eq!(v["schema"], "fsm-trace-diff/v1");
    assert_eq!(v["verdict"], "drift");
    assert_eq!(v["exitCode"], 1);
    let composite = v["fsms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["fsm"] == "composite/composite.fsm")
        .expect("composite entry present");
    assert_eq!(composite["result"], "drift");
    let fm = &composite["firstMismatch"];
    assert_eq!(
        fm["step"], 1,
        "the json first-mismatch step index must be 1"
    );
    assert_eq!(
        fm["expected"]["configAfter"][0], "s-Composite-WRONG",
        "json must carry the (mutated) expected record"
    );
    assert_eq!(
        fm["actual"]["configAfter"][0], "s-Composite-Inner2",
        "json must carry the real interpreter-produced actual record (expected≠actual)"
    );
    // Every other FSM still matches — drift is isolated, not a blanket fail.
    assert!(
        v["fsms"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["fsm"] != "composite/composite.fsm")
            .all(|f| f["result"] == "match"),
        "only the mutated FSM may drift; got:\n{jstdout}"
    );
}

#[test]
fn length_divergent_baseline_is_caught_as_drift_with_divergence_index() {
    // A different angle on (b): the length-divergence path. `--check`
    // replays the *frozen* trace (its `init` + `steps` are part of the
    // immutable oracle — drift means "same frozen input, different
    // interpreter output", which is the correct regression-oracle
    // semantics; the live suite `.steps.json` is only consulted on
    // `--record`). Truncating the frozen `expected` list while leaving the
    // frozen `steps` intact makes the interpreter produce MORE records than
    // the baseline ⇒ a length divergence drift, reported at the divergence
    // index with an explicit "no record at this step" marker.
    let td = tempfile::tempdir().unwrap();
    let corpus = td.path().join("corpus");
    fs::create_dir_all(&corpus).unwrap();
    for entry in fs::read_dir(corpus_dir()).unwrap() {
        let p = entry.unwrap().path();
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let raw = fs::read_to_string(&p).unwrap();
        if name == "motor__motor.fsm.json" {
            let mut doc: Value = serde_json::from_str(&raw).unwrap();
            // Drop the last 2 frozen expected records; keep `steps` whole.
            let exp = doc["trace"]["expected"].as_array().unwrap().clone();
            let kept = exp[..exp.len() - 2].to_vec();
            let divergence_idx = kept.len();
            doc["trace"]["expected"] = Value::Array(kept);
            fs::write(
                corpus.join(&name),
                serde_json::to_string_pretty(&doc).unwrap(),
            )
            .unwrap();
            // Sanity: the divergence index is deterministic for this fixture.
            assert!(divergence_idx >= 1);
        } else {
            fs::write(corpus.join(&name), raw).unwrap();
        }
    }

    let (code, stdout, _err) = run_baseline(&[
        "--check",
        "--corpus",
        corpus.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(
        code, 1,
        "a length-divergent baseline must be detected as drift (exit 1); \
         stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("DRIFT: motor/motor.fsm"),
        "the length-divergent motor must be reported as drift; got:\n{stdout}"
    );
    assert!(
        stdout.contains("length divergence"),
        "the human report must explain the length divergence; got:\n{stdout}"
    );

    // --json must still carry a machine-consumable first-mismatch (the
    // divergence index; the expected side is null past the baseline end).
    let (jcode, jstdout, _je) = run_baseline(&[
        "--check",
        "--json",
        "--corpus",
        corpus.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(jcode, 1);
    let v: Value = serde_json::from_str(&jstdout).unwrap();
    assert_eq!(v["verdict"], "drift");
    let motor = v["fsms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["fsm"] == "motor/motor.fsm")
        .unwrap();
    assert_eq!(motor["result"], "drift");
    assert!(
        motor["firstMismatch"]["step"].as_u64().is_some(),
        "json must carry the divergence step index; got:\n{jstdout}"
    );
    assert!(
        motor["firstMismatch"]["expected"].is_null(),
        "past the truncated baseline end the expected side is null; got:\n{jstdout}"
    );
    assert!(
        motor["firstMismatch"]["actual"].is_object(),
        "the interpreter-produced actual record is present at the divergence; \
         got:\n{jstdout}"
    );
}

// ---------------------------------------------------------------------------
// (c) Determinism: capture twice on the same build ⇒ byte-identical.
// ---------------------------------------------------------------------------

#[test]
fn recording_twice_yields_byte_identical_corpus() {
    let td = tempfile::tempdir().unwrap();
    let c1 = td.path().join("c1");
    let c2 = td.path().join("c2");

    for c in [&c1, &c2] {
        let (code, _o, err) = run_baseline(&[
            "--record",
            "--corpus",
            c.to_str().unwrap(),
            suite_dir().to_str().unwrap(),
        ]);
        assert_eq!(code, 0, "record must succeed; stderr:\n{err}");
    }

    let mut names1: Vec<_> = fs::read_dir(&c1)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let mut names2: Vec<_> = fs::read_dir(&c2)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names1.sort();
    names2.sort();
    assert_eq!(
        names1, names2,
        "the two captures must produce the same files"
    );
    assert!(!names1.is_empty(), "the curated suite must produce ≥1 file");

    for name in &names1 {
        let a = fs::read(c1.join(name)).unwrap();
        let b = fs::read(c2.join(name)).unwrap();
        assert_eq!(
            a, b,
            "corpus file {:?} must be byte-identical across two captures on the \
             same build (StepRecord determinism, Doc 13 §11)",
            name
        );
    }

    // And the freshly-captured corpus must itself round-trip clean (no
    // false drift between a capture and an immediate compare).
    let (code, stdout, _err) = run_baseline(&[
        "--check",
        "--corpus",
        c1.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(
        code, 0,
        "a just-captured corpus must compare clean on the same build; stdout:\n{stdout}"
    );
}

#[test]
fn committed_corpus_matches_a_fresh_capture_on_this_build() {
    // Guards the committed fixture against silent drift: re-capturing on
    // the current build must reproduce the committed corpus byte-for-byte.
    // If this fails, either a real semantic change landed (then the fix is
    // to re-record + review the diff) or determinism broke (a P0).
    let td = tempfile::tempdir().unwrap();
    let fresh = td.path().join("fresh");
    let (code, _o, err) = run_baseline(&[
        "--record",
        "--corpus",
        fresh.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "record must succeed; stderr:\n{err}");

    for entry in fs::read_dir(corpus_dir()).unwrap() {
        let committed = entry.unwrap().path();
        let name = committed.file_name().unwrap();
        let fresh_file = fresh.join(name);
        assert!(
            fresh_file.is_file(),
            "fresh capture is missing committed corpus file {:?}",
            name
        );
        assert_eq!(
            fs::read(&committed).unwrap(),
            fs::read(&fresh_file).unwrap(),
            "committed corpus file {:?} diverged from a fresh capture on this \
             build — re-record (semantic change) or investigate determinism (P0)",
            name
        );
    }
}

// ---------------------------------------------------------------------------
// (d) Inconclusive / IO paths.
// ---------------------------------------------------------------------------

#[test]
fn absent_baseline_is_inconclusive_exit_two_not_false_no_drift() {
    let td = tempfile::tempdir().unwrap();
    let empty_corpus = td.path().join("nope");
    // Note: directory does not exist at all — the check path must treat a
    // missing baseline as INCONCLUSIVE (exit 2), never a panic and never a
    // false "no drift" (exit 0).
    let (code, stdout, _err) = run_baseline(&[
        "--check",
        "--corpus",
        empty_corpus.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(
        code, 2,
        "an absent baseline must be INCONCLUSIVE (exit 2), not a false no-drift; \
         stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("inconclusive") && stdout.contains("6 inconclusive"),
        "human output must report inconclusive for every FSM; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("no-drift"),
        "an absent baseline must NOT report no-drift; got:\n{stdout}"
    );

    let (jcode, jstdout, _je) = run_baseline(&[
        "--check",
        "--json",
        "--corpus",
        empty_corpus.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(jcode, 2, "json inconclusive path must also exit 2");
    let v: Value = serde_json::from_str(&jstdout).unwrap();
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(v["exitCode"], 2);
    assert!(
        v["fsms"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["result"] == "inconclusive" && f["reason"].is_string()),
        "every FSM must be inconclusive with a reason; got:\n{jstdout}"
    );
}

#[test]
fn suite_not_a_directory_is_io_error_exit_three() {
    let (code, _stdout, err) = run_baseline(&[
        "--check",
        suite_dir().join("motor/motor.fsm").to_str().unwrap(),
    ]);
    assert_eq!(
        code, 3,
        "a non-directory suite is an IO error (exit 3); stderr:\n{err}"
    );
    assert!(
        err.contains("not a directory"),
        "stderr must explain the IO error; got:\n{err}"
    );
}

#[test]
fn empty_suite_dir_is_io_error_exit_three() {
    let td = tempfile::tempdir().unwrap();
    let (code, _stdout, err) = run_baseline(&["--check", td.path().to_str().unwrap()]);
    assert_eq!(
        code, 3,
        "a suite with no .fsm files is an IO error (exit 3); stderr:\n{err}"
    );
    assert!(
        err.contains("no .fsm files"),
        "stderr must explain there is nothing to baseline; got:\n{err}"
    );
}

#[test]
fn unanalyzable_fsm_is_out_of_scope_exit_four() {
    let td = tempfile::tempdir().unwrap();
    let suite = td.path().join("bad");
    fs::create_dir_all(&suite).unwrap();
    // Syntactically broken .fsm — cannot capture/compare what won't
    // compile; must be exit 4 (out-of-scope), distinct from a drift
    // verdict so a factory script never confuses "bad input" with
    // "semantics drifted".
    fs::write(suite.join("broken.fsm"), "language fsm 2.0\nmachine {{{ ").unwrap();
    let (code, _stdout, err) = run_baseline(&[
        "--record",
        "--corpus",
        td.path().join("c").to_str().unwrap(),
        suite.to_str().unwrap(),
    ]);
    assert_eq!(
        code, 4,
        "an unanalyzable suite FSM is out-of-scope (exit 4); stderr:\n{err}"
    );
}

// ---------------------------------------------------------------------------
// Versioning gate: a non-`fsm-trace/v1` corpus ⇒ inconclusive (never a
// false verdict). The schema marker is the factory-artifact durability
// contract; this proves it is enforced on read.
// ---------------------------------------------------------------------------

#[test]
fn corpus_with_incompatible_schema_major_is_inconclusive() {
    let td = tempfile::tempdir().unwrap();
    let corpus = td.path().join("corpus");
    fs::create_dir_all(&corpus).unwrap();
    // Take one real corpus file and bump its schema major to a future
    // (incompatible) version; copy the rest verbatim.
    for entry in fs::read_dir(corpus_dir()).unwrap() {
        let p = entry.unwrap().path();
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let raw = fs::read_to_string(&p).unwrap();
        if name == "motor__motor.fsm.json" {
            let mut doc: Value = serde_json::from_str(&raw).unwrap();
            doc["schema"] = Value::String("fsm-trace/v999".into());
            fs::write(
                corpus.join(&name),
                serde_json::to_string_pretty(&doc).unwrap(),
            )
            .unwrap();
        } else {
            fs::write(corpus.join(&name), raw).unwrap();
        }
    }
    let (code, stdout, _err) = run_baseline(&[
        "--check",
        "--json",
        "--corpus",
        corpus.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    // Drift precedence aside, every other FSM matches and the v999 one is
    // inconclusive ⇒ overall inconclusive (exit 2), never a false no-drift
    // and never a false drift on a schema we cannot compare.
    assert_eq!(
        code, 2,
        "an incompatible corpus schema major must be inconclusive (exit 2); \
         stdout:\n{stdout}"
    );
    let v: Value = serde_json::from_str(&stdout).unwrap();
    let motor = v["fsms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["fsm"] == "motor/motor.fsm")
        .unwrap();
    assert_eq!(motor["result"], "inconclusive");
    assert!(
        motor["reason"].as_str().unwrap().contains("not compatible"),
        "the reason must explain the schema incompatibility; got:\n{stdout}"
    );
}

#[test]
fn corpus_files_carry_the_fsm_trace_v1_schema_marker() {
    // The persisted artifact MUST self-describe its version (the
    // factory-artifact durability contract). Assert it on the committed
    // corpus directly.
    let mut checked = 0;
    for entry in fs::read_dir(corpus_dir()).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let v: Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(
            v["schema"], "fsm-trace/v1",
            "every corpus file must carry the fsm-trace/v1 marker: {:?}",
            p
        );
        assert!(
            v["fsm"].is_string(),
            "corpus envelope must record provenance"
        );
        assert!(
            v["machine"].is_string(),
            "corpus envelope must record the machine"
        );
        assert!(
            v["trace"]["expected"]
                .as_array()
                .map_or(false, |a| !a.is_empty()),
            "corpus envelope must carry a non-empty captured expected list: {:?}",
            p
        );
        checked += 1;
    }
    assert!(
        checked >= 6,
        "expected ≥6 committed corpus files, saw {checked}"
    );
}

// ---------------------------------------------------------------------------
// Headless pipeline composition (the owner pipeline-before-UI bar): record
// then check is a pure CLI loop, zero daemon, deterministic.
// ---------------------------------------------------------------------------

#[test]
fn record_then_check_is_a_clean_headless_loop() {
    let td = tempfile::tempdir().unwrap();
    let corpus = td.path().join("corpus");
    let (rc, rout, _re) = run_baseline(&[
        "--record",
        "--json",
        "--corpus",
        corpus.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(rc, 0, "record must exit 0; stdout:\n{rout}");
    let rv: Value = serde_json::from_str(&rout).unwrap();
    assert_eq!(rv["mode"], "record");
    assert_eq!(rv["verdict"], "no-drift");
    assert!(
        rv["fsms"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["result"] == "recorded"),
        "record mode reports every FSM as recorded; got:\n{rout}"
    );

    let (cc, cout, _ce) = run_baseline(&[
        "--check",
        "--json",
        "--corpus",
        corpus.to_str().unwrap(),
        suite_dir().to_str().unwrap(),
    ]);
    assert_eq!(
        cc, 0,
        "the immediately-following check must be clean; stdout:\n{cout}"
    );
    let cv: Value = serde_json::from_str(&cout).unwrap();
    assert_eq!(cv["verdict"], "no-drift");
}
