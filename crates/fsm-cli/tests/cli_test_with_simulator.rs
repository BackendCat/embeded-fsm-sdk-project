//! `fsm test` runner — exercises the runner against a `.trace` that, before
//! the analyzer↔simulator contract was fixed, would fail with
//! `"root initial is not Initial"`. The test now drives the simulator
//! through init + START + STOP and confirms the runner reports `pass:`.
//!
//! This is the CLI-level evidence for MVP gate **G6**.

use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;

fn write_suite() -> (tempfile::TempDir, PathBuf) {
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
    // No `expected` block → any non-error trace passes. We still want the
    // simulator to actually execute init, dispatch START, dispatch STOP.
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
fn runner_executes_trace_through_simulator_post_contract_fix() {
    let (_td, suite) = write_suite();
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["test"])
        .arg(&suite)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        stdout.contains("pass:") && stdout.contains("motor.trace"),
        "expected `pass: …motor.trace` in runner output (the simulator must \
         have executed the trace without the old `root initial is not Initial` \
         error), got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("fsm test: 1 passed, 0 failed"),
        "expected the runner summary to report 1 passed, got:\n{}",
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
}
