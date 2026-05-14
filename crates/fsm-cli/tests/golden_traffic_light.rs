//! End-to-end test: parse → analyze → emit C99 for the
//! `examples/traffic-light/traffic-light.fsm` example (which uses HSM + a
//! shallow_history pseudostate), then gcc-compile and dispatch the manual
//! events to confirm the OVERRIDE / RESUME transitions reach the expected
//! states.
//!
//! This completes the P1-3 expansion of the gcc gate from Motor-only to all
//! three shipped examples. The `after N ms` timer paths inside `Auto` are
//! intentionally NOT exercised here because P0-4 (timer arming on entry) is
//! a separate, in-flight fix; this test covers the parts of the example
//! orthogonal to that timer behaviour.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::process::Command;

use assert_cmd::Command as Assert;

#[test]
fn traffic_light_compiles_and_executes_manual_overrides() {
    if common::should_skip_gcc("golden_traffic_light") {
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let out_dir = tmp.path();

    let src = common::workspace_root().join("examples/traffic-light/traffic-light.fsm");
    assert!(src.is_file(), "fixture missing at {}", src.display());

    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--out"])
        .arg(out_dir)
        .arg(&src)
        .assert()
        .success();

    // Sanity: the codegen wrote the expected files.
    for f in &[
        "fsm_hal.h",
        "TrafficLight.h",
        "TrafficLight.c",
        "TrafficLight_conf.h",
        "TrafficLight_impl.h",
    ] {
        assert!(out_dir.join(f).is_file(), "missing generated file {f}");
    }

    // Stand-alone HAL implementation. We don't exercise the timer paths in
    // this test (the `after N ms` arming behaviour is P0-4 work-in-progress)
    // so `fsm_hal_clock_now_ms` returns a fixed 0 — enough for the codegen
    // to link without dragging in clock_gettime.
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

    // Driver: TrafficLight has no externs, only entry/exit prototypes. We
    // stub each one and walk init → OVERRIDE → Manual → RESUME → back into
    // the Auto region (the shallow_history target lands on Red, the
    // outer-Auto initial). The exact leaf state after RESUME is not asserted
    // because the history-restore behaviour depends on prior in-region
    // execution; we only check that we are no longer in Manual.
    let main_c = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "TrafficLight.h"

void TrafficLight_entry_AUTO(TrafficLight_t *m)               { (void)m; }
void TrafficLight_entry_RED(TrafficLight_t *m)                { (void)m; }
void TrafficLight_entry_GREENACCELERATING(TrafficLight_t *m)  { (void)m; }
void TrafficLight_entry_GREEN(TrafficLight_t *m)              { (void)m; }
void TrafficLight_entry_YELLOW(TrafficLight_t *m)             { (void)m; }
void TrafficLight_entry_MANUAL(TrafficLight_t *m)             { (void)m; }
void TrafficLight_exit_AUTO(TrafficLight_t *m)                { (void)m; }
void TrafficLight_exit_RED(TrafficLight_t *m)                 { (void)m; }
void TrafficLight_exit_GREENACCELERATING(TrafficLight_t *m)   { (void)m; }
void TrafficLight_exit_GREEN(TrafficLight_t *m)               { (void)m; }
void TrafficLight_exit_YELLOW(TrafficLight_t *m)              { (void)m; }
void TrafficLight_exit_MANUAL(TrafficLight_t *m)              { (void)m; }

int main(void) {
    TrafficLight_t tl;
    TrafficLight_init(&tl);

    /* Initial leaf is RED (Auto initial). */
    if (TrafficLight_current_state(&tl) != TRAFFICLIGHT_STATE_RED) {
        fprintf(stderr, "init: expected RED, got %d\n", TrafficLight_current_state(&tl));
        return 1;
    }

    /* OVERRIDE → Manual. */
    TrafficLight_Event_t over = { .id = TRAFFICLIGHT_EVENT_OVERRIDE };
    TrafficLight_dispatch(&tl, &over);
    if (TrafficLight_current_state(&tl) != TRAFFICLIGHT_STATE_MANUAL) {
        fprintf(stderr, "OVERRIDE: expected MANUAL, got %d\n", TrafficLight_current_state(&tl));
        return 2;
    }

    /* RESUME → exits Manual back into the Auto sub-region. The shallow
     * history pseudostate restores the in-Auto leaf — for a freshly-init'd
     * machine that's the Auto initial (Red). */
    TrafficLight_Event_t resume = { .id = TRAFFICLIGHT_EVENT_RESUME };
    TrafficLight_dispatch(&tl, &resume);
    if (TrafficLight_current_state(&tl) == TRAFFICLIGHT_STATE_MANUAL) {
        fprintf(stderr, "RESUME: still in MANUAL after dispatch\n");
        return 3;
    }

    return 0;
}
"#;
    fs::write(out_dir.join("main.c"), main_c).unwrap();

    let exe = out_dir.join("tl_test");
    let result = Command::new("gcc")
        .current_dir(out_dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "TrafficLight.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");

    if !result.status.success() || !result.stderr.is_empty() {
        let tl_c = fs::read_to_string(out_dir.join("TrafficLight.c")).unwrap_or_default();
        let tl_h = fs::read_to_string(out_dir.join("TrafficLight.h")).unwrap_or_default();
        eprintln!("=== generated TrafficLight.c ===\n{}", tl_c);
        eprintln!("=== generated TrafficLight.h ===\n{}", tl_h);
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

    let run = Command::new(&exe).output().expect("run tl_test");
    if !run.status.success() {
        let tl_c = fs::read_to_string(out_dir.join("TrafficLight.c")).unwrap_or_default();
        eprintln!("=== generated TrafficLight.c ===\n{}", tl_c);
        panic!(
            "tl_test binary failed: exit={:?}, stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}
