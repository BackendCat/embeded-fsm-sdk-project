//! Parallel-completion test — Doc 00 §2 B-08, §7.6.
//!
//! Parallel state `Monitor` with two regions, each ending in Final. The
//! generated all-regions-final helper MUST check every region's slot
//! against its Final state ID before signalling completion.
//!
//! ## Test strategy (v1.1-W0 / SUBAGENT_CONVENTIONS §5.4, PD-2)
//!
//! The B-08 *behaviour* for `parallel_motor_ir()` — both regions dispatch
//! independently and each reaches its Final state — is proven by the
//! gcc-compile-RUN tests in `parallel_dispatch_runs.rs`
//! (`parallel_two_regions_dispatch_independently_{switch,table}`), which
//! that file's header already records as having "replaced the
//! symbol-presence-only `parallel_completion.rs` checks per the audit
//! recommendation". This file is therefore retained as **secondary
//! structural- and negative-invariant checks** (§5.4 last paragraph):
//!
//!  - `*_helper_emitted` / `helper_checks_each_region_slot` /
//!    `handle_completion_uses_helper_for_parallel_parent` pin the B-08
//!    emission *shape* (the helper exists, reads both `_active[]` slots,
//!    references the per-region Final IDs, and is the guard on the
//!    parallel branch of `handle_completion`). These localize a B-08
//!    representation regression with a precise message; the behavioural
//!    consequence is covered by `parallel_dispatch_runs.rs`.
//!  - `machine_struct_carries_active_leaf_array` and
//!    `machine_struct_does_not_emit_legacy_state_region_slots` assert the
//!    PRESENCE / ABSENCE of specific struct members — a "must not emit the
//!    legacy `_state_region_N`" contract has no runtime behaviour to
//!    observe, so symbol-(non-)presence is the *correct* tool, not a
//!    P0-1-class proxy.
//!
//! `parallel_motor_ir()`'s `Monitor` declares no parent `done ->`
//! transition, so the "all-regions-final → fire parent completion"
//! consequence is not separately observable from this fixture beyond the
//! per-region Final states `parallel_dispatch_runs.rs` already asserts at
//! runtime; introducing a parent-completion fixture purely for this file
//! would be scope creep (and risks the duplicate-event fixture defect W0
//! just fixed in `common/mod.rs`). Tracked in docs/processes/TEST_DEBT.md.

#[path = "common/mod.rs"]
mod common;

use fsm_codegen_c::{emit, CodegenConfig};

#[test]
fn all_regions_final_helper_emitted() {
    // SECONDARY structural check (§5.4) — behaviour proven by
    // `parallel_dispatch_runs.rs`. Pins the B-08 helper's existence/shape.
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
    // SECONDARY structural check (§5.4) — runtime behaviour proven by
    // `parallel_dispatch_runs.rs`.
    let out = emit(&common::parallel_motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // P0-2 fix: the helper reads each region's `_active[]` slot. With
    // region 0 sharing slot 0 and region 1 taking slot 1, both slots
    // must be referenced.
    assert!(
        c.contains("m->_active[0]") && c.contains("m->_active[1]"),
        "Per-region `_active[]` slot checks missing from B-08 helper. Generated:\n{}",
        c
    );
    // The Final state(s) for each region must appear in the conditions.
    // Common test fixture (`parallel_motor_ir`) names the per-region final
    // states `AFinal` / `BFinal` — derived from `FinalState.name` since the
    // codegen now respects per-DSL `final NAME` declarations.
    assert!(
        c.contains("MOTOR_STATE_AFINAL") && c.contains("MOTOR_STATE_BFINAL"),
        "Final state IDs missing from B-08 region check"
    );
}

#[test]
fn handle_completion_uses_helper_for_parallel_parent() {
    // SECONDARY structural check (§5.4): pins that the parallel branch of
    // `handle_completion` is GUARDED by the all-regions-final helper. The
    // independent-region-advance behaviour is proven at runtime by
    // `parallel_dispatch_runs.rs`; this fixture has no parent `done ->`
    // target so the post-guard dispatch is not separately observable here.
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
fn machine_struct_carries_active_leaf_array() {
    let out = emit(&common::parallel_motor_ir(), &CodegenConfig::default()).unwrap();
    let h = out.find("Motor.h").unwrap().content.as_str();
    let conf = out.find("Motor_conf.h").unwrap().content.as_str();
    // P0-2 fix: the struct now carries a uniform `_active[]` array sized
    // by `MOTOR_MAX_PARALLEL_REGIONS`, plus a `_active_count` counter.
    assert!(
        h.contains("_active[MOTOR_MAX_PARALLEL_REGIONS]"),
        "machine struct missing `_active[]` array. Header:\n{}",
        h
    );
    assert!(
        h.contains("uint8_t _active_count"),
        "machine struct missing `_active_count`. Header:\n{}",
        h
    );
    // For two-region parallel `Monitor`, max regions is 2.
    assert!(
        conf.contains("#define MOTOR_MAX_PARALLEL_REGIONS  2u"),
        "Motor_conf.h missing MAX_PARALLEL_REGIONS == 2 for parallel-Motor fixture. Conf:\n{}",
        conf
    );
}

#[test]
fn machine_struct_does_not_emit_legacy_state_region_slots() {
    // P0-2 fix: the codegen no longer emits the unused
    // `_state_region_N` declarations / reads.
    let out = emit(&common::parallel_motor_ir(), &CodegenConfig::default()).unwrap();
    let h = out.find("Motor.h").unwrap().content.as_str();
    let c = out.find("Motor.c").unwrap().content.as_str();
    assert!(
        !h.contains("_state_region_"),
        "header still emits legacy `_state_region_N`:\n{}",
        h
    );
    assert!(
        !c.contains("_state_region_"),
        "source still reads legacy `_state_region_N`:\n{}",
        c
    );
}
