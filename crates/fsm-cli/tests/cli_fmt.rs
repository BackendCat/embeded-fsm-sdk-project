//! Integration tests for `fsm fmt`.
//!
//! Doc 23 §9 "What Done Looks Like":
//!   `fsm fmt f.fsm && fsm fmt --check f.fsm` → second invocation exits 0.

use std::fs;

use assert_cmd::Command;

const VALID_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/motor.fsm");

#[test]
fn check_on_already_canonical_fixture_exits_zero() {
    // The shipped fixture is already canonical (the formatter is
    // idempotent — see fsm-formatter/tests/idempotency.rs). Confirm.
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["fmt", "--check", VALID_FIXTURE])
        .assert()
        .success();
}

#[test]
fn check_on_unformatted_file_exits_one() {
    // Hand-roll a file with deliberately weird spacing.
    let td = tempfile::tempdir().unwrap();
    let path = td.path().join("ugly.fsm");
    fs::write(
        &path,
        "language fsm 2.0\n\n\n\nmachine M {\n  initial Idle\n  state Idle { }\n}\n",
    )
    .unwrap();

    Command::cargo_bin("fsm")
        .unwrap()
        .args(["fmt", "--check"])
        .arg(&path)
        .assert()
        .failure()
        .code(1);
}

#[test]
fn write_then_check_round_trip() {
    let td = tempfile::tempdir().unwrap();
    let path = td.path().join("round.fsm");
    // Same scruffy starting point as above.
    fs::write(
        &path,
        "language fsm 2.0\n\n\n\nmachine M {\n  initial Idle\n  state Idle { }\n}\n",
    )
    .unwrap();

    // 1. Format in place.
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["fmt"])
        .arg(&path)
        .assert()
        .success();

    // 2. --check on the now-canonical file must exit 0.
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["fmt", "--check"])
        .arg(&path)
        .assert()
        .success();
}

#[test]
fn stdin_mode_writes_to_stdout() {
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["fmt", "--stdin"])
        .write_stdin("language fsm 2.0\nmachine M { initial Idle state Idle { } }\n")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        stdout.contains("language fsm 2.0"),
        "expected formatted output on stdout, got:\n{}",
        stdout
    );
}
