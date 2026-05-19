//! Memory-budget test — Doc 00 §7.11.

#[path = "common/mod.rs"]
mod common;

use fsm_codegen_c::{compute_budget, CodegenConfig};

#[test]
fn motor_budget_is_nonempty_and_consistent() {
    let ir = common::motor_ir();
    let b = compute_budget(&ir, &CodegenConfig::default()).expect("budget");

    assert!(
        b.sizeof_context > 0,
        "context size must include u16+bool fields"
    );
    assert!(
        b.total_ram_bytes > b.sizeof_context,
        "total RAM must include queue + bookkeeping beyond context"
    );
    assert!(b.queue_bytes > 0, "non-zero queue capacity must show up");
    assert!(b.estimated_rom_bytes > 0, "any state machine has ROM cost");
    assert!(
        b.max_completion_depth >= 1,
        "depth of a 3-state flat machine is at least 1"
    );
}

#[test]
fn larger_queue_means_more_ram() {
    // F-2: `common::motor_ir()` carries an EXPLICIT in-source `QueueConfig`
    // (loc = "motor.fsm" ⇒ `is_explicit()`), so the budget's effective
    // capacity is resolved via `CodegenConfig::resolve_queue`. The bare
    // `queue_capacity` field is now ONLY the bottom-tier fallback and is
    // (correctly) ignored when the IR declares a `queue {}`. To make the
    // integrator capacity the budget driver, use the integrator OVERRIDE
    // (`queue_capacity_override`) — the post-F-2 way `--queue-size` /
    // `fsm.toml` retargets a model's queue. Pre-F-2 this test passed only
    // because codegen blindly used `config.queue_capacity` (the very
    // silent-misconfig F-2 fixes); it now asserts the override path.
    let ir = common::motor_ir();
    let cfg4 = CodegenConfig {
        queue_capacity_override: Some(4),
        ..Default::default()
    };
    let cfg16 = CodegenConfig {
        queue_capacity_override: Some(16),
        ..Default::default()
    };
    let b4 = compute_budget(&ir, &cfg4).expect("budget cfg4");
    let b16 = compute_budget(&ir, &cfg16).expect("budget cfg16");
    assert!(
        b16.queue_bytes > b4.queue_bytes,
        "an integrator queue-size override of 16 must budget more queue RAM \
         than 4 (b4={}, b16={})",
        b4.queue_bytes,
        b16.queue_bytes
    );
    assert!(b16.total_ram_bytes > b4.total_ram_bytes);
}

#[test]
fn hierarchical_machine_deeper_completion_depth() {
    let flat = compute_budget(&common::motor_ir(), &CodegenConfig::default()).expect("flat budget");
    let nested = compute_budget(&common::hierarchical_motor_ir(), &CodegenConfig::default())
        .expect("nested budget");
    assert!(
        nested.max_completion_depth >= flat.max_completion_depth,
        "nested machine should have at least the same depth: nested={:?}, flat={:?}",
        nested,
        flat
    );
}
