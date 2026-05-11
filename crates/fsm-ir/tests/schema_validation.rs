//! JSON Schema validation tests.
//!
//! Loads `schema/ir/1.0.0/model.json` from the repo root and verifies that:
//!   1. Representative happy-path IR documents validate successfully.
//!   2. A document missing the required `irVersion` field is rejected.
//!
//! Per Doc 09 §17 the schema file is the canonical wire-format contract;
//! any downstream consumer SHOULD run this validation before processing.

use std::path::PathBuf;

use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::*;

fn schema_path() -> PathBuf {
    // CARGO_MANIFEST_DIR points at crates/fsm-ir; the schema lives two
    // levels up under schema/ir/1.0.0/model.json.
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("schema");
    p.push("ir");
    p.push("1.0.0");
    p.push("model.json");
    p
}

fn load_schema() -> jsonschema::JSONSchema {
    let raw = std::fs::read_to_string(schema_path()).expect("schema file exists");
    let value: serde_json::Value = serde_json::from_str(&raw).expect("schema is valid JSON");
    jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .compile(&value)
        .expect("schema compiles")
}

fn loc() -> SourceLocation {
    SourceLocation::new("t.fsm", Span::new(0, 1), 1, 1)
}

fn small_motor() -> Ir {
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "sha256:abc".into(),
        source_files: vec!["motor.fsm".into()],
        machines: vec![MachineObject {
            id: "m-motor".into(),
            stable_id: "Motor".into(),
            name: "Motor".into(),
            context: ContextSchema::default(),
            events: vec![EventObject {
                id: "ev-tick".into(),
                stable_id: "Motor:event:TICK".into(),
                name: "TICK".into(),
                payload: vec![],
                loc: loc(),
            }],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-initial-0".into(),
                priority: 0,
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-initial-0".into(),
                        target: "s-idle".into(),
                        loc: loc(),
                    }),
                    #[allow(deprecated)]
                    StateNode::Simple(SimpleState {
                        id: "s-idle".into(),
                        stable_id: "Motor:state:Idle".into(),
                        name: "Idle".into(),
                        entry: vec![],
                        exit: vec![],
                        transitions: vec![TransitionObject {
                            id: "t-0".into(),
                            stable_id: "Motor:t:0".into(),
                            source: "s-idle".into(),
                            target: "s-idle".into(),
                            trigger: Some(Trigger::Event {
                                event_id: "ev-tick".into(),
                                payload_binding: None,
                            }),
                            guard: None,
                            actions: vec![],
                            priority: 100,
                            kind: TransitionKind::External,
                            internal: false,
                            loc: loc(),
                        }],
                        timers: vec![],
                        defers: vec![],
                        loc: loc(),
                    }),
                ],
                loc: loc(),
            },
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            queue: QueueConfig {
                capacity: 16,
                overflow_policy: OverflowPolicy::Assert,
                loc: loc(),
            },
            targets: vec![],
            loc: loc(),
        }],
        diagnostics: vec![],
    }
}

#[test]
fn schema_accepts_empty_ir() {
    let schema = load_schema();
    let value: serde_json::Value = serde_json::from_str(&to_json(&Ir::default()).unwrap()).unwrap();
    let result = schema.validate(&value);
    if let Err(errors) = result {
        let msgs: Vec<String> = errors.map(|e| format!("{e}")).collect();
        panic!("empty IR failed validation:\n{}", msgs.join("\n"));
    }
}

#[test]
fn schema_accepts_motor_ir() {
    let schema = load_schema();
    let json = to_json(&small_motor()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let result = schema.validate(&value);
    if let Err(errors) = result {
        let msgs: Vec<String> = errors.map(|e| format!("{e}")).collect();
        panic!(
            "motor IR failed validation:\n{}\n\nJSON was:\n{json}",
            msgs.join("\n")
        );
    }
}

#[test]
fn schema_rejects_document_missing_ir_version() {
    let schema = load_schema();
    // Hand-crafted invalid doc: omit `irVersion` entirely.
    let invalid: serde_json::Value = serde_json::json!({
        "sourceHash": "",
        "sourceFiles": [],
        "machines": [],
        "diagnostics": []
    });
    let result = schema.validate(&invalid);
    assert!(
        result.is_err(),
        "document missing `irVersion` MUST be rejected"
    );
}
