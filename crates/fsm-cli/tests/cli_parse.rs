//! Integration tests for `fsm parse`.

use assert_cmd::Command;

const VALID_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/motor.fsm");

#[test]
fn parse_clean_file_exits_zero() {
    Command::cargo_bin("fsm")
        .unwrap()
        .args(["parse", VALID_FIXTURE])
        .assert()
        .success();
}

#[test]
fn emit_cst_produces_json_tree() {
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["parse", "--emit-cst", VALID_FIXTURE])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("emit-cst stdout is valid JSON");
    // The root kind is FILE per fsm-parser/cst.
    assert_eq!(parsed["kind"], "FILE");
    assert!(parsed["children"].is_array());
}

#[test]
fn emit_ast_lists_machines() {
    let assertion = Command::cargo_bin("fsm")
        .unwrap()
        .args(["parse", "--emit-ast", VALID_FIXTURE])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assertion.get_output().stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("emit-ast stdout is valid JSON");
    assert_eq!(parsed["kind"], "File");
    let machines = parsed["machines"].as_array().expect("machines array");
    assert_eq!(machines.len(), 1);
    assert_eq!(machines[0]["name"], "Motor");
}
