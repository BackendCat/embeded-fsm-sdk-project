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
