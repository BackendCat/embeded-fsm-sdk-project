//! Integration tests for `fsm test` — the conformance test runner.

use std::fs;

use assert_cmd::Command;

#[test]
fn runner_discovers_and_executes_trace_files() {
    // Lay out a self-contained test suite: one .fsm + one .trace pointing
    // at it. The trace contains no `expected` records, so the simulator
    // executes through init without producing a mismatch — the runner
    // should report `pass:`.
    //
    // (Before the analyzer↔simulator Initial-pseudo-state contract was
    // fixed the simulator returned `"root initial is not Initial"` here
    // and the runner reported `fail:`; that historical caveat is gone.)
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
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        stdout.contains("pass:") && stdout.contains("simple.trace"),
        "expected `pass: …simple.trace` after analyzer↔simulator contract \
         fix, got:\n{}",
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
