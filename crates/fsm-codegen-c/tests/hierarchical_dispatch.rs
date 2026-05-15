//! Hierarchical-dispatch test — Doc 00 §7.8 (B-10).
//!
//! Composite parent `Operational` declares `FAULT -> Error`. When the
//! active leaf is `Op.Running`, the dispatcher MUST walk up via the parent
//! table to find the composite's transition.
//!
//! ## Test strategy (v1.1-W0 / SUBAGENT_CONVENTIONS §5.4, PD-2)
//!
//! The §5.4 behavioural-acceptance guard is
//! [`fault_in_composite_leaf_walks_up_to_error_in_gcc_built_binary_switch`]
//! / `..._table`: each `fsm generate`s the hierarchical IR, compiles it
//! with `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, RUNS the binary,
//! and asserts the *observable* B-10 behaviour — `FAULT` dispatched while
//! the active leaf is `Op.Running` drives the machine to `Error` (proving
//! the generated dispatcher actually performs the leaf→root parent-table
//! walk). A broken or absent walk leaves the machine stuck in `Op.Running`
//! and the binary exits non-zero.
//!
//! The four tests after them are **secondary structural-invariant checks**,
//! retained per §5.4's last paragraph (symbol-presence is acceptable
//! *alongside* a real behavioural test, never as the sole guard). They pin
//! the specific B-10 emission shape — `Motor_parent_table`, the ROOT
//! sentinel, the `parent_table[0] == ROOT` terminator — so a refactor that
//! silently changes the parent-table representation is caught with a
//! precise message even before the slower gcc test runs. They are NOT the
//! behaviour guard; the gcc-RUN tests are.
//!
//! Until W0, B-10 had ONLY the symbol-presence checks and no behavioural
//! coverage — the exact P0-1-class gap PD-2 closes. Wiring the gcc test
//! also surfaced a latent duplicate-event defect in the shared
//! `hierarchical_motor_ir()` fixture (it double-declared FAULT, yielding a
//! duplicate C enumerator); see the note in `common/mod.rs`.

#![cfg(not(target_os = "windows"))]

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig, DispatchStrategy};

// ---------------------------------------------------------------------------
// §5.4 behavioural acceptance — the real B-10 guard.
// ---------------------------------------------------------------------------

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
    let hal = r#"#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>

uint32_t fsm_hal_clock_now_ms(void) { return 0; }

void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) { fprintf(stderr, "[FSM ASSERT] %s\n", msg); abort(); }
}
"#;
    fs::write(dir.join("host_hal.c"), hal).expect("write host_hal.c");
}

/// Driver: init lands in the composite `Operational` (inner initial =
/// `Op.Running`). Dispatching `FAULT` — an event NO leaf transition
/// consumes — must be resolved by walking leaf→root through the parent
/// table to `Operational`'s `FAULT -> Error`. Non-zero exit codes are
/// distinct failed assertions about that observable behaviour.
fn write_driver_main_c(dir: &Path) {
    let main = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Motor.h"

void Motor_entry_OPERATIONAL(Motor_t *m) { (void)m; }
void Motor_entry_OP_RUNNING(Motor_t *m)  { (void)m; }
void Motor_entry_ERROR(Motor_t *m)       { (void)m; }
void Motor_exit_OPERATIONAL(Motor_t *m)  { (void)m; }
void Motor_exit_OP_RUNNING(Motor_t *m)   { (void)m; }
void Motor_exit_ERROR(Motor_t *m)        { (void)m; }
void Motor_action_startMotor(Motor_t *m, const Motor_Event_t *ev) { (void)m; (void)ev; }
void Motor_action_stopMotor(Motor_t *m, const Motor_Event_t *ev)  { (void)m; (void)ev; }

int main(void) {
    Motor_t m;
    Motor_init(&m);

    /* Initial config: entering the `Operational` composite descends to its
     * inner initial, so the active LEAF is `Op.Running`. */
    if (Motor_current_state(&m) != MOTOR_STATE_OP_RUNNING) {
        fprintf(stderr, "init: expected Op.Running (composite descent), got %d\n",
                (int)Motor_current_state(&m));
        return 10;
    }

    /* FAULT is consumed by NO transition on the `Op.Running` leaf. B-10
     * (Doc 00 §7.8): the dispatcher must walk leaf -> root via the parent
     * table, find `Operational`'s `FAULT -> Error`, and take it. A broken
     * or missing ancestor walk leaves us stuck in Op.Running. */
    Motor_Event_t fault = { .id = MOTOR_EVENT_FAULT };
    Motor_dispatch(&m, &fault);
    if (Motor_current_state(&m) != MOTOR_STATE_ERROR) {
        fprintf(stderr,
                "B-10 leaf->root walk failed: FAULT in Op.Running did NOT "
                "reach Error (got %d). The parent-table ancestor walk is "
                "broken or absent.\n",
                (int)Motor_current_state(&m));
        return 11;
    }
    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write main.c");
}

fn run_b10_acceptance(strategy: DispatchStrategy, label: &str) {
    if !gcc_available() {
        eprintln!("[hierarchical_dispatch:{label}] gcc not on PATH — skipping");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(&common::hierarchical_motor_ir(), &cfg).expect("emit hierarchical C");
    write_files(dir, &out);
    write_host_hal_c(dir);
    write_driver_main_c(dir);

    let exe = dir.join(format!("hier_test_{label}"));
    let build = Command::new("gcc")
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
    if !build.status.success() || !build.stderr.is_empty() {
        eprintln!(
            "=== generated Motor.c ({label}) ===\n{}",
            out.find("Motor.c").unwrap().content
        );
        eprintln!(
            "=== generated Motor.h ({label}) ===\n{}",
            out.find("Motor.h").unwrap().content
        );
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&build.stderr)
        );
        panic!("gcc failed [{label}]: status={:?}", build.status.code());
    }
    let run = Command::new(&exe).output().expect("run hier_test");
    if !run.status.success() {
        eprintln!(
            "=== generated Motor.c ({label}) ===\n{}",
            out.find("Motor.c").unwrap().content
        );
        panic!(
            "B-10 behavioural test FAILED [{label}]: exit={:?} (see assertion \
             codes in main.c), stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}

#[test]
fn fault_in_composite_leaf_walks_up_to_error_in_gcc_built_binary_switch() {
    run_b10_acceptance(DispatchStrategy::Switch, "switch");
}

#[test]
fn fault_in_composite_leaf_walks_up_to_error_in_gcc_built_binary_table() {
    run_b10_acceptance(DispatchStrategy::Table, "table");
}

// ---------------------------------------------------------------------------
// Secondary structural-invariant checks (§5.4 last paragraph). These guard
// the B-10 *emission shape*, not behaviour — the gcc-RUN tests above are the
// behaviour guard. Kept because the parent-table representation is a
// deliberate, stable codegen contract (Doc 00 §7.8) and a precise
// "parent_table changed" failure localizes a refactor regression faster
// than decoding a non-zero exit code.
// ---------------------------------------------------------------------------

#[test]
fn parent_table_emitted_for_hierarchical_machine() {
    let out = emit(&common::hierarchical_motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // Structural invariant only — behavioural correctness of the walk is
    // proven by the gcc-RUN tests above; this pins the representation.
    assert!(
        c.contains("Motor_parent_table"),
        "Motor_parent_table missing in generated source"
    );
}

#[test]
fn dispatch_walks_ancestors_via_parent_table() {
    let out = emit(&common::hierarchical_motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // Structural: the leaf-to-root loop indexes the parent table and
    // terminates at the ROOT sentinel. (Behaviour proven above.)
    assert!(
        c.contains("Motor_parent_table[s]"),
        "ancestor walk missing: should index parent table by current state"
    );
    assert!(
        c.contains("MOTOR_STATE_ROOT"),
        "ROOT terminator missing — the loop must compare against ROOT"
    );
}

#[test]
fn composite_transition_appears_in_per_state_try() {
    let out = emit(&common::hierarchical_motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // Structural: the composite's FAULT -> Error transition must be
    // reachable from the per-state try switch. (Behaviour proven above.)
    assert!(
        c.contains("MOTOR_STATE_OPERATIONAL"),
        "composite state id missing"
    );
    assert!(
        c.contains("MOTOR_STATE_ERROR"),
        "target state for composite transition missing"
    );
    assert!(
        c.contains("MOTOR_EVENT_FAULT"),
        "FAULT event must be present in dispatch"
    );
}

#[test]
fn parent_table_root_entry_is_root_sentinel() {
    let out = emit(&common::hierarchical_motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // Structural invariant: parent_table[0] is the ROOT self-sentinel that
    // terminates the ancestor loop. (Behaviour proven above.)
    assert!(
        c.contains("[0] = MOTOR_STATE_ROOT"),
        "parent_table[0] must be ROOT sentinel"
    );
}
