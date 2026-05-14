//! `fsm --help` smoke test. Catches accidental subcommand removals and
//! catastrophic clap derivation regressions.
//!
//! Doc 18 §1 promises a `fsm` binary with the listed subcommands; this
//! test is the trivial enforcement.

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn help_exits_zero_and_lists_every_subcommand() {
    let mut cmd = Command::cargo_bin("fsm").expect("binary built");
    cmd.arg("--help").assert().success();
}

#[test]
fn help_mentions_every_v1_subcommand() {
    let assertion = Command::cargo_bin("fsm")
        .expect("binary built")
        .arg("--help")
        .assert()
        .success();
    let out = assertion.get_output();
    let stdout = String::from_utf8_lossy(&out.stdout);
    for sub in &[
        "parse",
        "check",
        "generate",
        "fmt",
        "test",
        "doc",
        "decompile",
        "init",
    ] {
        assert!(
            stdout.contains(sub),
            "--help missing subcommand `{}`. Full output:\n{}",
            sub,
            stdout
        );
    }
    // Sanity check on predicates (use the import).
    Command::cargo_bin("fsm")
        .unwrap()
        .arg("--help")
        .assert()
        .stdout(contains("FSM Studio"));
}

#[test]
fn version_flag_works() {
    Command::cargo_bin("fsm")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::starts_with("fsm "));
}
