//! End-to-end behavioural acceptance for `fsm generate --import-header`
//! (ROADMAP v1.1 W5, SUBAGENT_CONVENTIONS §5.4).
//!
//! Proves the WHOLE chain works against a REAL header's REAL impls:
//!   1. `fsm generate --import-header examples/import-header/driver.h
//!      examples/import-header/motor_uses_driver.fsm` — the `.fsm` has NO
//!      `extern`s; they come from the header.
//!   2. `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` compiles + links
//!      the generated C with the example's real `driver.c`.
//!   3. The binary is **executed**, driving an event sequence.
//!   4. Asserts the imported externs were genuinely invoked (observable via
//!      `g_driver`'s side-effect counters) AND the FSM took the expected
//!      transitions — i.e. an imported extern behaves exactly like a DSL
//!      `extern`.
//!
//! Run for BOTH dispatch strategies (`auto` default + `table`).
//!
//! FAIL-on-main: on `main` the binary rejects `--import-header` as an
//! unknown argument (clap), so `generate` never produces C and step 1's
//! `.assert().success()` fails. PASS-after: the externs import, compile,
//! link and run.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::Command as Assert;

/// Host `main` that drives the imported-extern motor through its lifecycle
/// and asserts the externs actually fired. Exit code 0 == all good; any
/// non-zero is a specific failure point.
const MAIN_C: &str = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Motor.h"
#include "driver_probe.h"

/* Entry/exit handlers the impl-header contract requires (no-ops here). */
void Motor_entry_IDLE(Motor_t *m)    { (void)m; }
void Motor_entry_RUNNING(Motor_t *m) { (void)m; }
void Motor_entry_FAULTED(Motor_t *m) { (void)m; }
void Motor_exit_IDLE(Motor_t *m)     { (void)m; }
void Motor_exit_RUNNING(Motor_t *m)  { (void)m; }
void Motor_exit_FAULTED(Motor_t *m)  { (void)m; }

int main(void) {
    Motor_t mtr;
    Motor_init(&mtr);

    if (mtr._active[0] != MOTOR_STATE_IDLE) {
        fprintf(stderr, "init: expected IDLE, got %d\n", mtr._active[0]);
        return 1;
    }

    /* START: Idle -> Running, action `ctx.starts += 1; driver_set_speed(1500)`.
     * `driver_set_speed` is an IMPORTED extern (from driver.h, not the .fsm). */
    Motor_Event_t start = { .id = MOTOR_EVENT_START };
    Motor_dispatch(&mtr, &start);
    if (mtr._active[0] != MOTOR_STATE_RUNNING) {
        fprintf(stderr, "START: expected RUNNING, got %d\n", mtr._active[0]);
        return 2;
    }
    if (mtr.context.starts != 1) {
        fprintf(stderr, "START: expected starts=1, got %u\n", mtr.context.starts);
        return 3;
    }
    if (g_driver.set_speed_calls != 1) {
        fprintf(stderr, "START: imported driver_set_speed NOT called (calls=%u)\n",
                g_driver.set_speed_calls);
        return 4;
    }
    if (g_driver.last_rpm != 1500) {
        fprintf(stderr, "START: driver_set_speed got rpm=%u, expected 1500\n",
                g_driver.last_rpm);
        return 5;
    }

    /* FAULT: Running -> Faulted, action `driver_clear_fault(false)`. */
    Motor_Event_t fault = { .id = MOTOR_EVENT_FAULT };
    Motor_dispatch(&mtr, &fault);
    if (mtr._active[0] != MOTOR_STATE_FAULTED) {
        fprintf(stderr, "FAULT: expected FAULTED, got %d\n", mtr._active[0]);
        return 6;
    }
    if (g_driver.clear_fault_calls != 1 || g_driver.last_clear_force != false) {
        fprintf(stderr, "FAULT: imported driver_clear_fault(false) not seen "
                "(calls=%u force=%d)\n",
                g_driver.clear_fault_calls, g_driver.last_clear_force);
        return 7;
    }

    /* RESET: Faulted -> Idle, action `driver_clear_fault(true)`. */
    Motor_Event_t reset = { .id = MOTOR_EVENT_RESET };
    Motor_dispatch(&mtr, &reset);
    if (mtr._active[0] != MOTOR_STATE_IDLE) {
        fprintf(stderr, "RESET: expected IDLE, got %d\n", mtr._active[0]);
        return 8;
    }
    if (g_driver.clear_fault_calls != 2 || g_driver.last_clear_force != true) {
        fprintf(stderr, "RESET: imported driver_clear_fault(true) not seen "
                "(calls=%u force=%d)\n",
                g_driver.clear_fault_calls, g_driver.last_clear_force);
        return 9;
    }

    /* STOP only fires from Running; from Idle it's a no-op (no extra
     * set_speed). Re-START then STOP to exercise `driver_set_speed(0)`. */
    Motor_dispatch(&mtr, &start);
    Motor_Event_t stop = { .id = MOTOR_EVENT_STOP };
    Motor_dispatch(&mtr, &stop);
    if (mtr._active[0] != MOTOR_STATE_IDLE) {
        fprintf(stderr, "STOP: expected IDLE, got %d\n", mtr._active[0]);
        return 10;
    }
    if (g_driver.set_speed_calls != 3 || g_driver.last_rpm != 0) {
        fprintf(stderr, "STOP: expected driver_set_speed(0) (calls=%u rpm=%u)\n",
                g_driver.set_speed_calls, g_driver.last_rpm);
        return 11;
    }

    printf("OK imported externs invoked: set_speed=%u clear_fault=%u\n",
           g_driver.set_speed_calls, g_driver.clear_fault_calls);
    return 0;
}
"#;

fn run_for_strategy(strategy: &str) {
    if common::should_skip_gcc("import_header_e2e") {
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let out_dir = tmp.path();

    let root = common::workspace_root();
    let header = root.join("examples/import-header/driver.h");
    let fsm = root.join("examples/import-header/motor_uses_driver.fsm");
    let driver_c = root.join("examples/import-header/driver.c");
    let probe_h = root.join("examples/import-header/driver_probe.h");
    for p in [&header, &fsm, &driver_c, &probe_h] {
        assert!(p.is_file(), "fixture missing: {}", p.display());
    }

    // 1. Generate, importing externs from the real header.
    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--strategy", strategy, "--import-header"])
        .arg(&header)
        .arg(&fsm)
        .arg("--out")
        .arg(out_dir)
        .assert()
        .success();

    for f in &[
        "fsm_hal.h",
        "Motor.h",
        "Motor.c",
        "Motor_impl.h",
        "Motor_conf.h",
    ] {
        assert!(out_dir.join(f).is_file(), "missing generated {f}");
    }

    // The imported externs MUST appear as bare prototypes in the impl
    // header — identical to a DSL-declared extern (cheap secondary check
    // alongside the behavioural run below, per §5.4).
    let impl_h = fs::read_to_string(out_dir.join("Motor_impl.h")).unwrap();
    for proto in &[
        "void driver_set_speed(uint16_t rpm);",
        "void driver_clear_fault(bool force);",
        "uint8_t driver_read_fault(void);",
        "bool driver_init(void);",
    ] {
        assert!(
            impl_h.contains(proto),
            "[{strategy}] impl header missing imported extern prototype `{proto}`\n--- impl.h ---\n{impl_h}"
        );
    }
    // The pointer-param function MUST have been skipped (resilience): no
    // broken `driver_get_stats` prototype with silently-dropped params.
    assert!(
        !impl_h.contains("driver_get_stats"),
        "[{strategy}] driver_get_stats should be skipped, not emitted"
    );

    // Bring the example's real driver.c + probe header into the build dir.
    fs::copy(&driver_c, out_dir.join("driver.c")).unwrap();
    fs::copy(&probe_h, out_dir.join("driver_probe.h")).unwrap();
    fs::write(out_dir.join("main.c"), MAIN_C).unwrap();
    // Stand-alone host HAL (mirrors vending_machine_gcc.rs): provide the
    // two HAL symbols directly rather than via the fsm_hal.h POSIX-reference
    // macro (which needs _POSIX_C_SOURCE plumbed into the right TU).
    fs::write(
        out_dir.join("host_hal.c"),
        r#"#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
uint32_t fsm_hal_clock_now_ms(void) { return 0; }
void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) { fprintf(stderr, "[FSM ASSERT] %s\n", msg); abort(); }
}
"#,
    )
    .unwrap();

    // 2. gcc compile + link (strict).
    let bin = out_dir.join("ih_app");
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
            "driver.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&bin)
        .output()
        .expect("gcc spawn");
    assert!(
        result.status.success(),
        "[{strategy}] gcc -Werror failed to compile/link generated C with the real driver.c\n\
         === gcc stderr ===\n{}\n=== gcc stdout ===\n{}",
        String::from_utf8_lossy(&result.stderr),
        String::from_utf8_lossy(&result.stdout),
    );

    // 3 + 4. Run the binary; non-zero exit pinpoints which extern/transition
    // assertion failed (see MAIN_C return codes).
    let out = Command::new(&bin).output().expect("run ih_app");
    assert!(
        out.status.success(),
        "[{strategy}] generated FSM run failed (code {:?}).\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("OK imported externs invoked"),
        "[{strategy}] expected success banner, got: {stdout}"
    );
}

#[test]
fn imported_externs_compile_and_run_default_auto_strategy() {
    run_for_strategy("auto");
}

#[test]
fn imported_externs_compile_and_run_table_strategy() {
    run_for_strategy("table");
}

/// The synthesized DSL `extern`s satisfy the analyzer's name-resolution
/// (`FSM-E0102`): a `.fsm` with zero `extern`s but calling imported
/// functions must `generate` cleanly. (Regression guard for the design
/// decision to splice DSL source pre-parse rather than inject post-IR.)
#[test]
fn fsm_with_no_externs_resolves_imported_calls() {
    let tmp = tempfile::tempdir().unwrap();
    let root = common::workspace_root();
    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg("--import-header")
        .arg(root.join("examples/import-header/driver.h"))
        .arg(root.join("examples/import-header/motor_uses_driver.fsm"))
        .arg("--out")
        .arg(tmp.path())
        .assert()
        .success();
    // No FSM-E0102 leaked to the generated tree.
    assert!(tmp.path().join("Motor.c").is_file());
}

/// Belt-and-braces: a path that does NOT exist must fail cleanly (exit 3),
/// never panic.
#[test]
fn missing_header_path_is_clean_error_not_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let root = common::workspace_root();
    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg("--import-header")
        .arg("/no/such/driver.h")
        .arg(root.join("examples/import-header/motor_uses_driver.fsm"))
        .arg("--out")
        .arg(tmp.path())
        .assert()
        .failure()
        .code(3);
}

/// Sanity that the example header still exists where docs/tests reference it.
#[test]
fn example_assets_present() {
    let root = common::workspace_root();
    for rel in &[
        "examples/import-header/driver.h",
        "examples/import-header/driver.c",
        "examples/import-header/driver_probe.h",
        "examples/import-header/motor_uses_driver.fsm",
        "examples/import-header/README.md",
    ] {
        assert!(
            Path::new(&root).join(rel).is_file(),
            "missing example asset {rel}"
        );
    }
}
