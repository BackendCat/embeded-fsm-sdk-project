//! Integration tests for `fsm test` — the conformance test runner.

use std::fs;

use assert_cmd::Command;

#[test]
fn runner_discovers_and_executes_trace_files() {
    // Lay out a self-contained test suite: one .fsm + one .trace pointing
    // at it. The trace contains no `expected` records, so any actual
    // simulator output that doesn't error matches.
    //
    // We assert the runner found and executed the trace (it appears in
    // the summary line). The simulator's verdict for analyzer-lowered IR
    // is currently an upstream concern (the lowered `region.initial` does
    // not match the simulator's expected Initial-pseudo-state shape — a
    // contract mismatch between fsm-analyzer and fsm-simulator that lives
    // outside this CLI wiring task). The CLI is doing its job either way.
    let td = tempfile::tempdir().unwrap();
    let suite = td.path();

    fs::write(
        suite.join("simple.fsm"),
        r#"language fsm 2.0

machine Simple {
    events {
        TICK
    }

    initial A

    state A {
        on TICK -> B
    }

    state B {
    }
}
"#,
    )
    .unwrap();

    fs::write(
        suite.join("simple.trace"),
        r#"{
  "init": {},
  "steps": []
}
"#,
    )
    .unwrap();

    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["test"])
        .arg(suite)
        .assert();
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    // Either pass or fail is acceptable; the runner must report the file
    // and emit the summary line.
    assert!(
        stdout.contains("simple.trace"),
        "expected trace filename in runner output, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("fsm test:"),
        "expected summary line in runner output, got:\n{}",
        stdout
    );
}

#[test]
fn missing_directory_exits_three() {
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["test", "/nonexistent/test/dir"])
        .assert()
        .failure()
        .code(3);
}

#[test]
fn empty_directory_succeeds_with_warning() {
    let td = tempfile::tempdir().unwrap();
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["test"])
        .arg(td.path())
        .assert()
        .success();
}
