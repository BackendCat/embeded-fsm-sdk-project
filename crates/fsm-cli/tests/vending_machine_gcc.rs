//! End-to-end test: parse → analyze → emit C99 for the
//! `examples/vending-machine/vending-machine.fsm` example (which uses
//! two parallel regions), then gcc-compile and drive the binary to
//! exercise both regions independently. Validates P0-2 + P0-3 from
//! `docs/AUDIT_2026_05_14.md`.

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
    // crates/fsm-cli/ → repo root.
    p.pop();
    p.pop();
    p
}

#[test]
fn vending_machine_compiles_and_drives_both_regions() {
    if !gcc_available() {
        eprintln!("[vending_machine_gcc] gcc not on PATH — skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let out_dir = tmp.path();

    let src = workspace_root().join("examples/vending-machine/vending-machine.fsm");
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
        "VendingMachine.h",
        "VendingMachine.c",
        "VendingMachine_conf.h",
        "VendingMachine_impl.h",
    ] {
        assert!(out_dir.join(f).is_file(), "missing generated file {f}");
    }

    // Stand-alone HAL implementation.
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

    // Main: drive every region through its lifecycle in lock-step. After
    // payment + selection both reach Final, dispatch RESET to exit the
    // parallel state and end at `Done`.
    let main_c = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "VendingMachine.h"

/* Stub entry / exit handlers + the dispense_item extern. */
void VendingMachine_entry_IDLE(VendingMachine_t *m)               { (void)m; }
void VendingMachine_entry_COININSERTED(VendingMachine_t *m)       { (void)m; }
void VendingMachine_entry_CHANGEAVAILABLE(VendingMachine_t *m)    { (void)m; }
void VendingMachine_entry_BROWSING(VendingMachine_t *m)           { (void)m; }
void VendingMachine_entry_SELECTED(VendingMachine_t *m)           { (void)m; }
void VendingMachine_entry_DISPENSING(VendingMachine_t *m)         { (void)m; }
void VendingMachine_entry_OPERATIONAL(VendingMachine_t *m)        { (void)m; }
void VendingMachine_entry_DONE(VendingMachine_t *m)               { (void)m; }
void VendingMachine_exit_IDLE(VendingMachine_t *m)                { (void)m; }
void VendingMachine_exit_COININSERTED(VendingMachine_t *m)        { (void)m; }
void VendingMachine_exit_CHANGEAVAILABLE(VendingMachine_t *m)     { (void)m; }
void VendingMachine_exit_BROWSING(VendingMachine_t *m)            { (void)m; }
void VendingMachine_exit_SELECTED(VendingMachine_t *m)            { (void)m; }
void VendingMachine_exit_DISPENSING(VendingMachine_t *m)          { (void)m; }
void VendingMachine_exit_OPERATIONAL(VendingMachine_t *m)         { (void)m; }
void VendingMachine_exit_DONE(VendingMachine_t *m)                { (void)m; }
void dispense_item(void)                                          { /* no-op */ }

int main(void) {
    VendingMachine_t vm;
    VendingMachine_init(&vm);
    /* v1.0 codegen does not yet apply DSL context defaults (out of
     * scope for the P0-2 + P0-3 audit). Set `price` manually to match
     * the DSL declaration `price : u16 = 150`. */
    vm.context.price = 150;

    /* Initial state expands the parallel `Operational` into both
     * regions. Payment region → Idle (slot 0), Selection region →
     * Browsing (slot 1). */
    if (vm._active_count != 2) {
        fprintf(stderr, "init: expected _active_count=2, got %d\n", vm._active_count);
        return 1;
    }
    if (vm._active[0] != VENDINGMACHINE_STATE_IDLE) {
        fprintf(stderr, "init: expected slot 0 = IDLE, got %d\n", vm._active[0]);
        return 2;
    }
    if (vm._active[1] != VENDINGMACHINE_STATE_BROWSING) {
        fprintf(stderr, "init: expected slot 1 = BROWSING, got %d\n", vm._active[1]);
        return 3;
    }

    /* Dispatch COIN(50): Payment region transitions Idle → CoinInserted.
     * Selection region is untouched. */
    VendingMachine_Event_t coin = { .id = VENDINGMACHINE_EVENT_COIN };
    coin.__payload.COIN.value = 50;
    VendingMachine_dispatch(&vm, &coin);
    if (vm._active[0] != VENDINGMACHINE_STATE_COININSERTED) {
        fprintf(stderr, "COIN: expected slot 0 = COININSERTED, got %d\n", vm._active[0]);
        return 4;
    }
    if (vm._active[1] != VENDINGMACHINE_STATE_BROWSING) {
        fprintf(stderr, "COIN: expected slot 1 unchanged (BROWSING), got %d\n", vm._active[1]);
        return 5;
    }
    if (vm.context.balance != 50) {
        fprintf(stderr, "COIN: expected balance = 50, got %u\n", vm.context.balance);
        return 6;
    }

    /* Dispatch COIN(150) to top up: Payment region self-transitions
     * (CoinInserted → CoinInserted) accumulating balance. */
    coin.__payload.COIN.value = 150;
    VendingMachine_dispatch(&vm, &coin);
    if (vm.context.balance != 200) {
        fprintf(stderr, "COIN(150): expected balance = 200, got %u\n", vm.context.balance);
        return 7;
    }
    if (vm._active[0] != VENDINGMACHINE_STATE_COININSERTED) {
        fprintf(stderr, "COIN(150): expected slot 0 still = COININSERTED, got %d\n", vm._active[0]);
        return 27;
    }

    /* Dispatch SELECT: Selection region transitions Browsing → Selected.
     * Payment region untouched. */
    VendingMachine_Event_t select = { .id = VENDINGMACHINE_EVENT_SELECT };
    VendingMachine_dispatch(&vm, &select);
    if (vm._active[0] != VENDINGMACHINE_STATE_COININSERTED) {
        fprintf(stderr, "SELECT: expected slot 0 unchanged (COININSERTED), got %d\n", vm._active[0]);
        return 8;
    }
    if (vm._active[1] != VENDINGMACHINE_STATE_SELECTED) {
        fprintf(stderr, "SELECT: expected slot 1 = SELECTED, got %d\n", vm._active[1]);
        return 9;
    }

    /* Dispatch DISPENSE: both regions can transition on this event.
     * Payment: CoinInserted → ChangeAvailable (balance >= price);
     * Selection: Selected → Dispensing. */
    VendingMachine_Event_t dispense = { .id = VENDINGMACHINE_EVENT_DISPENSE };
    VendingMachine_dispatch(&vm, &dispense);
    if (vm._active[0] != VENDINGMACHINE_STATE_CHANGEAVAILABLE) {
        fprintf(stderr, "DISPENSE: expected slot 0 = CHANGEAVAILABLE, got %d\n", vm._active[0]);
        return 10;
    }
    if (vm._active[1] != VENDINGMACHINE_STATE_DISPENSING) {
        fprintf(stderr, "DISPENSE: expected slot 1 = DISPENSING, got %d\n", vm._active[1]);
        return 11;
    }
    /* Balance reduced by price (200 - 150 = 50). */
    if (vm.context.balance != 50) {
        fprintf(stderr, "DISPENSE: expected balance = 50, got %u\n", vm.context.balance);
        return 12;
    }

    /* The `done` transitions on ChangeAvailable / Dispensing are
     * synthesised as completion events. v1.0 codegen does not yet
     * auto-fire those for non-Final states (out of scope for the P0-2
     * + P0-3 audit fix), so we dispatch the completion event manually
     * to drive each region into its Final state. */
    VendingMachine_Event_t comp = { .id = VENDINGMACHINE_EVENT__COMPLETION };
    VendingMachine_dispatch(&vm, &comp);
    if (vm._active[0] != VENDINGMACHINE_STATE_PAYMENTFINAL) {
        fprintf(stderr, "COMP1: expected slot 0 = PAYMENTFINAL, got %d\n", vm._active[0]);
        return 20;
    }
    if (vm._active[1] != VENDINGMACHINE_STATE_SELECTIONFINAL) {
        fprintf(stderr, "COMP1: expected slot 1 = SELECTIONFINAL, got %d\n", vm._active[1]);
        return 21;
    }

    /* Dispatch RESET: parallel's own transition fires, exiting both
     * regions and landing in Done. */
    VendingMachine_Event_t reset = { .id = VENDINGMACHINE_EVENT_RESET };
    VendingMachine_dispatch(&vm, &reset);
    if (vm._active_count != 1) {
        fprintf(stderr, "RESET: expected _active_count = 1, got %d\n", vm._active_count);
        return 13;
    }
    if (vm._active[0] != VENDINGMACHINE_STATE_DONE) {
        fprintf(stderr, "RESET: expected slot 0 = DONE, got %d\n", vm._active[0]);
        return 14;
    }
    /* Region slot 1 must have been cleared. */
    if (vm._active[1] != 0) {
        fprintf(stderr, "RESET: expected slot 1 cleared, got %d\n", vm._active[1]);
        return 15;
    }

    return 0;
}
"#;
    fs::write(out_dir.join("main.c"), main_c).unwrap();

    let exe = out_dir.join("vm_test");
    let result = Command::new("gcc")
        .current_dir(out_dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "VendingMachine.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");

    if !result.status.success() || !result.stderr.is_empty() {
        let vm_c = fs::read_to_string(out_dir.join("VendingMachine.c")).unwrap_or_default();
        let vm_h = fs::read_to_string(out_dir.join("VendingMachine.h")).unwrap_or_default();
        eprintln!("=== generated VendingMachine.c ===\n{}", vm_c);
        eprintln!("=== generated VendingMachine.h ===\n{}", vm_h);
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

    let run = Command::new(&exe).output().expect("run vm_test");
    if !run.status.success() {
        let vm_c = fs::read_to_string(out_dir.join("VendingMachine.c")).unwrap_or_default();
        eprintln!("=== generated VendingMachine.c ===\n{}", vm_c);
        panic!(
            "vm_test binary failed: exit={:?}, stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}
