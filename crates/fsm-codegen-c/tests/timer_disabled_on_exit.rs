//! P0-4 regression — when a state owns a timer and an external transition
//! exits the state before the timer's deadline, the timer MUST be cancelled
//! (Doc 08 §13.2). Pre-fix, codegen never cleared the timer slot on exit
//! either, so the timer could fire stale events after the owning state had
//! already left the active configuration.
//!
//! Scenario: Idle → Running (`after 100 ms -> Faulted`, `on STOP -> Idle`).
//! Drive START, advance 50ms (timer still armed), dispatch STOP (cancels
//! timer + leaves Running), advance another 1s. Machine MUST stay in Idle
//! — no spurious fire.

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
fn ir() -> Ir {
    let timer_id = "tm-run-after".to_string();
    let after_t = TransitionObject {
        id: "t-running-after".into(),
        stable_id: "Motor:transition:after".into(),
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
    let stop_t = TransitionObject {
        id: "t-running-stop".into(),
        stable_id: "Motor:transition:stop".into(),
        source: "s-running".into(),
        target: "s-idle".into(),
        trigger: Some(Trigger::Event {
            event_id: "e-stop".into(),
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
    let timer = TimerObject {
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
            events: vec![
                EventObject {
                    id: "e-start".into(),
                    stable_id: "Motor:event:START".into(),
                    name: "START".into(),
                    payload: vec![],
                    loc: loc(),
                },
                EventObject {
                    id: "e-stop".into(),
                    stable_id: "Motor:event:STOP".into(),
                    name: "STOP".into(),
                    payload: vec![],
                    loc: loc(),
                },
            ],
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
                    make_simple("s-running", "Running", vec![after_t, stop_t], vec![timer]),
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
        fs::write(dir.join(&f.path), &f.content).expect("write generated");
    }
}

#[test]
fn timer_disabled_on_exit() {
    if !gcc_available() {
        eprintln!("[timer_disabled_on_exit] gcc not available — skipping");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let out = emit(&ir(), &CodegenConfig::default()).expect("emit");
    write_files(dir, &out);
    fs::write(
        dir.join("host_hal.c"),
        r#"#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
uint32_t fsm_hal_clock_now_ms(void) { return 0; }
void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) { fprintf(stderr, "ASSERT: %s\n", msg); abort(); }
}
"#,
    )
    .expect("write hal");

    let main_c = r#"#include "Motor.h"

void Motor_entry_IDLE(Motor_t *m)      { (void)m; }
void Motor_entry_RUNNING(Motor_t *m)   { (void)m; }
void Motor_entry_FAULTED(Motor_t *m)   { (void)m; }
void Motor_exit_IDLE(Motor_t *m)       { (void)m; }
void Motor_exit_RUNNING(Motor_t *m)    { (void)m; }
void Motor_exit_FAULTED(Motor_t *m)    { (void)m; }

int main(void) {
    Motor_t m;
    Motor_init(&m);
    Motor_Event_t start = { .id = MOTOR_EVENT_START };
    Motor_dispatch(&m, &start);
    if (Motor_current_state(&m) != MOTOR_STATE_RUNNING) return 21;
    Motor_advance_clock(&m, 50u);
    if (Motor_current_state(&m) != MOTOR_STATE_RUNNING) return 22;
    Motor_Event_t stop = { .id = MOTOR_EVENT_STOP };
    Motor_dispatch(&m, &stop);
    if (Motor_current_state(&m) != MOTOR_STATE_IDLE) return 23;
    /* Stale timer must NOT fire. */
    Motor_advance_clock(&m, 1000u);
    if (Motor_current_state(&m) != MOTOR_STATE_IDLE) return 24;
    return 0;
}
"#;
    fs::write(dir.join("main.c"), main_c).expect("write main");
    let exe = dir.join("motor_test");
    let compile = Command::new("gcc")
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
    if !compile.status.success() {
        panic!(
            "gcc failed:\nstderr={}\nMotor.c=\n{}",
            String::from_utf8_lossy(&compile.stderr),
            out.find("Motor.c").unwrap().content,
        );
    }
    let run = Command::new(&exe).output().expect("run exe");
    if !run.status.success() {
        panic!(
            "exe exit={:?}, stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        );
    }
}
