//! End-to-end test: parse → analyze → emit C99 for the
//! `examples/motor/motor.fsm` example, then gcc-compile and drive the
//! binary to verify P0-4 (timer-arm-on-entry + distinct timer event id).
//!
//! Scenario: `state Running { after 5000 ms -> Faulted; ... }`. We START
//! the motor (Idle → Running, arming the timer), then advance the clock
//! 5500ms WITHOUT dispatching any other event. The machine MUST reach
//! Faulted purely on the timer's fire. Pre-fix this test failed because
//! the timer slot was never armed when entering Running.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use assert_cmd::Command as Assert;

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

#[test]
fn motor_after_timer_drives_running_to_faulted_via_clock_only() {
    if !gcc_available() {
        eprintln!("[motor_timer_e2e] gcc not on PATH — skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let out_dir = tmp.path();

    let src = workspace_root().join("examples/motor/motor.fsm");
    assert!(src.is_file(), "fixture missing at {}", src.display());

    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--out"])
        .arg(out_dir)
        .arg(&src)
        .assert()
        .success();

    for f in &[
        "fsm_hal.h",
        "Motor.h",
        "Motor.c",
        "Motor_conf.h",
        "Motor_impl.h",
    ] {
        assert!(out_dir.join(f).is_file(), "missing generated file {f}");
    }

    // The header MUST contain a distinct per-timer event variant — pre-fix
    // codegen routed the timer through MOTOR_EVENT__COMPLETION, colliding
    // with any `done` transition in the same state.
    let header_content = fs::read_to_string(out_dir.join("Motor.h")).expect("read Motor.h");
    assert!(
        header_content.contains("MOTOR_EVENT_TIMER_") && header_content.contains("_FIRED"),
        "expected MOTOR_EVENT_TIMER_<id>_FIRED variant in Motor.h, got:\n{header_content}"
    );

    let source_content = fs::read_to_string(out_dir.join("Motor.c")).expect("read Motor.c");
    assert!(
        source_content.contains("fsm_hal_clock_now_ms"),
        "Motor.c should reference fsm_hal_clock_now_ms"
    );
    // The Running entry path must arm the timer slot.
    assert!(
        source_content.contains("/* P0-4: arm timer on entry */"),
        "Motor.c should arm timers on entry per P0-4"
    );

    let hal_c = r#"#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>

uint32_t fsm_hal_clock_now_ms(void) { return 0; }

void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) { fprintf(stderr, "[FSM ASSERT] %s\n", msg); abort(); }
}
"#;
    fs::write(out_dir.join("host_hal.c"), hal_c).unwrap();

    let main_c = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Motor.h"

bool can_start(void) { return true; }
void set_speed(uint16_t rpm) { (void)rpm; }
void reset_link(void) { }

void Motor_entry_IDLE(Motor_t *m)     { (void)m; }
void Motor_entry_RUNNING(Motor_t *m)  { (void)m; }
void Motor_entry_FAULTED(Motor_t *m)  { (void)m; }
void Motor_exit_IDLE(Motor_t *m)      { (void)m; }
void Motor_exit_RUNNING(Motor_t *m)   { (void)m; }
void Motor_exit_FAULTED(Motor_t *m)   { (void)m; }

int main(void) {
    Motor_t m;
    Motor_init(&m);
    if (Motor_current_state(&m) != MOTOR_STATE_IDLE) {
        fprintf(stderr, "init: expected IDLE, got %d\n", (int)Motor_current_state(&m));
        return 41;
    }

    /* START arms the timer (entry to Running). */
    Motor_Event_t start = { .id = MOTOR_EVENT_START };
    Motor_dispatch(&m, &start);
    if (Motor_current_state(&m) != MOTOR_STATE_RUNNING) {
        fprintf(stderr, "post-START: expected RUNNING, got %d\n", (int)Motor_current_state(&m));
        return 42;
    }

    /* 4999ms: still under deadline, MUST NOT fire. */
    Motor_advance_clock(&m, 4999u);
    if (Motor_current_state(&m) != MOTOR_STATE_RUNNING) {
        fprintf(stderr, "@4999ms: expected RUNNING, got %d\n", (int)Motor_current_state(&m));
        return 43;
    }

    /* +1ms = 5000ms: timer fires, machine MUST end in Faulted. No event
     * dispatched here other than the timer's own internal fire. */
    Motor_advance_clock(&m, 1u);
    if (Motor_current_state(&m) != MOTOR_STATE_FAULTED) {
        fprintf(stderr, "@5000ms: expected FAULTED, got %d\n", (int)Motor_current_state(&m));
        return 44;
    }
    return 0;
}
"#;
    fs::write(out_dir.join("main.c"), main_c).unwrap();

    let exe = out_dir.join("motor_timer_test");
    let result = Command::new("gcc")
        .current_dir(out_dir)
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
        eprintln!("=== generated Motor.c ===\n{source_content}");
        eprintln!("=== generated Motor.h ===\n{header_content}");
        eprintln!(
            "=== gcc stdout ===\n{}",
            String::from_utf8_lossy(&result.stdout)
        );
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
        panic!("gcc failed (status: {:?})", result.status.code());
    }

    let run = Command::new(&exe).output().expect("run motor_timer_test");
    if !run.status.success() {
        eprintln!("=== generated Motor.c ===\n{source_content}");
        panic!(
            "motor_timer_test binary failed: exit={:?}, stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}
