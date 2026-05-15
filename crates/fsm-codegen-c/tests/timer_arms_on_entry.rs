//! P0-4 regression — a `after N ms -> X` timer MUST be armed when the
//! owning state is entered (Doc 08 §13.1), not only at `Motor_init`.
//!
//! ## Test classification (v1.1-W0 / SUBAGENT_CONVENTIONS §5.4, PD-2)
//!
//! The whole test IS the §5.4 behavioural guard: it emits the IR,
//! compiles with `gcc -…-Werror`, RUNS it, drives Idle→Running, advances
//! the virtual clock past the timer, and asserts the machine reaches
//! Faulted. The two `header.contains("MOTOR_EVENT_TIMER_")` /
//! `"_FIRED,"` assertions are a **secondary structural sanity check**
//! (§5.4 last paragraph) on the emitted header *shape*, guarded by the
//! same test's runtime assertions. Not converted — already behavioural.
//!
//! Pre-fix the generated `Motor_init` armed the timer slot for the initial
//! state only. Entering a timer-owning state via a later transition left
//! the slot at zero and `Motor_advance_clock` did nothing for the rest of
//! the program's lifetime. This test drives Idle → Running (which has
//! `after 100 ms -> Faulted`), advances the clock past the threshold, and
//! asserts the machine reaches Faulted.

#![cfg(not(target_os = "windows"))]

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    ContextSchema, EventObject, InitialPseudo, Ir, MachineObject, OverflowPolicy, QueueConfig,
    RegionObject, SimpleState, StateNode, TimerKind, TimerObject, TransitionKind, TransitionObject,
    Trigger,
};

fn loc() -> SourceLocation {
    SourceLocation::new("p04.fsm", Span::new(0, 1), 1, 1)
}

#[allow(deprecated)]
fn ir_with_after_in_running() -> Ir {
    let timer_id = "tm-running-after".to_string();
    let running_after_faulted = TransitionObject {
        id: "t-running-after-faulted".into(),
        stable_id: "Motor:transition:running-after-faulted".into(),
        source: "s-running".into(),
        target: "s-faulted".into(),
        trigger: Some(Trigger::After {
            duration_ms: 100,
            timer_id: timer_id.clone(),
        }),
        guard: None,
        actions: vec![],
        priority: 0,
        kind: TransitionKind::External,
        internal: false,
        hint: None,
        loc: loc(),
    };
    let idle_to_running = TransitionObject {
        id: "t-idle-running".into(),
        stable_id: "Motor:transition:idle-running".into(),
        source: "s-idle".into(),
        target: "s-running".into(),
        trigger: Some(Trigger::Event {
            event_id: "e-start".into(),
            payload_binding: None,
        }),
        guard: None,
        actions: vec![],
        priority: 100,
        kind: TransitionKind::External,
        internal: false,
        hint: None,
        loc: loc(),
    };
    let running_timer = TimerObject {
        id: timer_id,
        stable_id: "Motor:timer:RunningAfter".into(),
        kind: TimerKind::After,
        duration_ms: 100,
        owner_state_id: "s-running".into(),
        target: Some("s-faulted".into()),
        actions: vec![],
        loc: loc(),
    };
    let make_simple =
        |id: &str, name: &str, transitions: Vec<TransitionObject>, timers: Vec<TimerObject>| {
            StateNode::Simple(SimpleState {
                id: id.into(),
                stable_id: format!("Motor:state:{}", name),
                name: name.into(),
                entry: vec![],
                exit: vec![],
                transitions,
                timers,
                defers: vec![],
                loc: loc(),
            })
        };
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "".into(),
        source_files: vec!["p04.fsm".into()],
        machines: vec![MachineObject {
            id: "m-motor".into(),
            stable_id: "Motor".into(),
            name: "Motor".into(),
            context: ContextSchema::default(),
            events: vec![EventObject {
                id: "e-start".into(),
                stable_id: "Motor:event:START".into(),
                name: "START".into(),
                payload: vec![],
                loc: loc(),
            }],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-init".into(),
                        target: "s-idle".into(),
                        loc: loc(),
                    }),
                    make_simple("s-idle", "Idle", vec![idle_to_running], vec![]),
                    make_simple(
                        "s-running",
                        "Running",
                        vec![running_after_faulted],
                        vec![running_timer],
                    ),
                    make_simple("s-faulted", "Faulted", vec![], vec![]),
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

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn write_files(dir: &Path, out: &fsm_codegen_c::EmittedFiles) {
    for f in &out.files {
        let p = dir.join(&f.path);
        fs::write(&p, &f.content).expect("write generated file");
    }
}

fn host_hal_c() -> &'static str {
    // Virtual clock advances explicitly via Motor_advance_clock; the HAL
    // returns 0 so no implicit time elapses between dispatch and tick.
    r#"#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>

uint32_t fsm_hal_clock_now_ms(void) { return 0; }

void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) { fprintf(stderr, "ASSERT: %s\n", msg); abort(); }
}
"#
}

#[test]
fn timer_arms_on_entry_to_running() {
    if !gcc_available() {
        eprintln!("[timer_arms_on_entry] gcc not available — skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let ir = ir_with_after_in_running();
    let out = emit(&ir, &CodegenConfig::default()).expect("emit");
    write_files(dir, &out);
    fs::write(dir.join("host_hal.c"), host_hal_c()).expect("write hal");

    // Sanity: generated header must contain a distinct timer event variant.
    let header = out.find("Motor.h").expect("Motor.h").content.clone();
    assert!(
        header.contains("MOTOR_EVENT_TIMER_"),
        "expected per-timer event variant in Motor.h, got:\n{header}"
    );
    assert!(
        header.contains("_FIRED,"),
        "expected `_FIRED` suffix on timer event in Motor.h"
    );

    let main_c = r#"#include "Motor.h"
#include <stdio.h>

void Motor_entry_IDLE(Motor_t *m)      { (void)m; }
void Motor_entry_RUNNING(Motor_t *m)   { (void)m; }
void Motor_entry_FAULTED(Motor_t *m)   { (void)m; }
void Motor_exit_IDLE(Motor_t *m)       { (void)m; }
void Motor_exit_RUNNING(Motor_t *m)    { (void)m; }
void Motor_exit_FAULTED(Motor_t *m)    { (void)m; }

int main(void) {
    Motor_t m;
    Motor_init(&m);
    if (Motor_current_state(&m) != MOTOR_STATE_IDLE) return 10;
    Motor_Event_t start = { .id = MOTOR_EVENT_START };
    Motor_dispatch(&m, &start);
    if (Motor_current_state(&m) != MOTOR_STATE_RUNNING) return 11;
    /* Advance 50ms — timer is at 100ms, must NOT fire. */
    Motor_advance_clock(&m, 50u);
    if (Motor_current_state(&m) != MOTOR_STATE_RUNNING) return 12;
    /* Advance another 50ms — timer reaches due, MUST fire to Faulted. */
    Motor_advance_clock(&m, 50u);
    if (Motor_current_state(&m) != MOTOR_STATE_FAULTED) return 13;
    return 0;
}
"#;
    fs::write(dir.join("main.c"), main_c).expect("write main.c");

    let exe = dir.join("motor_test");
    let status = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-I.",
            "Motor.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");
    if !status.status.success() {
        panic!(
            "gcc failed:\nstdout={}\nstderr={}\nMotor.c=\n{}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr),
            out.find("Motor.c").unwrap().content,
        );
    }
    let run = Command::new(&exe).output().expect("run test exe");
    if !run.status.success() {
        panic!(
            "test executable returned non-zero exit {:?}: {}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        );
    }
}
