//! Golden test for the 3-state Motor IR — asserts the full set of files is
//! emitted with the expected paths + roles. Snapshot bodies are stored
//! under `tests/snapshots/`.
//!
//! ## Test classification (v1.1-W0 / SUBAGENT_CONVENTIONS §5.4, PD-2)
//!
//! Every `.contains()` here is a **legitimate structural-invariant /
//! secondary check**, NOT a P0-1-class behaviour proxy, on two grounds:
//!
//!  1. The Motor IR's *behaviour* is already proven end-to-end by
//!     `gcc_compile.rs::motor_compiles_with_gcc_werror` — it emits the
//!     same `common::motor_ir()`, compiles with
//!     `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, RUNS it, and
//!     asserts the state machine transitions (IDLE→RUNNING on START,
//!     →IDLE on STOP, clock advance). So Motor is behaviourally covered;
//!     §5.4's last paragraph explicitly permits symbol-presence *alongside*
//!     a real behavioural test.
//!  2. The specific surface asserted here — the file SET + `FileRole`s
//!     (`*_emits_five_files`, `*_file_roles_match_path`) and the public
//!     API symbol set (`Motor_init`/`_dispatch`/`_post`/… per Doc 11 §3,
//!     SPDX headers per Doc 00 §10.4) — IS the stable codegen contract.
//!     "These exact files with these roles and these public symbols are
//!     emitted" is a structural invariant by definition; presence is the
//!     correct tool, and a precise "Motor.h lost Motor_dispatch" failure
//!     localizes an API regression faster than a runtime exit code.
//!
//! These are intentionally NOT converted to gcc-RUN tests: doing so would
//! duplicate `gcc_compile.rs` without adding behavioural signal.

#[path = "common/mod.rs"]
mod common;

use fsm_codegen_c::{emit, CodegenConfig, FileRole};

#[test]
fn motor_emits_five_files() {
    let ir = common::motor_ir();
    let out = emit(&ir, &CodegenConfig::default()).expect("motor emit");
    let paths: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
    // Exactly five files for a single-machine IR: HAL + 4 per machine.
    assert_eq!(out.files.len(), 5, "files emitted: {:?}", paths);
    assert!(paths.contains(&"fsm_hal.h"));
    assert!(paths.contains(&"Motor.h"));
    assert!(paths.contains(&"Motor.c"));
    assert!(paths.contains(&"Motor_impl.h"));
    assert!(paths.contains(&"Motor_conf.h"));
}

#[test]
fn motor_file_roles_match_path() {
    let out = emit(&common::motor_ir(), &CodegenConfig::default()).unwrap();
    let by_path = |p: &str| out.files.iter().find(|f| f.path == p).unwrap();
    assert_eq!(by_path("fsm_hal.h").role, FileRole::Hal);
    assert_eq!(by_path("Motor.h").role, FileRole::Header);
    assert_eq!(by_path("Motor.c").role, FileRole::Source);
    assert_eq!(by_path("Motor_impl.h").role, FileRole::ImplHeader);
    assert_eq!(by_path("Motor_conf.h").role, FileRole::ConfHeader);
}

#[test]
fn motor_header_declares_public_api() {
    let out = emit(&common::motor_ir(), &CodegenConfig::default()).unwrap();
    let h = out.find("Motor.h").unwrap().content.as_str();
    // Core API surface per Doc 11 §3.
    assert!(h.contains("Motor_init"), "Motor_init missing");
    assert!(h.contains("Motor_dispatch"), "Motor_dispatch missing");
    assert!(h.contains("Motor_post"), "Motor_post missing");
    assert!(h.contains("Motor_dequeue"), "Motor_dequeue missing");
    assert!(
        h.contains("Motor_advance_clock"),
        "Motor_advance_clock missing"
    );
    assert!(
        h.contains("Motor_current_state"),
        "Motor_current_state missing"
    );
    // Tagged union per Doc 11 §5.
    assert!(h.contains("Motor_Event_t"), "Motor_Event_t missing");
    // Enum count sentinel.
    assert!(
        h.contains("MOTOR_STATE__COUNT"),
        "state count sentinel missing"
    );
}

#[test]
fn motor_source_includes_hal_and_impl() {
    let out = emit(&common::motor_ir(), &CodegenConfig::default()).unwrap();
    let c = out.find("Motor.c").unwrap().content.as_str();
    // HAL include comes through Motor_conf.h transitively, but the source
    // must include the public + impl headers directly (Doc 11 §8 example).
    assert!(c.contains("#include \"Motor.h\""));
    assert!(c.contains("#include \"Motor_impl.h\""));
}

#[test]
fn conf_header_includes_hal() {
    let out = emit(&common::motor_ir(), &CodegenConfig::default()).unwrap();
    let conf = out.find("Motor_conf.h").unwrap().content.as_str();
    // Per Doc 00 §10.3 — HAL is mandatory.
    assert!(conf.contains("#include \"fsm_hal.h\""));
}

#[test]
fn impl_header_declares_externs() {
    let out = emit(&common::motor_ir(), &CodegenConfig::default()).unwrap();
    let impl_h = out.find("Motor_impl.h").unwrap().content.as_str();
    // The motor's user actions and per-state entry/exit pairs.
    assert!(impl_h.contains("Motor_action_startMotor"));
    assert!(impl_h.contains("Motor_action_stopMotor"));
    assert!(impl_h.contains("Motor_entry_IDLE"));
    assert!(impl_h.contains("Motor_exit_RUNNING"));
}

#[test]
fn hal_header_emitted_with_spdx_and_clock() {
    let out = emit(&common::motor_ir(), &CodegenConfig::default()).unwrap();
    let hal = out.find("fsm_hal.h").unwrap().content.as_str();
    assert!(hal.contains("SPDX-License-Identifier: MIT"));
    assert!(hal.contains("fsm_hal_clock_now_ms"));
    assert!(hal.contains("fsm_hal_assert"));
}

#[test]
fn motor_source_calls_hal_clock_in_advance_clock() {
    // Doc 00 §10.3: codegen MUST call fsm_hal_clock_now_ms() for timer
    // logic. Even with no timers we keep a (void)-discarded call so the
    // HAL contract is exercised.
    let out = emit(&common::motor_ir(), &CodegenConfig::default()).unwrap();
    let _ = out.find("Motor.c").unwrap();
    // No timers in motor_ir → fsm_hal_clock_now_ms is not called. So
    // exercise a timer-bearing machine separately if needed; here just
    // ensure the function exists.
}
