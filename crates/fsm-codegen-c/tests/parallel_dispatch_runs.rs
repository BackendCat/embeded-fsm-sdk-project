//! Behavioral parallel-dispatch test — P0-2 + P0-3 (Doc 00 §7.8 B-11).
//!
//! Generates C99 from a parallel two-region IR, compiles it with
//! `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, links a tiny host
//! main that:
//!
//! 1. Initializes the machine.
//! 2. Dispatches `A_DONE` and asserts only region A advanced (region B
//!    is untouched).
//! 3. Dispatches `B_DONE` and asserts BOTH regions are now in their
//!    Final state.
//! 4. Confirms `_active_count == 2` throughout (parallel two-region
//!    configuration).
//!
//! Any failure in the generated C surfaces as a non-zero exit code from
//! the built binary, which the test asserts on.
//!
//! Replaces the symbol-presence-only `parallel_completion.rs` checks
//! per the audit recommendation.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig};

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn write_files(dir: &Path, out: &fsm_codegen_c::EmittedFiles) {
    for f in &out.files {
        fs::write(dir.join(&f.path), &f.content).expect("write generated file");
    }
}

fn write_host_hal_c(dir: &Path) {
    let hal = r#"#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

uint32_t fsm_hal_clock_now_ms(void) { return 0; }

void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) { fprintf(stderr, "[FSM ASSERT] %s\n", msg); abort(); }
}
"#;
    fs::write(dir.join("host_hal.c"), hal).expect("write host_hal.c");
}

fn write_parallel_main_c(dir: &Path) {
    // Drives the parallel Motor IR: two regions, each with one Active →
    // Final transition. We expect both regions to advance independently,
    // and the per-region counts to remain at 2 until both reach Final
    // (which then triggers parallel completion).
    let main = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Motor.h"

/* Stub entries / exits — the IR carries some action-less leaves; we
 * keep these as no-ops since we're testing dispatch flow, not actions. */
void Motor_entry_IDLE(Motor_t *m)        { (void)m; }
void Motor_entry_RUNNING(Motor_t *m)     { (void)m; }
void Motor_entry_FAULTED(Motor_t *m)     { (void)m; }
void Motor_entry_MONITOR(Motor_t *m)     { (void)m; }
void Motor_entry_A_ACTIVE(Motor_t *m)    { (void)m; }
void Motor_entry_B_ACTIVE(Motor_t *m)    { (void)m; }
void Motor_exit_IDLE(Motor_t *m)         { (void)m; }
void Motor_exit_RUNNING(Motor_t *m)      { (void)m; }
void Motor_exit_FAULTED(Motor_t *m)      { (void)m; }
void Motor_exit_MONITOR(Motor_t *m)      { (void)m; }
void Motor_exit_A_ACTIVE(Motor_t *m)     { (void)m; }
void Motor_exit_B_ACTIVE(Motor_t *m)     { (void)m; }
void Motor_action_startMotor(Motor_t *m, const Motor_Event_t *ev) { (void)m; (void)ev; }
void Motor_action_stopMotor(Motor_t *m, const Motor_Event_t *ev)  { (void)m; (void)ev; }

int main(void) {
    Motor_t motor;
    Motor_init(&motor);

    /* Initial state for the parallel-Motor fixture is `Idle` (top-level);
     * we then need to enter `Monitor` via some event to test parallel
     * dispatch. But the fixture's transitions don't include such a path:
     * the parallel `Monitor` state is unreachable through events.
     *
     * Instead, manually force the machine into the parallel configuration
     * to exercise the dispatch loop. */
    motor._active[0] = MOTOR_STATE_A_ACTIVE;
    motor._active[1] = MOTOR_STATE_B_ACTIVE;
    motor._active_count = 2;

    /* Dispatch A_DONE — region A's transition fires; region B unchanged. */
    Motor_Event_t a_done = { .id = MOTOR_EVENT_A_DONE };
    Motor_dispatch(&motor, &a_done);
    if (motor._active[0] != MOTOR_STATE_AFINAL) {
        fprintf(stderr, "expected slot 0 = AFinal, got %d\n", motor._active[0]);
        return 1;
    }
    if (motor._active[1] != MOTOR_STATE_B_ACTIVE) {
        fprintf(stderr, "expected slot 1 = B_Active (unchanged), got %d\n", motor._active[1]);
        return 2;
    }
    if (motor._active_count != 2) {
        fprintf(stderr, "expected _active_count = 2, got %d\n", motor._active_count);
        return 3;
    }

    /* Dispatch B_DONE — region B's transition fires. Both regions are
     * now in Final. */
    Motor_Event_t b_done = { .id = MOTOR_EVENT_B_DONE };
    Motor_dispatch(&motor, &b_done);
    if (motor._active[0] != MOTOR_STATE_AFINAL) {
        fprintf(stderr, "expected slot 0 still = AFinal, got %d\n", motor._active[0]);
        return 4;
    }
    if (motor._active[1] != MOTOR_STATE_BFINAL) {
        fprintf(stderr, "expected slot 1 = BFinal, got %d\n", motor._active[1]);
        return 5;
    }

    /* All-regions-final completion check — both slots in final state. */
    if (motor._active_count != 2) {
        fprintf(stderr, "expected _active_count still = 2, got %d\n", motor._active_count);
        return 6;
    }

    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write main.c");
}

fn run_parallel_smoke(strategy: fsm_codegen_c::DispatchStrategy, label: &str) {
    if !gcc_available() {
        eprintln!("[parallel_dispatch_runs:{label}] gcc not on PATH — skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(&common::parallel_motor_ir(), &cfg).expect("emit");
    write_files(dir, &out);
    write_host_hal_c(dir);
    write_parallel_main_c(dir);

    let exe = dir.join(format!("parallel_test_{}", label));
    let result = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "Motor.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");

    if !result.status.success() || !result.stderr.is_empty() {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        eprintln!(
            "=== generated Motor.c ({}) ===\n{}",
            label,
            out.find("Motor.c").unwrap().content
        );
        eprintln!("=== gcc stdout ===\n{}", stdout);
        eprintln!("=== gcc stderr ===\n{}", stderr);
        panic!("gcc failed [{label}]: status={:?}", result.status.code());
    }

    let run = Command::new(&exe).output().expect("run parallel_test");
    if !run.status.success() {
        eprintln!(
            "=== generated Motor.c ({}) ===\n{}",
            label,
            out.find("Motor.c").unwrap().content
        );
        panic!(
            "parallel_test binary failed [{label}]: exit={:?}, stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        );
    }
}

#[test]
fn parallel_two_regions_dispatch_independently_switch() {
    run_parallel_smoke(fsm_codegen_c::DispatchStrategy::Switch, "switch");
}

#[test]
fn parallel_two_regions_dispatch_independently_table() {
    run_parallel_smoke(fsm_codegen_c::DispatchStrategy::Table, "table");
}
