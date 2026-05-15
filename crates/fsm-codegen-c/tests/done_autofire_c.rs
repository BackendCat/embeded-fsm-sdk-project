//! Audit 2026-05-14 — generated C must auto-fire `done` on non-final states.
//!
//! ## Test classification (v1.1-W0 / SUBAGENT_CONVENTIONS §5.4, PD-2)
//!
//! This file ALREADY carries its §5.4 behavioural guard:
//! `done_on_simple_state_drives_runtime_in_gcc_built_binary` compiles the
//! generated C with `gcc -…-Werror`, RUNS it, and asserts the machine
//! auto-advances Start→Middle on init (no caller dispatch) then →Stop on
//! GO. The `.contains()` in `done_on_simple_state_appears_in_generated_\
//! handle_completion` are a **secondary structural check** (§5.4 last
//! paragraph): they pin that the auto-fire dispatch sits *inside the Start
//! case* of `handle_completion` (a precise emission-shape invariant) and
//! localize a regression faster than the slower gcc test. Not converted —
//! the behavioural guard already exists in this same file.
//!
//! Sibling test to `crates/fsm-simulator/tests/done_autofire.rs`: proves
//! the codegen-c emission produces gcc-compilable C that, when run, makes
//! a Simple state with `done -> Target` move to `Target` *without* the
//! caller dispatching `EVENT__COMPLETION` manually.
//!
//! Pre-fix, the `vending_machine_gcc` test had to dispatch the completion
//! event by hand (`comp.id = VENDINGMACHINE_EVENT__COMPLETION`). The fix
//! extends `Motor_handle_completion`'s switch to also fire on Simple
//! states that declare a `done` transition.
//!
//! Mirrors the gcc-driven layout in `gcc_compile.rs`; skipped when gcc is
//! not on PATH.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    EventObject, InitialPseudo, Ir, MachineObject, OverflowPolicy, QueueConfig, RegionObject,
    SimpleState, StateNode, TransitionKind, TransitionObject,
};

fn loc() -> SourceLocation {
    SourceLocation::new("autofire.fsm", Span::new(0, 1), 1, 1)
}

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Build a three-state machine: `Start --done--> Middle --GO--> Stop`,
/// initial = Start. After init alone, the runtime should land in Middle
/// (auto-fired). Dispatching GO then takes us to Stop.
fn autofire_ir() -> Ir {
    #[allow(deprecated)]
    let start_done = TransitionObject {
        id: "t-start-done".into(),
        stable_id: "AutoFire:transition:t-start-done".into(),
        source: "s-start".into(),
        target: "s-middle".into(),
        trigger: None,
        guard: None,
        actions: vec![],
        priority: 0,
        kind: TransitionKind::Completion,
        internal: false,
        loc: loc(),
    };
    #[allow(deprecated)]
    let middle_to_stop = TransitionObject {
        id: "t-middle-stop".into(),
        stable_id: "AutoFire:transition:t-middle-stop".into(),
        source: "s-middle".into(),
        target: "s-stop".into(),
        trigger: Some(fsm_ir::Trigger::Event {
            event_id: "e-go".into(),
            payload_binding: None,
        }),
        guard: None,
        actions: vec![],
        priority: 0,
        kind: TransitionKind::External,
        internal: false,
        loc: loc(),
    };
    let start = SimpleState {
        id: "s-start".into(),
        stable_id: "AutoFire:state:Start".into(),
        name: "Start".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![start_done],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    };
    let middle = SimpleState {
        id: "s-middle".into(),
        stable_id: "AutoFire:state:Middle".into(),
        name: "Middle".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![middle_to_stop],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    };
    let stop = SimpleState {
        id: "s-stop".into(),
        stable_id: "AutoFire:state:Stop".into(),
        name: "Stop".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    };
    let root = RegionObject {
        id: "r-root".into(),
        stable_id: None,
        name: "__root".into(),
        initial: "ps-init".into(),
        states: vec![
            StateNode::Initial(InitialPseudo {
                id: "ps-init".into(),
                target: "s-start".into(),
                loc: loc(),
            }),
            StateNode::Simple(start),
            StateNode::Simple(middle),
            StateNode::Simple(stop),
        ],
        priority: 0,
        loc: loc(),
    };
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "".into(),
        source_files: vec!["autofire.fsm".into()],
        machines: vec![MachineObject {
            id: "m-autofire".into(),
            stable_id: "AutoFire".into(),
            name: "AutoFire".into(),
            context: Default::default(),
            events: vec![EventObject {
                id: "e-go".into(),
                stable_id: "AutoFire:event:GO".into(),
                name: "GO".into(),
                payload: vec![],
                loc: loc(),
            }],
            externs: vec![],
            root,
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
fn done_on_simple_state_appears_in_generated_handle_completion() {
    // Emit-only check (runs on every host, no gcc dependency): the
    // generated `AutoFire_handle_completion` body must include a case
    // for `Start` that calls `AutoFire_dispatch(m, &comp);`.
    let ir = autofire_ir();
    let out = emit(&ir, &CodegenConfig::default()).expect("emit");
    let c = out
        .find("AutoFire.c")
        .expect("AutoFire.c emitted")
        .content
        .clone();
    // Isolate the handle_completion function body. The source contains
    // a forward declaration (ends with `;`) followed by the actual
    // definition (starts with `static void AutoFire_handle_completion(... {`).
    // We anchor on the definition by searching for the `_handle_completion`
    // text followed by `) {`.
    let needle = "AutoFire_handle_completion(AutoFire_t *m) {";
    let fn_start = c
        .find(needle)
        .expect("handle_completion definition present");
    let body_open = c[fn_start..]
        .find('{')
        .expect("function open brace present");
    // Track brace depth to find the matching close brace.
    let bytes = c.as_bytes();
    let body_start = fn_start + body_open;
    let mut depth = 0i32;
    let mut body_end = c.len();
    for (i, b) in bytes.iter().enumerate().skip(body_start) {
        match *b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &c[body_start..body_end];
    assert!(
        body.contains("case AUTOFIRE_STATE_START:"),
        "handle_completion must include the Start case (auto-fire entry);\n--- body ---\n{}",
        body,
    );
    assert!(
        body.contains("AutoFire_dispatch(m, &comp);"),
        "handle_completion must dispatch a completion event somewhere in its body;\n--- body ---\n{}",
        body,
    );
    // Also assert that the call sits inside the Start case rather than only
    // appearing in some unrelated path (defensive).
    let start_block_idx = body
        .find("case AUTOFIRE_STATE_START:")
        .expect("Start case present");
    let after_start = &body[start_block_idx..];
    let break_at = after_start
        .find("break;")
        .expect("Start case must end with break");
    let case_body = &after_start[..break_at];
    assert!(
        case_body.contains("AutoFire_dispatch(m, &comp);"),
        "Start case body inside handle_completion must dispatch a completion;\n--- case body ---\n{}",
        case_body,
    );
}

#[test]
fn done_on_simple_state_drives_runtime_in_gcc_built_binary() {
    if !gcc_available() {
        eprintln!("[done_autofire_c] gcc not on PATH — skipping");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let out = emit(&autofire_ir(), &CodegenConfig::default()).expect("emit");
    for f in &out.files {
        fs::write(dir.join(&f.path), &f.content).expect("write generated");
    }
    write_host_hal_c(dir);
    write_main_c(dir);

    let exe = dir.join("autofire_test");
    let build = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "AutoFire.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");
    if !build.status.success() || !build.stderr.is_empty() {
        let c = out.find("AutoFire.c").unwrap().content.clone();
        eprintln!("=== AutoFire.c ===\n{}", c);
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&build.stderr)
        );
        panic!("gcc build failed: {:?}", build.status.code());
    }
    let run = Command::new(&exe).output().expect("run binary");
    if !run.status.success() {
        let c = out.find("AutoFire.c").unwrap().content.clone();
        eprintln!("=== AutoFire.c ===\n{}", c);
        eprintln!("=== stderr ===\n{}", String::from_utf8_lossy(&run.stderr));
        panic!("autofire_test failed: exit={:?}", run.status.code());
    }
}

fn write_host_hal_c(dir: &Path) {
    let hal = r#"#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>

uint32_t fsm_hal_clock_now_ms(void) { return 0; }

void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) { fprintf(stderr, "[FSM ASSERT] %s\n", msg); abort(); }
}
"#;
    fs::write(dir.join("host_hal.c"), hal).expect("write hal");
}

fn write_main_c(dir: &Path) {
    let main = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "AutoFire.h"

void AutoFire_entry_START(AutoFire_t *m)  { (void)m; }
void AutoFire_entry_MIDDLE(AutoFire_t *m) { (void)m; }
void AutoFire_entry_STOP(AutoFire_t *m)   { (void)m; }
void AutoFire_exit_START(AutoFire_t *m)   { (void)m; }
void AutoFire_exit_MIDDLE(AutoFire_t *m)  { (void)m; }
void AutoFire_exit_STOP(AutoFire_t *m)    { (void)m; }

int main(void) {
    AutoFire_t m;
    AutoFire_init(&m);
    /* After init, `done -> Middle` on Start MUST have auto-fired without
     * any caller dispatch. Pre-fix, `_active[0]` would still be `Start`.
     * Audit P1-8 sibling, 2026-05-14. */
    if (AutoFire_current_state(&m) != AUTOFIRE_STATE_MIDDLE) {
        fprintf(stderr, "init: expected Middle (auto-fired), got %d\n",
                AutoFire_current_state(&m));
        return 1;
    }
    /* Now external dispatch GO drives Middle -> Stop. */
    AutoFire_Event_t go = { .id = AUTOFIRE_EVENT_GO };
    AutoFire_dispatch(&m, &go);
    if (AutoFire_current_state(&m) != AUTOFIRE_STATE_STOP) {
        fprintf(stderr, "GO: expected Stop, got %d\n",
                AutoFire_current_state(&m));
        return 2;
    }
    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write main");
}
