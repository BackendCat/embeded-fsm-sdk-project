//! `fsm test` runner — exercises the runner against `.trace` fixtures that
//! verify the three documented modes of the `expected` block:
//!
//! - Empty `expected` is a HARD FAIL by default (P1-1). The opt-in flag
//!   `--allow-empty-expected` re-enables the legacy capture-mode skip.
//! - Correct `expected` → the runner reports `pass:` and exits 0.
//! - Wrong `expected` → the runner reports `fail:` with an actionable diff.
//!
//! This is the CLI-level evidence for MVP gate **G6** plus the empty-expected
//! gate from P1-1.

use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;

fn write_suite_without_expected() -> (tempfile::TempDir, PathBuf) {
    let td = tempfile::tempdir().unwrap();
    let suite = td.path().to_path_buf();
    fs::write(
        suite.join("motor.fsm"),
        r#"language fsm 2.0
machine Motor {
    events {
        START
        STOP
    }
    initial Idle
    state Idle {
        on START -> Running
    }
    state Running {
        on STOP -> Idle
    }
}
"#,
    )
    .unwrap();
    // No `expected` block → under the new behavior the runner must FAIL by
    // default and SKIP with --allow-empty-expected. Tests below cover both.
    fs::write(
        suite.join("motor.trace"),
        r#"{
  "init": { "machineName": "Motor" },
  "steps": [
    { "action": "dispatch", "event": "START" },
    { "action": "dispatch", "event": "STOP" }
  ]
}
"#,
    )
    .unwrap();
    (td, suite)
}

#[test]
fn runner_fails_when_trace_has_no_expected_block() {
    let (_td, suite) = write_suite_without_expected();
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["test"])
        .arg(&suite)
        .assert()
        .failure()
        .code(1);
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        stdout.contains("fail:") && stdout.contains("motor.trace"),
        "expected `fail: …motor.trace` for empty-expected trace, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("no `expected` block"),
        "expected the failure reason to mention the missing `expected` block, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("fsm test: 0 passed, 1 failed"),
        "expected the runner summary to report 1 failed, got:\n{}",
        stdout
    );
}

#[test]
fn runner_skips_empty_expected_under_opt_in_flag() {
    let (_td, suite) = write_suite_without_expected();
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["test", "--allow-empty-expected"])
        .arg(&suite)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        stdout.contains("skip:") && stdout.contains("motor.trace"),
        "expected `skip: …motor.trace` under --allow-empty-expected, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("fsm test: 0 passed, 0 failed, 1 skipped"),
        "expected skip count in summary, got:\n{}",
        stdout
    );
}

#[test]
fn runner_passes_when_expected_matches_actual() {
    let td = tempfile::tempdir().unwrap();
    let suite = td.path();
    fs::write(
        suite.join("simple.fsm"),
        r#"language fsm 2.0
machine Simple {
    events { TICK }
    initial A
    state A { on TICK -> B }
    state B { }
}
"#,
    )
    .unwrap();
    // Init only → one record; the `enteredStates` and `configAfter` mirror
    // the simulator's actual init output for this trivial machine.
    fs::write(
        suite.join("simple.trace"),
        r#"{
  "init": {},
  "steps": [],
  "expected": [
    {
      "traceId": 0,
      "kind": "init",
      "virtualClockMs": 0,
      "enteredStates": ["s-Simple-A"],
      "configBefore": [],
      "configAfter": ["s-Simple-A"]
    }
  ]
}
"#,
    )
    .unwrap();
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["test"])
        .arg(suite)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        stdout.contains("pass:") && stdout.contains("simple.trace"),
        "expected pass for correct expected list, got:\n{}",
        stdout
    );
}

#[test]
fn runner_reports_mismatch_against_expected_records() {
    // Provide an `expected` list that intentionally mismatches reality (we
    // assert the initial config_after is "wrong-state"). The runner must
    // report a fail.
    let td = tempfile::tempdir().unwrap();
    let suite = td.path();
    fs::write(
        suite.join("simple.fsm"),
        r#"language fsm 2.0
machine Simple {
    events { TICK }
    initial A
    state A { on TICK -> B }
    state B { }
}
"#,
    )
    .unwrap();
    fs::write(
        suite.join("simple.trace"),
        r#"{
  "init": {},
  "steps": [],
  "expected": [
    {
      "traceId": 0,
      "kind": "init",
      "virtualClockMs": 0,
      "exitedStates": [],
      "enteredStates": ["s-Simple-A"],
      "actionsExecuted": [],
      "configBefore": [],
      "configAfter": ["wrong-state"]
    }
  ]
}
"#,
    )
    .unwrap();

    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["test"])
        .arg(suite)
        .assert()
        .failure()
        .code(1);
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        stdout.contains("fail:"),
        "expected `fail:` in mismatch output, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("trace mismatch"),
        "expected mismatch reason in output, got:\n{}",
        stdout
    );
    // The diff must surface the offending record on both sides so the
    // operator can fix the trace without re-running with extra flags.
    assert!(
        stdout.contains("expected:") && stdout.contains("actual:"),
        "expected the mismatch output to include both expected/actual record summaries, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("wrong-state"),
        "expected the expected-record summary to surface the bogus state name, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("s-Simple-A"),
        "expected the actual-record summary to surface the real state name, got:\n{}",
        stdout
    );
}
