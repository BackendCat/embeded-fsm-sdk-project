//! Wire-format byte-determinism regression — Audit D P1-C.
//!
//! Before the BTreeMap migration, `StepRecord.eventReceived.payload`,
//! `StepRecord` consumers that round-tripped a snapshot's `context`/`history`,
//! and the trace-file `InitTrace.context` all serialised via `HashMap`. Rust's
//! `HashMap` uses `RandomState` seeded per process — every program run can
//! emit a different key order, breaking Doc 13 §11 byte-exact contract under
//! golden-master replay.
//!
//! This test drives an Interpreter through a fixed sequence with a non-trivial
//! payload + context schema, serialises the resulting `StepRecord` vector +
//! the `InterpreterSnapshot` to JSON 100 times in a row, and asserts every
//! serialisation is byte-identical to the first. If any `HashMap` sneaks back
//! onto the serialisation path, this will fail almost immediately.

mod common;

use common::*;
use fsm_ir::{
    ContextField, ContextSchema, EventObject, Literal, Param, StateNode, StringLit, TransitionKind,
    Type,
};
use fsm_simulator::{InitOptions, Interpreter, Value};
use std::collections::BTreeMap;

fn payload_event() -> EventObject {
    // Five-payload event so any HashMap ordering will permute > 99% of runs.
    EventObject {
        id: "ev-go".into(),
        stable_id: "M:event:GO".into(),
        name: "GO".into(),
        payload: vec![
            Param {
                name: "zebra".into(),
                ty: Type::Primitive { name: "i32".into() },
                id: None,
                loc: None,
            },
            Param {
                name: "alpha".into(),
                ty: Type::Primitive { name: "i32".into() },
                id: None,
                loc: None,
            },
            Param {
                name: "mango".into(),
                ty: Type::Primitive { name: "i32".into() },
                id: None,
                loc: None,
            },
            Param {
                name: "bravo".into(),
                ty: Type::Primitive { name: "i32".into() },
                id: None,
                loc: None,
            },
            Param {
                name: "kilo".into(),
                ty: Type::Primitive { name: "i32".into() },
                id: None,
                loc: None,
            },
        ],
        loc: loc(),
    }
}

fn build_ir() -> fsm_ir::Ir {
    let mut idle = simple("s-idle");
    idle.transitions.push(transition(
        "t-go",
        "s-idle",
        "s-idle",
        "ev-go",
        TransitionKind::External,
    ));
    let root = region(
        "r-root",
        "ps-init",
        vec![initial("ps-init", "s-idle"), StateNode::Simple(idle)],
    );
    let mut m = machine("WireFmt", root);
    m.context = ContextSchema {
        fields: vec![
            ContextField {
                id: "cf-yankee".into(),
                name: "yankee".into(),
                ty: Type::Primitive {
                    name: "string".into(),
                },
                default: Some(Literal::String(StringLit {
                    value: "y".into(),
                    loc: None,
                })),
                loc: loc(),
            },
            ContextField {
                id: "cf-aspen".into(),
                name: "aspen".into(),
                ty: Type::Primitive {
                    name: "string".into(),
                },
                default: Some(Literal::String(StringLit {
                    value: "a".into(),
                    loc: None,
                })),
                loc: loc(),
            },
            ContextField {
                id: "cf-merlot".into(),
                name: "merlot".into(),
                ty: Type::Primitive {
                    name: "string".into(),
                },
                default: Some(Literal::String(StringLit {
                    value: "m".into(),
                    loc: None,
                })),
                loc: loc(),
            },
        ],
    };
    m.events.push(payload_event());
    ir_one_machine(m)
}

fn make_payload(seed: i32) -> BTreeMap<String, Value> {
    // Seed flips the *values* but never the *keys*.
    let mut p = BTreeMap::new();
    p.insert("zebra".into(), Value::I32(seed));
    p.insert("alpha".into(), Value::I32(seed + 1));
    p.insert("mango".into(), Value::I32(seed + 2));
    p.insert("bravo".into(), Value::I32(seed + 3));
    p.insert("kilo".into(), Value::I32(seed + 4));
    p
}

fn run_once() -> (String, String) {
    let ir = build_ir();
    let mut interp = Interpreter::new(&ir).unwrap();
    let mut init_ctx = BTreeMap::new();
    init_ctx.insert("yankee".into(), Value::String("y2".into()));
    init_ctx.insert("aspen".into(), Value::String("a2".into()));
    init_ctx.insert("merlot".into(), Value::String("m2".into()));
    let init_records = interp
        .init(InitOptions {
            machine_name: "WireFmt".into(),
            initial_context: Some(init_ctx),
            virtual_clock_start_ms: 0,
        })
        .expect("init must succeed");
    // Drive 5 dispatches with full payload — populates `StepRecord.eventReceived.payload`.
    let mut all_records = init_records;
    for i in 0..5 {
        let recs = interp
            .dispatch_with_payload("GO", Some(make_payload(i * 10)))
            .expect("dispatch ok");
        all_records.extend(recs);
    }
    let records_json = serde_json::to_string(&all_records).expect("records serialise");
    let snapshot_json =
        serde_json::to_string(&interp.snapshot().expect("snapshot ok")).expect("snap serialise");
    (records_json, snapshot_json)
}

#[test]
fn step_records_serialise_byte_identically_across_100_runs() {
    let (first_records, first_snap) = run_once();
    // Both must be non-trivial so a HashMap regression is observable.
    assert!(first_records.contains("\"alpha\""));
    assert!(first_records.contains("\"zebra\""));
    assert!(first_snap.contains("\"yankee\""));
    for iteration in 1..100 {
        let (r, s) = run_once();
        assert_eq!(
            r, first_records,
            "trace records non-deterministic at iteration {iteration} — \
             HashMap leaked back onto the serialisation path"
        );
        assert_eq!(
            s, first_snap,
            "snapshot non-deterministic at iteration {iteration} — \
             HashMap leaked back onto the serialisation path"
        );
    }
}
