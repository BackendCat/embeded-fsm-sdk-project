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

fn load_schema() -> jsonschema::Validator {
    let raw = std::fs::read_to_string(schema_path()).expect("schema file exists");
    let value: serde_json::Value = serde_json::from_str(&raw).expect("schema is valid JSON");
    // Non-deprecated 0.22 API (see crates/fsm-ir/src/json.rs); `Validator` is
    // the concrete type the old `JSONSchema` alias pointed at.
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .build(&value)
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
                            hint: None,
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

/// Regression for v1.1-W0 / plan PD-3: the schema MUST accept the *exact*
/// serde wire shape model.rs emits for the timer triggers — `after`/`every`
/// use snake_case `duration_ms`/`period_ms` plus the post-P0-4 optional
/// `timer_id` (the enum's `rename_all="camelCase"` renames variant tags,
/// NOT struct-variant fields). The pre-W0 schema required `durationMs` and
/// forbade `timer_id`, so the gate rejected every shipped timer machine.
#[test]
fn schema_accepts_timer_trigger_wire_shape_from_model_rs() {
    use fsm_ir::{TimerKind, TimerObject, Trigger};

    // Build the precise JSON serde produces — go through the real types so
    // this test breaks if model.rs changes the wire form.
    let after = Trigger::After {
        duration_ms: 5000,
        timer_id: "tm-0".into(),
    };
    let after_json: serde_json::Value = serde_json::from_str(&to_json_value(&after)).unwrap();
    assert_eq!(after_json["kind"], "after");
    assert_eq!(after_json["duration_ms"], 5000);
    assert_eq!(after_json["timer_id"], "tm-0");

    // Embed it on a transition inside an otherwise-valid Motor and validate
    // the whole document end-to-end against the schema.
    let mut ir = small_motor();
    let timer = TimerObject {
        id: "tm-0".into(),
        stable_id: "Motor:timer:0".into(),
        kind: TimerKind::After,
        duration_ms: 5000,
        owner_state_id: "s-idle".into(),
        target: Some("s-idle".into()),
        actions: vec![],
        loc: loc(),
    };
    if let StateNode::Simple(s) = &mut ir.machines[0].root.states[1] {
        s.timers.push(timer);
        s.transitions[0].trigger = Some(after);
    } else {
        panic!("fixture shape changed");
    }

    let schema = load_schema();
    let json = to_json(&ir).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let errs: Vec<String> = match schema.validate(&value) {
        Ok(()) => Vec::new(),
        Err(errors) => errors.map(|e| format!("{e}")).collect(),
    };
    assert!(
        errs.is_empty(),
        "timer-trigger IR rejected — schema drifted from model.rs again \
         (PD-3 class):\n{}\n\nJSON:\n{json}",
        errs.join("\n")
    );
}

fn to_json_value<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap()
}
