//! Integration tests for `fsm check`.
//!
//! Doc 23 §9 "What Done Looks Like":
//!   - `fsm check motor.fsm`  → exit 0, no output.
//!   - `fsm check broken.fsm` → exit 1, Rust-style error with `--> file:line:col`.

use std::fs;

use assert_cmd::Command;
use predicates::str::contains;

const VALID_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/motor.fsm");

#[test]
fn clean_file_exits_zero_with_no_stderr_output() {
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["check", VALID_FIXTURE])
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&assertion.get_output().stderr);
    assert!(
        stderr.trim().is_empty(),
        "expected no stderr on clean file, got:\n{}",
        stderr
    );
}

#[test]
fn broken_file_exits_one_with_diagnostic_pointer() {
    // Write a deliberately broken source. Missing closing brace forces a
    // parser error that lands in the FSM-E0010 family.
    let td = tempfile::tempdir().unwrap();
    let path = td.path().join("broken.fsm");
    fs::write(
        &path,
        "language fsm 2.0\nmachine Broken {\n    initial Idle\n    state Idle {\n",
    )
    .unwrap();

    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["check"])
        .arg(&path)
        .assert()
        .failure()
        .code(1);
    let stderr = String::from_utf8_lossy(&assertion.get_output().stderr);
    // miette's GraphicalReportHandler renders "× message" (with the source
    // line citation). We assert on the file-relative path appearing in the
    // snippet — that proves the `-->` pointer machinery wired up.
    assert!(
        stderr.contains(path.file_name().unwrap().to_str().unwrap()),
        "expected file label in diagnostic, got:\n{}",
        stderr
    );
    // Diagnostic code is preserved through miette.
    assert!(
        stderr.contains("FSM-E"),
        "expected FSM-Exxxx code in diagnostic, got:\n{}",
        stderr
    );
}

#[test]
fn missing_file_exits_three() {
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["check", "/nonexistent/path/that/does/not/exist.fsm"])
        .assert()
        .failure()
        .code(3)
        .stderr(contains("cannot read"));
}

#[test]
fn json_flag_emits_array() {
    // Even on a clean file, --json should produce a well-formed `[]`.
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["check", "--json", VALID_FIXTURE])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    // JSON aggregate is a valid array. On a clean file it's empty.
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout is valid JSON");
    assert!(parsed.is_array(), "expected JSON array, got {:?}", parsed);
}

// ---------------------------------------------------------------------------
// FU#67 — `fsm.toml [compiler] allow`/`deny` (Doc 18 §6 / §6.1) end-to-end.
//
// These drive the REAL `fsm` binary against a fixture `.fsm` plus a
// sibling `fsm.toml`, asserting EXIT CODES and DIAGNOSTIC SEVERITY (via
// the structured `--json` projection so severity is machine-checked, not
// scraped from a rendered string) — never a `.contains()` on source.
// SUBAGENT_CONVENTIONS §5.4: behavioural acceptance, not symbol presence.
//
// `W0600_FIXTURE` emits EXACTLY ONE diagnostic, `FSM-W0600` (Warning).
// ---------------------------------------------------------------------------

const W0600_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/single_state_region_w0600.fsm"
);

/// `fsm check --json` over a copy of the fixture in `dir` (so the
/// directory's `fsm.toml`, if any, is the one the loader finds first).
/// Returns `(exit_code, parsed_json_or_none)`. JSON is `None` when the
/// run aborted before emitting the array (e.g. an exit-4 config error).
fn check_json_in(dir: &std::path::Path) -> (i32, Option<serde_json::Value>) {
    let fsm = dir.join("m.fsm");
    fs::copy(W0600_FIXTURE, &fsm).unwrap();
    let out = Command::cargo_bin("fsm")
        .unwrap()
        .args(["check", "--json"])
        .arg(&fsm)
        .output()
        .unwrap();
    let code = out.status.code().expect("process exited with a code");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json = serde_json::from_str::<serde_json::Value>(stdout.trim()).ok();
    (code, json)
}

/// Codes present in a `--json` diagnostic array, as `(code, severity)`.
fn codes(v: &serde_json::Value) -> Vec<(String, String)> {
    v.as_array()
        .expect("diagnostics JSON is an array")
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap().to_owned(),
                d["severity"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn compiler_no_allow_deny_is_unchanged_control() {
    // Control: an `fsm.toml` WITHOUT any `[compiler] allow`/`deny` must
    // leave the diagnostic set byte-identical to pre-FU#67 — W0600 is
    // still a Warning and exit is 0 (a lone warning is not an error).
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("fsm.toml"), "[generate]\ntarget = \"c99\"\n").unwrap();
    let (code, json) = check_json_in(td.path());
    let json = json.expect("control run emits the JSON array");
    assert_eq!(code, 0, "lone warning must not fail the check");
    assert_eq!(
        codes(&json),
        vec![("FSM-W0600".to_owned(), "warning".to_owned())],
        "control: exactly the one W0600 warning, unchanged"
    );
}

#[test]
fn compiler_allow_suppresses_the_listed_code() {
    // `allow = ["FSM-W0600"]` -> that diagnostic is removed entirely
    // (Doc 18 §6 "Suppress globally"): empty diagnostic set, exit 0.
    let td = tempfile::tempdir().unwrap();
    fs::write(
        td.path().join("fsm.toml"),
        "[compiler]\nallow = [\"FSM-W0600\"]\n",
    )
    .unwrap();
    let (code, json) = check_json_in(td.path());
    let json = json.expect("allow run still emits the (empty) JSON array");
    assert_eq!(code, 0);
    assert!(
        codes(&json).is_empty(),
        "allow must remove W0600 entirely, got {:?}",
        codes(&json)
    );
}

#[test]
fn compiler_deny_elevates_warning_to_error() {
    // `deny = ["FSM-W0600"]` -> that warning becomes an Error (Doc 18 §6
    // "Treat as error"): the code is still present but severity is
    // `error`, and the exit code is now 1.
    let td = tempfile::tempdir().unwrap();
    fs::write(
        td.path().join("fsm.toml"),
        "[compiler]\ndeny = [\"FSM-W0600\"]\n",
    )
    .unwrap();
    let (code, json) = check_json_in(td.path());
    let json = json.expect("deny run emits the JSON array");
    assert_eq!(code, 1, "a denied warning must fail the check");
    assert_eq!(
        codes(&json),
        vec![("FSM-W0600".to_owned(), "error".to_owned())],
        "deny must flip W0600 to error severity"
    );
}

#[test]
fn compiler_allow_wins_when_a_code_is_both_allowed_and_denied() {
    // Deterministic ordering: `allow` is applied before `deny`, so a code
    // in BOTH lists is suppressed (it is gone before `deny` runs) — a
    // suppressed diagnostic cannot also be "an error". Exit 0.
    let td = tempfile::tempdir().unwrap();
    fs::write(
        td.path().join("fsm.toml"),
        "[compiler]\nallow = [\"FSM-W0600\"]\ndeny = [\"FSM-W0600\"]\n",
    )
    .unwrap();
    let (code, json) = check_json_in(td.path());
    let json = json.expect("emits the (empty) JSON array");
    assert_eq!(code, 0, "allow wins -> suppressed -> not an error");
    assert!(codes(&json).is_empty(), "got {:?}", codes(&json));
}

#[test]
fn compiler_unknown_code_is_a_clean_exit_4_not_a_silent_ignore() {
    // The FU#67 cardinal-sin guard: a code string the registry does not
    // know (neither live nor retired) is a LOUD exit-4 config error with
    // a clear message — NEVER silently dropped. No diagnostics are
    // rendered; the user sees only the config error.
    let td = tempfile::tempdir().unwrap();
    fs::write(
        td.path().join("fsm.toml"),
        "[compiler]\nallow = [\"FSM-W9999\"]\n",
    )
    .unwrap();
    let fsm = td.path().join("m.fsm");
    fs::copy(W0600_FIXTURE, &fsm).unwrap();
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["check"])
        .arg(&fsm)
        .assert()
        .failure()
        .code(4);
    let stderr = String::from_utf8_lossy(&assertion.get_output().stderr);
    assert!(
        stderr.contains("unknown diagnostic code") && stderr.contains("FSM-W9999"),
        "expected a clear unknown-code config error, got:\n{}",
        stderr
    );
    // And it must NOT have leaked the suppressed-attempt diagnostic.
    assert!(
        !stderr.contains("W0600"),
        "config error must abort before any diagnostic is rendered, got:\n{}",
        stderr
    );
}

#[test]
fn compiler_retired_code_is_silently_accepted_doc10_rule2() {
    // Doc 10 §14 rule 2: a RETIRED code (e.g. FSM-E0301) MUST still parse
    // and be silently accepted in suppression contexts — a project that
    // pinned `allow = ["FSM-E0301"]` before that code was retired must
    // not start failing. It is NOT an exit-4 error; the run proceeds
    // normally (here nothing emits E0301, so W0600 is untouched, exit 0).
    let td = tempfile::tempdir().unwrap();
    fs::write(
        td.path().join("fsm.toml"),
        "[compiler]\nallow = [\"FSM-E0301\"]\n",
    )
    .unwrap();
    let (code, json) = check_json_in(td.path());
    let json = json.expect("retired-code run proceeds and emits the array");
    assert_eq!(code, 0, "retired code in allow is accepted, not exit-4");
    assert_eq!(
        codes(&json),
        vec![("FSM-W0600".to_owned(), "warning".to_owned())],
        "E0301 not emitted here, so W0600 is untouched"
    );
}
