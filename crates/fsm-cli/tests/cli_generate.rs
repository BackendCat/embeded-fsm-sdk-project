//! Integration tests for `fsm generate`.
//!
//! Doc 23 §9 "What Done Looks Like":
//!   `fsm generate --target c99 motor.fsm` → Motor.{h,c}, Motor_impl.h,
//!   Motor_conf.h in `generated/`.

use std::fs;

use assert_cmd::Command;

const VALID_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/motor.fsm");

#[test]
fn emits_all_four_motor_files_plus_hal() {
    let td = tempfile::tempdir().unwrap();
    let out = td.path();

    Command::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--out"])
        .arg(out)
        .arg(VALID_FIXTURE)
        .assert()
        .success();

    for expected in &[
        "fsm_hal.h",
        "Motor.h",
        "Motor.c",
        "Motor_impl.h",
        "Motor_conf.h",
    ] {
        assert!(
            out.join(expected).is_file(),
            "expected emitted file {} under {}, got entries: {:?}",
            expected,
            out.display(),
            fs::read_dir(out)
                .unwrap()
                .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn unknown_target_exits_two() {
    let td = tempfile::tempdir().unwrap();
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "cpp17", "--out"])
        .arg(td.path())
        .arg(VALID_FIXTURE)
        .assert()
        .failure()
        .code(2);
}

#[test]
fn report_memory_emits_budget_line() {
    let td = tempfile::tempdir().unwrap();
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--report-memory", "--out"])
        .arg(td.path())
        .arg(VALID_FIXTURE)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    assert!(
        stdout.contains("Memory budget"),
        "expected memory-budget header in stdout, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("Motor"),
        "expected machine name in budget output, got:\n{}",
        stdout
    );
}

#[test]
fn license_flag_propagates_into_emitted_files() {
    let td = tempfile::tempdir().unwrap();
    Command::cargo_bin("fsm")
        .unwrap()
        .args([
            "generate",
            "--target",
            "c99",
            "--license",
            "Apache-2.0",
            "--out",
        ])
        .arg(td.path())
        .arg(VALID_FIXTURE)
        .assert()
        .success();
    let header = fs::read_to_string(td.path().join("Motor.h")).unwrap();
    assert!(
        header.contains("SPDX-License-Identifier: Apache-2.0"),
        "expected Apache-2.0 SPDX line in Motor.h, got header start:\n{}",
        header.lines().take(6).collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn emit_ir_writes_json_alongside() {
    let td = tempfile::tempdir().unwrap();
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--emit-ir", "--out"])
        .arg(td.path())
        .arg(VALID_FIXTURE)
        .assert()
        .success();
    let ir_path = td.path().join("Motor.ir.json");
    assert!(ir_path.is_file(), "expected Motor.ir.json under out dir");
    let txt = fs::read_to_string(&ir_path).unwrap();
    assert!(txt.contains("\"machines\""), "IR JSON missing `machines`");
}
