//! Hierarchical-dispatch test — Doc 00 §7.8 (B-10).
//!
//! Composite parent `Operational` declares `FAULT -> Error`. When the
//! active leaf is `Op.Running`, the dispatcher MUST walk up via the parent
//! table to find the composite's transition. We assert this pattern by
//! checking that the generated Motor.c references the parent table and
//! the leaf-to-root loop.

#[path = "common/mod.rs"]
mod common;

use fsm_codegen_c::{emit, CodegenConfig};

#[test]
fn parent_table_emitted_for_hierarchical_machine() {
    let out = emit(&common::hierarchical_motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // The B-10 parent table must appear.
    assert!(
        c.contains("Motor_parent_table"),
        "Motor_parent_table missing in generated source"
    );
}

#[test]
fn dispatch_walks_ancestors_via_parent_table() {
    let out = emit(&common::hierarchical_motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // The outer dispatch should reference both the parent table and the
    // ROOT terminator — the leaf-to-root loop pattern.
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
    // The composite's transition (FAULT -> Error) must be reachable from
    // the try_transitions_in_state switch when called with the composite's
    // state id.
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
    // The root sentinel's parent slot is ROOT itself (terminator). Look
    // for the line `[0] = MOTOR_STATE_ROOT`.
    assert!(
        c.contains("[0] = MOTOR_STATE_ROOT"),
        "parent_table[0] must be ROOT sentinel"
    );
}
