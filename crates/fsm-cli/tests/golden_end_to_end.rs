//! End-to-end golden test exercising the full `parse → analyze → codegen →
//! gcc -Werror` pipeline **through the actual `fsm` binary**.
//!
//! When gcc is on PATH, success here means MVP gate **G3 + G4** is met
//! (Doc 23 §9): the v1.0 CLI emits source files that a strict compiler
//! accepts without warnings. The matching pure-Rust test lives in
//! `fsm-codegen-c/tests/gcc_compile.rs`; this one is the integration check
//! that the CLI itself does not regress the contract.
//!
//! When gcc is not on PATH the test prints a skip notice and exits OK so
//! CI on minimal runners still passes.
//!
//! [`golden_simulator_runs_motor`] also covers MVP gate **G6** (simulator
//! trace match): the same Motor fixture is driven through `parse + analyze
//! + fsm_simulator::Interpreter` and asserted to flow Idle → Running → Idle
//! → Faulted on the canonical event sequence. This proves the
//! analyzer↔simulator IR contract is honoured end-to-end.

use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;

use assert_cmd::Command;
use fsm_analyzer::analyze_with_source;
use fsm_parser::parse;
use fsm_simulator::{InitOptions, Interpreter};

const VALID_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/motor.fsm");

fn gcc_available() -> bool {
    StdCommand::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Tiny host HAL + extern stubs sufficient to link the generated Motor.c.
/// The generated `Motor_impl.h` declares only entry/exit hooks for the
/// active-at-rest states — for the v1.0 fixture there are no entry/exit
/// actions in the DSL, but the codegen still emits prototypes for them
/// (analyzer-derived from the state list) so we provide empty bodies.
fn host_hal_c() -> &'static str {
    r#"#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#include "fsm_hal.h"
#include "Motor.h"

uint32_t fsm_hal_clock_now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint32_t)((uint64_t)ts.tv_sec * 1000u + (uint64_t)ts.tv_nsec / 1000000u);
}

void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) {
        fprintf(stderr, "[FSM ASSERT] %s\n", msg);
        abort();
    }
}

void Motor_entry_IDLE(Motor_t *m)    { (void)m; }
void Motor_entry_RUNNING(Motor_t *m) { (void)m; }
void Motor_entry_FAULTED(Motor_t *m) { (void)m; }
void Motor_exit_IDLE(Motor_t *m)     { (void)m; }
void Motor_exit_RUNNING(Motor_t *m)  { (void)m; }
void Motor_exit_FAULTED(Motor_t *m)  { (void)m; }
"#
}

#[test]
fn cli_generated_motor_compiles_with_gcc_werror() {
    if !gcc_available() {
        eprintln!("[golden_end_to_end] gcc not on PATH — skipping; cargo test still passes");
        return;
    }

    let td = tempfile::tempdir().expect("tempdir");
    let out = td.path();

    // 1. CLI: parse → analyze → codegen-c → write files.
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--out"])
        .arg(out)
        .arg(VALID_FIXTURE)
        .assert()
        .success();

    // 2. Drop the host HAL implementation in alongside the generated files.
    fs::write(out.join("host_hal.c"), host_hal_c()).expect("write host_hal.c");

    // 3. gcc -Werror: this is the MVP gate G3+G4 acceptance.
    let result = StdCommand::new("gcc")
        .current_dir(out)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "-c", // compile only — no main, no link
            "Motor.c",
            "host_hal.c",
        ])
        .output()
        .expect("invoke gcc");

    if !result.status.success() || !result.stderr.is_empty() {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        // Surface the generated source so a regression review can read it
        // straight from the test log.
        eprintln!(
            "=== generated Motor.c ===\n{}",
            fs::read_to_string(out.join("Motor.c")).unwrap_or_default()
        );
        eprintln!(
            "=== generated Motor.h ===\n{}",
            fs::read_to_string(out.join("Motor.h")).unwrap_or_default()
        );
        eprintln!("=== gcc stdout ===\n{}", stdout);
        eprintln!("=== gcc stderr ===\n{}", stderr);
        panic!(
            "MVP GATE G3+G4 FAILED: gcc rejected CLI-generated Motor.c (status: {:?})",
            result.status.code()
        );
    }
}

#[test]
fn cli_generated_hal_compiles_standalone() {
    if !gcc_available() {
        eprintln!("[golden_end_to_end] gcc not on PATH — skipping");
        return;
    }

    let td = tempfile::tempdir().expect("tempdir");
    let out = td.path();

    // Only need fsm_hal.h here — but the CLI doesn't have an "emit-only-
    // HAL" flag, so we still run the full generate and use what we need.
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--out"])
        .arg(out)
        .arg(VALID_FIXTURE)
        .assert()
        .success();

    let stub = r#"#define FSM_HAL_PROVIDE_POSIX_REFERENCE
#include "fsm_hal.h"
int main(void) { return (int)fsm_hal_clock_now_ms() == 0 ? 0 : 0; }
"#;
    fs::write(out.join("hal_smoke.c"), stub).unwrap();

    let exe: &Path = &out.join("hal_smoke");
    let result = StdCommand::new("gcc")
        .current_dir(out)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "hal_smoke.c",
            "-o",
        ])
        .arg(exe)
        .output()
        .expect("invoke gcc");
    assert!(
        result.status.success() && result.stderr.is_empty(),
        "fsm_hal.h failed standalone compile: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

/// MVP gate **G6** — simulator-trace match.
///
/// Drive the canonical Motor fixture through the in-process simulator and
/// assert the active-state sequence Idle → Running → Faulted → Idle on the
/// declared transitions. This is the smoke test that the analyzer's IR is
/// compatible with the simulator (Doc 09 §4.4 + §5 contract).
#[test]
fn golden_simulator_runs_motor() {
    let src = fs::read_to_string(VALID_FIXTURE).expect("read motor.fsm");
    let pr = parse(&src);
    let result = analyze_with_source(&pr, VALID_FIXTURE, &src);

    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == fsm_diagnostics::Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "analyzer reported errors on motor.fsm: {:?}",
        errors
    );
    let ir = result.ir.expect("ir produced");

    let mut interp = Interpreter::new(&ir).expect("build interpreter");
    // The analyzer's v1.0 lowering does not yet emit guard expressions on
    // transitions, so the `[can_start]` annotation in motor.fsm has no
    // effect on the lowered IR — every transition fires unconditionally.
    // Once guard lowering ships, register `ex-Motor-can_start` via
    // `interp.externs_mut().register(...)` to drive deterministic outcomes.
    let records = interp
        .init(InitOptions {
            machine_name: "Motor".into(),
            ..Default::default()
        })
        .expect("init must succeed — proves Doc 09 §4.4/§5 contract is honoured");

    assert!(
        records
            .iter()
            .any(|r| r.config_after.iter().any(|s| s == "s-Motor-Idle")),
        "init must enter Idle, got records: {:?}",
        records
    );
    assert_eq!(interp.current_states(), vec!["s-Motor-Idle".to_string()]);

    // START → Running
    let recs = interp.dispatch("START").expect("dispatch START");
    assert!(
        recs.iter()
            .any(|r| r.config_after == vec!["s-Motor-Running"]),
        "after START active state must be Running, got: {:?}",
        recs
    );
    assert_eq!(interp.current_states(), vec!["s-Motor-Running".to_string()]);

    // STOP → Faulted
    let recs = interp.dispatch("STOP").expect("dispatch STOP");
    assert!(
        recs.iter()
            .any(|r| r.config_after == vec!["s-Motor-Faulted"]),
        "after STOP active state must be Faulted, got: {:?}",
        recs
    );
    assert_eq!(interp.current_states(), vec!["s-Motor-Faulted".to_string()]);

    // START (from Faulted) → Idle, closing the loop.
    let recs = interp
        .dispatch("START")
        .expect("dispatch START from Faulted");
    assert!(
        recs.iter().any(|r| r.config_after == vec!["s-Motor-Idle"]),
        "after START from Faulted active state must be Idle, got: {:?}",
        recs
    );
    assert_eq!(interp.current_states(), vec!["s-Motor-Idle".to_string()]);
}
