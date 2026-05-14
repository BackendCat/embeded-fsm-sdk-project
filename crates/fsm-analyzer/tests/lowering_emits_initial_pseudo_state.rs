//! Doc 09 §4.4 + §5 contract: lowering MUST emit an Initial pseudo-state
//! node inside `region.states`, and `region.initial` MUST be that
//! pseudo-state's id (not the target state's name).
//!
//! Before this contract was honoured the simulator could not initialise any
//! analyzer-lowered machine — `Interpreter::init` looked up `region.initial`
//! expecting an `{ kind: "initial", target: ... }` node and got the literal
//! state name "Idle" instead.

use fsm_analyzer::analyze;
use fsm_ir::StateNode;
use fsm_parser::parse;

fn lower(src: &str) -> fsm_ir::Ir {
    let pr = parse(src);
    let res = analyze(&pr);
    assert!(
        !res.diagnostics
            .iter()
            .any(|d| d.severity == fsm_diagnostics::Severity::Error),
        "unexpected analyzer errors: {:?}",
        res.diagnostics
    );
    res.ir.expect("ir must be produced")
}

#[test]
fn root_region_initial_field_points_at_initial_pseudo_state_node() {
    let src = r#"language fsm 2.0
machine Motor {
    events { START STOP }
    initial Idle
    state Idle { on START -> Running }
    state Running { on STOP -> Idle }
}"#;

    let ir = lower(src);
    let m = &ir.machines[0];
    let region = &m.root;
    assert!(
        !region.initial.is_empty(),
        "region.initial must be set when source declared `initial`"
    );

    // The id stored in region.initial MUST resolve to a StateNode::Initial
    // inside region.states.
    let init_node = region
        .states
        .iter()
        .find(|s| matches!(s, StateNode::Initial(_)))
        .expect("region.states must contain an Initial pseudo-state");
    let StateNode::Initial(init) = init_node else {
        unreachable!()
    };
    assert_eq!(
        init.id, region.initial,
        "region.initial must equal the Initial pseudo-state's id (Doc 09 §5)"
    );

    // Initial.target must be a STATE ID, not the bare name "Idle".
    // The analyzer's canonical state-id form is `s-<machine>-<name>`.
    assert_eq!(init.target, "s-Motor-Idle");

    // The target id must actually exist among the region's simple states.
    let idle = region
        .states
        .iter()
        .find_map(|s| match s {
            StateNode::Simple(simple) if simple.id == init.target => Some(simple),
            _ => None,
        })
        .expect("Initial.target must point at an actual state in the region");
    assert_eq!(idle.name, "Idle");
}

#[test]
fn nested_composite_inner_region_initial_is_also_pseudo_state_id() {
    let src = r#"language fsm 2.0
machine M {
    events { E }
    initial Outer
    state Outer {
        initial Inner
        state Inner { on E -> Inner }
    }
}"#;

    let ir = lower(src);
    let outer = ir.machines[0]
        .root
        .states
        .iter()
        .find_map(|s| match s {
            StateNode::Composite(c) => Some(c),
            _ => None,
        })
        .expect("Outer is composite");
    let inner_region = outer.regions.first().expect("composite has one region");
    let inner_initial = inner_region
        .states
        .iter()
        .find_map(|s| match s {
            StateNode::Initial(i) => Some(i),
            _ => None,
        })
        .expect("inner region has an Initial pseudo-state");
    assert_eq!(inner_region.initial, inner_initial.id);
    assert_eq!(inner_initial.target, "s-M-Inner");
}

#[test]
fn ir_is_consumable_by_simulator_machine_index_contract() {
    // Mirror the lookup the simulator's `MachineIndex::build` does at init:
    // (1) look up region.initial in region.states; (2) confirm it's an
    // Initial pseudo; (3) confirm its `target` resolves to a state in the
    // same region.
    let src = r#"language fsm 2.0
machine Motor {
    events { START STOP }
    initial Idle
    state Idle { on START -> Running }
    state Running { on STOP -> Idle }
}"#;

    let ir = lower(src);
    let region = &ir.machines[0].root;

    // Step 1+2: find Initial pseudo by id.
    let init = region
        .states
        .iter()
        .find_map(|s| match s {
            StateNode::Initial(i) if i.id == region.initial => Some(i),
            _ => None,
        })
        .expect(
            "simulator-compatible IR: region.initial must resolve to a StateNode::Initial \
             living in region.states",
        );

    // Step 3: target resolves.
    let resolved = region
        .states
        .iter()
        .find(|s| match s {
            StateNode::Simple(simple) => simple.id == init.target,
            StateNode::Composite(c) => c.id == init.target,
            StateNode::Parallel(p) => p.id == init.target,
            _ => false,
        })
        .expect("Initial.target must point at a real state in the region");
    let _ = resolved;
}
