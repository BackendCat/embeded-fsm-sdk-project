//! IR-schema invariant gate — behavioural acceptance for PD-3.
//!
//! Plan retrospective PD-3: `schema/ir/1.0.0/model.json` existed but was
//! NEVER enforced in the pipeline, so the analyzer↔simulator
//! `region.initial` representation mismatch (fixed in Wave 1.9) surfaced
//! three crates downstream instead of at the producer. v1.1-W0 wires the
//! schema as a `#[cfg(debug_assertions)]` gate at the analyzer's lowering
//! exit (`fsm_analyzer::analyze_with_source` →
//! `fsm_ir::validate_ir_against_schema`).
//!
//! This is a §5.4-style behavioural test *for an invariant* (not a
//! symbol-presence check). It proves the gate is **load-bearing**:
//!
//!  - POSITIVE: a real source lowered by the analyzer produces IR that
//!    validates clean against the embedded schema. (If the analyzer's
//!    `debug_assert_ir_schema` were firing spuriously on valid output,
//!    `analyze` itself would already have panicked before this assertion —
//!    every other analyzer test would be red too.)
//!  - NEGATIVE: a deliberately schema-violating IR is REJECTED by the
//!    exact function the analyzer calls, with a useful, node-localized
//!    error. This test FAILS if the gate is a no-op (e.g. the schema is
//!    gutted, the validator stubbed, or `validate_ir_against_schema`
//!    always returns `Ok`) — that is the definition of load-bearing.
//!
//! The negative case also documents *why* the analyzer's
//! `debug_assert_ir_schema` is a real safety net: feeding the analyzer
//! input that lowered to this shape would trip the debug-assert at the
//! lowering boundary, exactly where the Wave-1.9 bug should have been
//! caught.

#![cfg(feature = "schema-validate")]
#![allow(deprecated)]

use fsm_analyzer::analyze;
use fsm_diagnostics::{Severity, SourceLocation, Span};
use fsm_ir::{
    validate_ir_against_schema, EventObject, InitialPseudo, Ir, MachineObject, OverflowPolicy,
    ParallelState, QueueConfig, RegionObject, SimpleState, StateNode, TransitionKind,
    TransitionObject, Trigger,
};

fn loc() -> SourceLocation {
    SourceLocation::new("gate.fsm", Span::new(0, 1), 1, 1)
}

// ---------------------------------------------------------------------------
// POSITIVE — analyzer output validates clean.
// ---------------------------------------------------------------------------

#[test]
fn analyzer_lowered_ir_validates_against_schema() {
    let src = r#"language fsm 2.0

feature timers

extern set_speed(u16 rpm)

machine Motor {
    context {
        speed : u16 = 0
    }

    events {
        START
        STOP
        FAULT(code: u8)
    }

    initial Idle

    state Idle {
        on START -> Running
    }

    state Running {
        after 5000 ms -> Faulted
        on STOP  -> Idle : set_speed(0)
        on FAULT -> Faulted : ctx.speed = payload.code
    }

    state Faulted {
        on STOP -> Idle
    }
}"#;

    let pr = fsm_parser::parse(src);
    let res = analyze(&pr);
    assert!(
        !res.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error),
        "fixture must analyze clean; got: {:?}",
        res.diagnostics
    );
    let ir = res.ir.expect("analyzer produced IR");

    // The exact function the analyzer's debug-assert gate calls. Reaching
    // here at all means `analyze` did NOT panic in its own
    // `debug_assert_ir_schema`, i.e. the gate accepts genuine output; this
    // re-assertion makes the contract explicit and fails loudly if a
    // future schema edit regresses a real lowering shape.
    if let Err(violations) = validate_ir_against_schema(&ir) {
        panic!(
            "analyzer-lowered IR must satisfy schema/ir/1.0.0/model.json — \
             realign the schema to model.rs. Violations:\n  {}",
            violations.join("\n  ")
        );
    }
}

#[test]
fn every_shipped_example_class_lowers_to_schema_valid_ir() {
    // A timer + payload + action + guard machine exercises Trigger::After
    // (`duration_ms`/`timer_id`), payload binding, Statement::Call and a
    // field-compare guard — the precise shapes whose schema drift PD-3's
    // gate just caught and W0 realigned. Keeping this as an explicit
    // regression so a future schema/model divergence in any of them is a
    // hard failure, not silent.
    let src = r#"language fsm 2.0

feature timers

extern dispense()

machine Vend {
    context {
        credit : u16 = 0
        price  : u16 = 150
    }

    events {
        COIN(amount: u16)
        SELECT
        DISPENSE
    }

    initial Idle

    state Idle {
        on COIN                          -> Idle : ctx.credit = ctx.credit + payload.amount
        on SELECT [ctx.credit >= ctx.price] -> Vending
    }

    state Vending {
        after 3000 ms -> Idle
        on DISPENSE   -> Idle : dispense()
    }
}"#;
    let pr = fsm_parser::parse(src);
    let res = analyze(&pr);
    assert!(
        !res.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error),
        "fixture must analyze clean; got: {:?}",
        res.diagnostics
    );
    let ir = res.ir.expect("analyzer produced IR");
    assert!(
        validate_ir_against_schema(&ir).is_ok(),
        "timer/payload/guard machine must be schema-valid: {:?}",
        validate_ir_against_schema(&ir).err()
    );
}

// ---------------------------------------------------------------------------
// NEGATIVE — proves the gate REJECTS malformed IR (load-bearing proof).
// ---------------------------------------------------------------------------

/// Build a minimal IR whose ONLY defect is a `parallel` state carrying a
/// single region. The schema requires `minItems: 2` for parallel regions
/// (Doc 09 §4.3 / analyzer H0004), so a correct gate MUST reject this.
///
/// This is the kind of structurally-invalid IR a lowering bug could
/// produce; the analyzer's `debug_assert_ir_schema` exists precisely to
/// stop such IR at the boundary instead of letting codegen/sim choke on it
/// three crates later (the Wave-1.9 failure mode).
fn ir_with_one_region_parallel() -> Ir {
    let inner_simple = StateNode::Simple(SimpleState {
        id: "s-a".into(),
        stable_id: "M:state:A".into(),
        name: "A".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![TransitionObject {
            id: "t-0".into(),
            stable_id: "M:t:0".into(),
            source: "s-a".into(),
            target: "s-a".into(),
            trigger: Some(Trigger::Event {
                event_id: "ev-e".into(),
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
    });
    // Exactly ONE region — the schema violation.
    let parallel = StateNode::Parallel(ParallelState {
        id: "s-par".into(),
        stable_id: "M:state:Par".into(),
        name: "Par".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        regions: vec![RegionObject {
            id: "r-only".into(),
            stable_id: None,
            name: "Only".into(),
            initial: "ps-a-init".into(),
            states: vec![
                StateNode::Initial(InitialPseudo {
                    id: "ps-a-init".into(),
                    target: "s-a".into(),
                    loc: loc(),
                }),
                inner_simple,
            ],
            priority: 0,
            loc: loc(),
        }],
        loc: loc(),
    });
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "sha256:test".into(),
        source_files: vec!["gate.fsm".into()],
        machines: vec![MachineObject {
            id: "m-m".into(),
            stable_id: "M".into(),
            name: "M".into(),
            context: Default::default(),
            events: vec![EventObject {
                id: "ev-e".into(),
                stable_id: "M:event:E".into(),
                name: "E".into(),
                payload: vec![],
                loc: loc(),
            }],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-root-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-root-init".into(),
                        target: "s-par".into(),
                        loc: loc(),
                    }),
                    parallel,
                ],
                priority: 0,
                loc: loc(),
            },
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            queue: QueueConfig {
                capacity: 8,
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
fn gate_rejects_schema_violating_ir_with_useful_error() {
    let bad = ir_with_one_region_parallel();

    // Sanity: the IR is otherwise well-formed Rust — the ONLY problem is
    // the schema-level parallel-region cardinality. So a passing gate here
    // can only mean the validator is a no-op (schema gutted / stubbed /
    // always-Ok). That is exactly what "load-bearing" tests against.
    let result = validate_ir_against_schema(&bad);
    let violations = result.expect_err(
        "GATE NOT LOAD-BEARING: a single-region `parallel` IR was accepted. \
         The schema requires >=2 regions for parallel states (Doc 09 §4.3); \
         if this passes, schema/ir/1.0.0/model.json is no longer enforced \
         and the PD-3 protection is gone.",
    );

    assert!(
        !violations.is_empty(),
        "rejection must carry at least one violation message"
    );
    // The error must be actionable: it points at a node path under
    // /machines/.../states (not an opaque 'invalid' with no location).
    let joined = violations.join("\n");
    assert!(
        joined.contains("/machines/0/root/states"),
        "violation must localize the offending node; got:\n{joined}"
    );
}

#[test]
fn gate_rejects_wrong_ir_version_shape() {
    // A second, independent schema violation so the negative proof does
    // not hinge on one rule: `irVersion` must match MAJOR.MINOR.PATCH.
    // A gutted/stub validator would accept this too.
    let mut bad = ir_with_one_region_parallel();
    // Fix the parallel defect so THIS test isolates the version rule…
    if let StateNode::Parallel(p) = &mut bad.machines[0].root.states[1] {
        // Duplicate the lone region to satisfy minItems:2, leaving only
        // the irVersion defect below.
        let r0 = p.regions[0].clone();
        let mut r1 = r0.clone();
        r1.id = "r-two".into();
        p.regions.push(r1);
    }
    assert!(
        validate_ir_against_schema(&bad).is_ok(),
        "control: with two regions the IR must be schema-valid (isolates \
         the version rule for the next assertion): {:?}",
        validate_ir_against_schema(&bad).err()
    );

    bad.ir_version = "v1".into(); // not MAJOR.MINOR.PATCH
    let violations =
        validate_ir_against_schema(&bad).expect_err("malformed irVersion must be rejected");
    let joined = violations.join("\n");
    assert!(
        joined.contains("/irVersion"),
        "violation must point at /irVersion; got:\n{joined}"
    );
}
