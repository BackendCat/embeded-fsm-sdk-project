//! Phase-audit **P1-2** codegen-safety acceptance (CLI seam).
//!
//! On `main`, a submachine reference nested inside a composite/parallel
//! state slipped through `fsm check` (exit 0) and `fsm generate` happily
//! emitted C that then FAILED the project's own §5.4-mandated
//! `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` with
//! `implicit declaration of function 'Host_exit_INNER' / 'Host_entry_INNER'`
//! (W2d's `collect_sub_refs` walks only the root region, so the nested
//! ref's entry_/exit_ were called but never declared). That is a real
//! prose-vs-code defect: the docs called the nested case an "inert leaf"
//! but it miscompiled.
//!
//! After the fix the analyzer rejects the nested ref with an error-severity
//! `FSM-E0502`, so `fsm check` exits non-zero AND `fsm generate` aborts
//! *before codegen* (crates/fsm-cli/src/cmd/generate.rs: "abort on any
//! error-severity diagnostic"). Therefore the broken-C path is no longer
//! reachable from any CLI invocation — there is no input that produces
//! non-`-Werror` C via the nested-submachine route, because such input is
//! rejected at analysis.
//!
//! This is the §5.4 behavioural-acceptance angle for a *rejection* fix: the
//! observable, user-facing behaviour is "the tool refuses, with an
//! actionable diagnostic, and writes no output", proven through the real
//! `fsm` binary (not an internal fixture). The companion analyzer test
//! `fsm-analyzer/tests/nested_submachine_rejected.rs` proves the lowering
//! never emits a nested `StateNode::Submachine` even on the partial-IR
//! path, so codegen's input genuinely cannot contain the broken construct.
//!
//! FAIL-on-main: on `main` `fsm check` exits 0 (no E0502) and `fsm
//! generate` exits 0 writing `Host.c` (which then fails gcc -Werror) — both
//! `failure()` assertions below fail. PASS-after: both exit non-zero, no
//! `.c` is written, and the top-level control still generates + compiles
//! gcc-clean.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::process::Command as StdCommand;

use assert_cmd::Command;

const NESTED_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/nested_submachine.fsm"
);

/// `fsm check` on a nested-submachine source exits non-zero and names
/// `FSM-E0502` with the actionable message. (On `main`: exit 0, no
/// diagnostic — this assertion fails.)
#[test]
fn fsm_check_rejects_nested_submachine_with_e0502() {
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["check", NESTED_FIXTURE])
        .assert()
        .failure();
    let out = assertion.get_output();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("FSM-E0502"),
        "expected FSM-E0502 in `fsm check` output, got:\n{combined}"
    );
    assert!(
        combined.contains("top-level") && combined.contains("SUB-FU-2"),
        "expected the actionable nested-ref message, got:\n{combined}"
    );
}

/// `fsm generate` on the SAME source also refuses (exit non-zero) and writes
/// NO C — codegen never runs, so the previously-broken `gcc -Werror` path is
/// unreachable. (On `main`: exit 0 + `Host.c` written → assertion fails.)
#[test]
fn fsm_generate_refuses_nested_submachine_and_writes_no_c() {
    let td = tempfile::tempdir().unwrap();
    let out = td.path();

    Command::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--out"])
        .arg(out)
        .arg(NESTED_FIXTURE)
        .assert()
        .failure();

    // The decisive codegen-safety assertion: not a single `.c` file exists,
    // so there is no artifact that could fail gcc -Werror. On `main` this
    // directory contains `Host.c` (+ `Sub.c`, `fsm_hal.h`, …).
    let entries: Vec<String> = fs::read_dir(out)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    let c_files: Vec<&String> = entries.iter().filter(|n| n.ends_with(".c")).collect();
    assert!(
        c_files.is_empty(),
        "fsm generate must write NO C for a rejected nested-submachine \
         source (codegen never runs); found: {entries:?}"
    );
}

/// No-regression at the CLI seam: the canonical *top-level* submachine
/// example still `fsm generate`s AND the emitted C compiles cleanly under
/// the project's mandated strict gcc flags. This is the supported W2a–W2d
/// path; the P1-2 fix must not touch it.
#[test]
fn top_level_submachine_example_still_generates_and_compiles_werror_clean() {
    if common::should_skip_gcc("nested_submachine_no_broken_c") {
        return;
    }
    let src = common::workspace_root().join("examples/submachine/submachine.fsm");
    let td = tempfile::tempdir().unwrap();
    let out = td.path();

    Command::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--out"])
        .arg(out)
        .arg(&src)
        .assert()
        .success();

    // Device + Connection sources must both be emitted (top-level
    // submachine path intact).
    for f in &["Device.c", "Device.h", "Connection.c", "Connection.h"] {
        assert!(
            out.join(f).is_file(),
            "expected top-level submachine output {f}; got: {:?}",
            fs::read_dir(out)
                .unwrap()
                .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
                .collect::<Vec<_>>()
        );
    }

    let result = StdCommand::new("gcc")
        .current_dir(out)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "-c",
            "Device.c",
            "Connection.c",
        ])
        .output()
        .expect("invoke gcc");

    assert!(
        result.status.success() && result.stderr.is_empty(),
        "top-level submachine C must compile gcc -Werror clean (no \
         regression). gcc status: {:?}\nstderr:\n{}",
        result.status.code(),
        String::from_utf8_lossy(&result.stderr)
    );
}
