//! Parallel-completion test — Doc 00 §2 B-08, §7.6.
//!
//! Parallel state `Monitor` with two regions, each ending in Final. The
//! generated all-regions-final helper MUST check every region's slot
//! against its Final state ID before signalling completion.

#[path = "common/mod.rs"]
mod common;

use fsm_codegen_c::{emit, CodegenConfig};

#[test]
fn all_regions_final_helper_emitted() {
    let out = emit(&common::parallel_motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // B-08: helper must exist and reference the parallel state's child
    // regions.
    assert!(
        c.contains("Motor_all_regions_final"),
        "B-08 helper Motor_all_regions_final missing"
    );
    assert!(
        c.contains("MOTOR_STATE_MONITOR"),
        "parallel state Monitor missing from helper"
    );
}

#[test]
fn helper_checks_each_region_slot() {
    let out = emit(&common::parallel_motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // Each region must contribute a check against its own slot.
    assert!(
        c.contains("_state_region_0"),
        "region 0 slot missing from helper"
    );
    assert!(
        c.contains("_state_region_1"),
        "region 1 slot missing from helper"
    );
    // The Final state(s) for each region must appear in the conditions.
    // Common test fixture (`parallel_motor_ir`) names the per-region final
    // states `AFinal` / `BFinal` — derived from `FinalState.name` since the
    // codegen now respects per-DSL `final NAME` declarations.
    assert!(
        c.contains("MOTOR_STATE_AFINAL") || c.contains("MOTOR_STATE_BFINAL"),
        "Final state IDs missing from B-08 region check"
    );
}

#[test]
fn handle_completion_uses_helper_for_parallel_parent() {
    let out = emit(&common::parallel_motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // The completion switch should ONLY dispatch the parent's completion
    // event after the all-regions-final helper returns true. Look for the
    // call inside Motor_handle_completion's parallel branch.
    let helper_call = c.find("Motor_all_regions_final(m, MOTOR_STATE_MONITOR)");
    assert!(
        helper_call.is_some(),
        "Motor_handle_completion must guard the parallel branch with the all-regions-final helper"
    );
}

#[test]
fn machine_struct_carries_one_state_slot_per_region() {
    let out = emit(&common::parallel_motor_ir(), &CodegenConfig::default()).unwrap();
    let h = out.find("Motor.h").unwrap().content.as_str();
    // Doc 11 §24: parallel machines need one _state_regionN per region.
    assert!(h.contains("_state_region_0"));
    assert!(h.contains("_state_region_1"));
}
