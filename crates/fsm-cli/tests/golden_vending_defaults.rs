//! Audit 2026-05-14 — end-to-end golden test: run `fsm generate` on the
//! vending-machine example and confirm the generated C99 initialises the
//! declared `balance: u16 = 0` and `price: u16 = 150` context defaults.
//!
//! Complements the targeted unit tests in `fsm-codegen-c` (which build
//! synthetic IR) with a real-world example fixture, proving the fix
//! holds through the full parse → analyze → emit pipeline.

#[path = "common/mod.rs"]
mod common;

use std::fs;

use assert_cmd::Command as Assert;

#[test]
fn fsm_generate_emits_context_defaults_for_vending_machine() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out_dir = tmp.path();

    let src = common::workspace_root().join("examples/vending-machine/vending-machine.fsm");
    assert!(src.is_file(), "fixture missing at {}", src.display());

    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--out"])
        .arg(out_dir)
        .arg(&src)
        .assert()
        .success();

    let vm_c =
        fs::read_to_string(out_dir.join("VendingMachine.c")).expect("VendingMachine.c emitted");

    // The two DSL declarations:
    //   balance: u16 = 0
    //   price  : u16 = 150
    // Both must surface as explicit assignments in `VendingMachine_init`.
    assert!(
        vm_c.contains("m->context.balance = 0;"),
        "VendingMachine.c must initialise balance from DSL default;\n--- VendingMachine.c ---\n{}",
        vm_c,
    );
    assert!(
        vm_c.contains("m->context.price = 150;"),
        "VendingMachine.c must initialise price from DSL default;\n--- VendingMachine.c ---\n{}",
        vm_c,
    );
}

#[test]
fn fsm_generate_emits_done_autofire_for_vending_machine() {
    // Same fixture also exercises the `done -> Target` auto-fire on
    // non-final states (ChangeAvailable and Dispensing both have
    // `done ->` clauses).
    let tmp = tempfile::tempdir().expect("tempdir");
    let out_dir = tmp.path();

    let src = common::workspace_root().join("examples/vending-machine/vending-machine.fsm");
    assert!(src.is_file(), "fixture missing at {}", src.display());

    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--out"])
        .arg(out_dir)
        .arg(&src)
        .assert()
        .success();

    let vm_c =
        fs::read_to_string(out_dir.join("VendingMachine.c")).expect("VendingMachine.c emitted");

    // The `Motor_handle_completion` body must include cases for both
    // non-final states with `done -> ...` transitions.
    assert!(
        vm_c.contains("case VENDINGMACHINE_STATE_CHANGEAVAILABLE:"),
        "VendingMachine.c must include ChangeAvailable in handle_completion (auto-fire case);\n--- VendingMachine.c ---\n{}",
        vm_c,
    );
    assert!(
        vm_c.contains("case VENDINGMACHINE_STATE_DISPENSING:"),
        "VendingMachine.c must include Dispensing in handle_completion (auto-fire case);\n--- VendingMachine.c ---\n{}",
        vm_c,
    );
}
