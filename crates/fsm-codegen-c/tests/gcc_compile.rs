//! End-to-end gcc compilation test — Doc 11 §1, gate G5.
//!
//! Emits the Motor IR, writes the generated files + a minimal host HAL +
//! a tiny `main.c`, and invokes
//! `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`. The test passes iff
//! gcc produces an executable with no warnings on stderr.
//!
//! If gcc is unavailable on PATH the test prints a skip notice and exits
//! successfully — this keeps `cargo test` green on minimal CI runners
//! while still being a hard gate on developer machines and the regular
//! pipeline.

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
        let p = dir.join(&f.path);
        fs::write(&p, &f.content).expect("write generated file");
    }
}

fn write_host_hal_c(dir: &Path) {
    // Stand-alone HAL implementation that satisfies the contract. Embedded
    // targets ship something equivalent; for host tests we use a counter
    // clock + abort()-on-assert.
    let hal = r#"#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

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
"#;
    fs::write(dir.join("host_hal.c"), hal).expect("write host_hal.c");
}

fn write_main_c(dir: &Path) {
    let main = r#"#include "fsm_hal.h"
#include "Motor.h"

void Motor_entry_IDLE(Motor_t *m)      { (void)m; }
void Motor_entry_RUNNING(Motor_t *m)   { (void)m; }
void Motor_entry_FAULTED(Motor_t *m)   { (void)m; }
void Motor_exit_IDLE(Motor_t *m)       { (void)m; }
void Motor_exit_RUNNING(Motor_t *m)    { (void)m; }
void Motor_exit_FAULTED(Motor_t *m)    { (void)m; }
void Motor_action_startMotor(Motor_t *m, const Motor_Event_t *ev) { (void)m; (void)ev; }
void Motor_action_stopMotor(Motor_t *m, const Motor_Event_t *ev)  { (void)m; (void)ev; }

int main(void) {
    Motor_t motor;
    Motor_init(&motor);
    if (Motor_current_state(&motor) != MOTOR_STATE_IDLE) return 1;

    Motor_Event_t start;
    start.id = MOTOR_EVENT_START;
    Motor_dispatch(&motor, &start);
    if (Motor_current_state(&motor) != MOTOR_STATE_RUNNING) return 2;

    Motor_Event_t stop;
    stop.id = MOTOR_EVENT_STOP;
    Motor_dispatch(&motor, &stop);
    if (Motor_current_state(&motor) != MOTOR_STATE_IDLE) return 3;

    Motor_advance_clock(&motor, 100u);
    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write main.c");
}

#[test]
fn motor_compiles_with_gcc_werror() {
    if !gcc_available() {
        eprintln!("[gcc_compile] gcc not on PATH — skipping (cargo test still passes)");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let out = emit(&common::motor_ir(), &CodegenConfig::default()).expect("emit");
    write_files(dir, &out);
    write_host_hal_c(dir);
    write_main_c(dir);

    let exe = dir.join("motor_test");
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
            "=== generated Motor.c ===\n{}",
            out.find("Motor.c").unwrap().content
        );
        eprintln!(
            "=== generated Motor.h ===\n{}",
            out.find("Motor.h").unwrap().content
        );
        eprintln!("=== gcc stdout ===\n{}", stdout);
        eprintln!("=== gcc stderr ===\n{}", stderr);
        panic!(
            "gcc failed (status: {:?}); see stderr above",
            result.status.code()
        );
    }

    // Now run the executable — exit 0 means the runtime is behaving as
    // expected end-to-end.
    let run = Command::new(&exe).output().expect("run motor_test binary");
    assert!(
        run.status.success(),
        "motor_test binary failed: exit={:?}, stderr={}",
        run.status.code(),
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn hal_header_compiles_in_isolation() {
    if !gcc_available() {
        eprintln!("[gcc_compile] gcc not on PATH — skipping (cargo test still passes)");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    // Write only the HAL header and a tiny .c file that defines the
    // contract. Validates the HAL header is standalone-compilable.
    let out = emit(&common::motor_ir(), &CodegenConfig::default()).unwrap();
    let hal = out.find("fsm_hal.h").unwrap();
    fs::write(dir.join("fsm_hal.h"), &hal.content).unwrap();

    let stub = r#"#define FSM_HAL_PROVIDE_POSIX_REFERENCE
#include "fsm_hal.h"
int main(void) { return (int)fsm_hal_clock_now_ms() == 0 ? 0 : 0; }
"#;
    fs::write(dir.join("hal_smoke.c"), stub).unwrap();

    let exe = dir.join("hal_smoke");
    let result = Command::new("gcc")
        .current_dir(dir)
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
        .arg(&exe)
        .output()
        .expect("invoke gcc");
    assert!(
        result.status.success() && result.stderr.is_empty(),
        "gcc rejected fsm_hal.h\nstatus: {:?}\nstderr: {}",
        result.status.code(),
        String::from_utf8_lossy(&result.stderr),
    );
}
