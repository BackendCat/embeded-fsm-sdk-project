//! §5.4 behavioural-acceptance — submachine C99 codegen actually runs the
//! sub-instance (v1.1-W2d, epic loop-closer 4/4).
//!
//! W2b lowered `submachine Connection { … }` / `state Connecting is
//! Connection { done -> Online }` to correct IR (`MachineObject.submachines`
//! populated + a `StateNode::Submachine` ref-state). W2c made the
//! *simulator* run the sub-instance. Pre-W2d the *codegen* treated
//! `StateNode::Submachine` as an inert leaf: generated C compiled
//! `-Werror`-clean but the sub was never instantiated, no event delegated,
//! and the parent `done -> Online` never fired — the parent was
//! behaviourally stuck on `Connecting`. So on `main` this test's binary
//! exits non-zero (the parent never reaches `Online`); it passes only after
//! the W2d codegen wave.
//!
//! NOT a symbol-presence test (SUBAGENT_CONVENTIONS §5.2 / §5.4). It builds
//! the Device+Connection IR (the exact shape W2b's analyzer emits — verified
//! against `fsm generate --emit-ir examples/submachine/submachine.fsm`),
//! `emit()`s C for BOTH dispatch strategies, compiles each with
//! `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, links a host HAL stub +
//! driver `main`, EXECUTES it, and asserts (via process exit codes) the
//! observable Doc 08 §12 behaviour:
//!
//!   1. init lands on `Connecting`; the `Connection` sub-instance is
//!      instantiated at its implicit initial `Idle` (§12.2).
//!   2. `CONNECT`/`ACK`/`ESTABLISHED` are not consumed by the parent
//!      (`Connecting` only has `RECONNECT` + the `done` completion), so each
//!      delegates into the sub: `Idle → Handshake → Established → Done`
//!      (§12.1, after transition-wins parent selection). Parent stays
//!      `Connecting` throughout.
//!   3. The sub reaching its `final Done` (§12.3) makes the parent receive
//!      a synthetic completion that fires `Connecting --done--> Online`
//!      through the EXISTING completion machinery — NO external event.
//!   4. `RECONNECT` from `Online` re-enters `Connecting` with a FRESH
//!      sub-instance (back at `Idle`, not the stale `Done`); the fresh sub
//!      progresses again (proves a real re-init, not a one-shot).
//!   5. `RECONNECT` while IN `Connecting` (the ref-state's own
//!      self-transition) wins over delegation (transition-wins) and resets
//!      the sub fresh — mirroring W2c's `run_exit` teardown → re-sync.
//!
//! This is the gcc-RUN sim≡codegen cross-check the W2c brief deferred to
//! W2d: the observable parent/sub progression asserted here is exactly the
//! one `examples/submachine/submachine.trace` (W2c's hand-verified ground
//! truth) encodes. Both strategies must reproduce it.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig, DispatchStrategy};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    ContextSchema, EventObject, FinalState, InitialPseudo, Ir, MachineObject, OverflowPolicy,
    QueueConfig, RegionObject, SimpleState, StateNode, SubmachineRef, TransitionKind,
    TransitionObject, Trigger,
};

fn loc() -> SourceLocation {
    SourceLocation::new("submachine.fsm", Span::new(0, 1), 1, 1)
}

fn event(machine: &str, name: &str) -> EventObject {
    EventObject {
        id: format!("ev-{machine}-{name}"),
        stable_id: format!("M:{machine}:event:{name}"),
        name: name.into(),
        payload: vec![],
        loc: loc(),
    }
}

#[allow(deprecated)]
fn ev_transition(
    machine: &str,
    idx: usize,
    source: &str,
    target: &str,
    ev: &str,
) -> TransitionObject {
    TransitionObject {
        id: format!("t-{machine}-{idx}"),
        stable_id: format!("M:{machine}:transition:t-{machine}-{idx}"),
        source: source.into(),
        target: target.into(),
        trigger: Some(Trigger::Event {
            event_id: format!("ev-{machine}-{ev}"),
            payload_binding: None,
        }),
        guard: None,
        actions: vec![],
        priority: 0,
        kind: TransitionKind::External,
        internal: false,
        hint: None,
        loc: loc(),
    }
}

/// A `done -> Target` completion edge (no trigger, `TransitionKind::Completion`)
/// — exactly how W2b lowers `done -> Online` on the SubmachineRef.
#[allow(deprecated)]
fn done_transition(machine: &str, idx: usize, source: &str, target: &str) -> TransitionObject {
    TransitionObject {
        id: format!("t-{machine}-{idx}"),
        stable_id: format!("M:{machine}:transition:t-{machine}-{idx}"),
        source: source.into(),
        target: target.into(),
        trigger: None,
        guard: None,
        actions: vec![],
        priority: 0,
        kind: TransitionKind::Completion,
        internal: false,
        hint: None,
        loc: loc(),
    }
}

fn simple(machine: &str, name: &str, transitions: Vec<TransitionObject>) -> StateNode {
    StateNode::Simple(SimpleState {
        id: format!("s-{machine}-{name}"),
        stable_id: format!("M:{machine}:state:{name}"),
        name: name.into(),
        entry: vec![],
        exit: vec![],
        transitions,
        timers: vec![],
        defers: vec![],
        loc: loc(),
    })
}

/// The reusable `Connection` submachine template — its own self-contained
/// `MachineObject` (id `m-Connection`), exactly as W2b mirrors every
/// `submachine` into the parent's `submachines` with `submachines: []` (the
/// nested lower uses `include_submachines:false`). Idle —CONNECT→ Handshake
/// —ACK→ Established —ESTABLISHED→ Done(final).
fn connection_template() -> MachineObject {
    MachineObject {
        id: "m-Connection".into(),
        stable_id: "M:Connection".into(),
        name: "Connection".into(),
        context: ContextSchema { fields: vec![] },
        events: vec![
            event("Connection", "CONNECT"),
            event("Connection", "ACK"),
            event("Connection", "ESTABLISHED"),
        ],
        externs: vec![],
        root: RegionObject {
            id: "r-Connection-root".into(),
            stable_id: None,
            name: "Connection__root".into(),
            initial: "ps-initial-Connection-0".into(),
            states: vec![
                StateNode::Initial(InitialPseudo {
                    id: "ps-initial-Connection-0".into(),
                    target: "s-Connection-Idle".into(),
                    loc: loc(),
                }),
                simple(
                    "Connection",
                    "Idle",
                    vec![ev_transition(
                        "Connection",
                        0,
                        "s-Connection-Idle",
                        "s-Connection-Handshake",
                        "CONNECT",
                    )],
                ),
                simple(
                    "Connection",
                    "Handshake",
                    vec![ev_transition(
                        "Connection",
                        1,
                        "s-Connection-Handshake",
                        "s-Connection-Established",
                        "ACK",
                    )],
                ),
                simple(
                    "Connection",
                    "Established",
                    vec![ev_transition(
                        "Connection",
                        2,
                        "s-Connection-Established",
                        "s-Connection-Done",
                        "ESTABLISHED",
                    )],
                ),
                StateNode::Final(FinalState {
                    id: "s-Connection-Done".into(),
                    stable_id: "M:Connection:state:Done".into(),
                    name: "Done".into(),
                    loc: loc(),
                }),
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
    }
}

/// `Device`: `initial Connecting`; `state Connecting is Connection { done ->
/// Online; on RECONNECT -> Connecting }`; `state Online { on RECONNECT ->
/// Connecting }`. The ref-state's own transitions ride on the
/// `SubmachineRef` (Doc 09 §4.11). Byte-shape-faithful to
/// `fsm generate --emit-ir examples/submachine/submachine.fsm`.
fn device_connection_ir() -> Ir {
    let connecting = StateNode::Submachine(SubmachineRef {
        id: "s-Device-Connecting".into(),
        stable_id: "M:Device:state:Connecting".into(),
        name: "Connecting".into(),
        submachine_id: "m-Connection".into(),
        entry_points: vec![],
        exit_points: vec![],
        // Document order from the example: `on RECONNECT` (t-Device-0)
        // then `done -> Online` (t-Device-1). The dispatcher sorts by
        // priority (equal here) then doc order — either fires correctly.
        transitions: vec![
            ev_transition(
                "Device",
                0,
                "s-Device-Connecting",
                "s-Device-Connecting",
                "RECONNECT",
            ),
            done_transition("Device", 1, "s-Device-Connecting", "s-Device-Online"),
        ],
        loc: loc(),
    });
    let online = simple(
        "Device",
        "Online",
        vec![ev_transition(
            "Device",
            2,
            "s-Device-Online",
            "s-Device-Connecting",
            "RECONNECT",
        )],
    );

    Ir {
        ir_version: "1.0.0".into(),
        source_hash: String::new(),
        source_files: vec!["submachine.fsm".into()],
        machines: vec![MachineObject {
            id: "m-Device".into(),
            stable_id: "M:Device".into(),
            name: "Device".into(),
            context: ContextSchema { fields: vec![] },
            events: vec![
                event("Device", "CONNECT"),
                event("Device", "ACK"),
                event("Device", "ESTABLISHED"),
                event("Device", "RECONNECT"),
            ],
            externs: vec![],
            root: RegionObject {
                id: "r-Device-root".into(),
                stable_id: None,
                name: "Device__root".into(),
                initial: "ps-initial-Device-0".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-initial-Device-0".into(),
                        target: "s-Device-Connecting".into(),
                        loc: loc(),
                    }),
                    connecting,
                    online,
                ],
                priority: 0,
                loc: loc(),
            },
            // W2b mirrors every `submachine` into the parent's `submachines`
            // so a `MachineObject` is a self-contained codegen unit; the
            // ref-state's `submachine_id` resolves here.
            submachines: vec![connection_template()],
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
        fs::write(dir.join(&f.path), &f.content).expect("write generated file");
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
    fs::write(dir.join("host_hal.c"), hal).expect("write host_hal.c");
}

/// Driver `main`: each non-zero exit code is a distinct failed assertion
/// about the *observable* submachine behaviour. State checks read the
/// public `Device_current_state` / `Connection_current_state`; the sub
/// member is reached through the generated struct (`d._sub_CONNECTING`),
/// proving the sub-instance is a real nested value member that actually
/// advances — not an inert leaf.
fn write_driver_main_c(dir: &Path) {
    let main = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Device.h"

/* `Connecting` (is Connection) has NO user entry/exit extern — codegen owns
 * the sub-instance lifecycle. `Online` and the Connection sub-template's own
 * Simple states keep the usual contract (action-less here). */
void Device_entry_ONLINE(Device_t *m) { (void)m; }
void Device_exit_ONLINE(Device_t *m)  { (void)m; }
void Connection_entry_IDLE(Connection_t *m)        { (void)m; }
void Connection_entry_HANDSHAKE(Connection_t *m)   { (void)m; }
void Connection_entry_ESTABLISHED(Connection_t *m) { (void)m; }
void Connection_exit_IDLE(Connection_t *m)         { (void)m; }
void Connection_exit_HANDSHAKE(Connection_t *m)    { (void)m; }
void Connection_exit_ESTABLISHED(Connection_t *m)  { (void)m; }

int main(void) {
    Device_t d;
    Device_init(&d);

    /* (1) init → Connecting; sub instantiated at implicit initial Idle. */
    if (Device_current_state(&d) != DEVICE_STATE_CONNECTING) {
        fprintf(stderr, "init: expected CONNECTING, got %d\n",
                (int)Device_current_state(&d));
        return 10;
    }
    if (Connection_current_state(&d._sub_CONNECTING) != CONNECTION_STATE_IDLE) {
        fprintf(stderr, "init: sub expected IDLE, got %d "
                "(uninstantiated => inert-leaf, the pre-W2d bug)\n",
                (int)Connection_current_state(&d._sub_CONNECTING));
        return 11;
    }

    /* (2) CONNECT not consumed by the parent → delegated: Idle->Handshake. */
    Device_Event_t connect = { .id = DEVICE_EVENT_CONNECT };
    Device_dispatch(&d, &connect);
    if (Device_current_state(&d) != DEVICE_STATE_CONNECTING) {
        fprintf(stderr, "CONNECT: parent must stay CONNECTING, got %d\n",
                (int)Device_current_state(&d));
        return 20;
    }
    if (Connection_current_state(&d._sub_CONNECTING) != CONNECTION_STATE_HANDSHAKE) {
        fprintf(stderr, "CONNECT: sub expected HANDSHAKE, got %d "
                "(IDLE => delegation never ran)\n",
                (int)Connection_current_state(&d._sub_CONNECTING));
        return 21;
    }

    /* ACK → Handshake->Established. */
    Device_Event_t ack = { .id = DEVICE_EVENT_ACK };
    Device_dispatch(&d, &ack);
    if (Connection_current_state(&d._sub_CONNECTING) != CONNECTION_STATE_ESTABLISHED) {
        fprintf(stderr, "ACK: sub expected ESTABLISHED, got %d\n",
                (int)Connection_current_state(&d._sub_CONNECTING));
        return 30;
    }
    if (Device_current_state(&d) != DEVICE_STATE_CONNECTING) {
        fprintf(stderr, "ACK: parent must stay CONNECTING, got %d\n",
                (int)Device_current_state(&d));
        return 31;
    }

    /* (3) ESTABLISHED → Established->Done(final). The sub reaching Final
     * makes the parent receive a synthetic completion firing
     * Connecting --done--> Online through the EXISTING machinery, with NO
     * external event. A single dispatch must leave us in Online. */
    Device_Event_t est = { .id = DEVICE_EVENT_ESTABLISHED };
    Device_dispatch(&d, &est);
    if (Device_current_state(&d) != DEVICE_STATE_ONLINE) {
        fprintf(stderr, "ESTABLISHED: sub-completion did NOT advance the parent: "
                "expected ONLINE, got %d (CONNECTING => `done -> Online` never "
                "fired => inert-leaf, the pre-W2d bug)\n",
                (int)Device_current_state(&d));
        return 40;
    }

    /* (4) RECONNECT from Online → Connecting with a FRESH sub at Idle
     * (the spent sub was at Done; re-init must reset it). */
    Device_Event_t recon = { .id = DEVICE_EVENT_RECONNECT };
    Device_dispatch(&d, &recon);
    if (Device_current_state(&d) != DEVICE_STATE_CONNECTING) {
        fprintf(stderr, "RECONNECT: expected CONNECTING, got %d\n",
                (int)Device_current_state(&d));
        return 50;
    }
    if (Connection_current_state(&d._sub_CONNECTING) != CONNECTION_STATE_IDLE) {
        fprintf(stderr, "RECONNECT: sub must be a FRESH instance at IDLE, got %d "
                "(DONE => stale sub not re-initialised)\n",
                (int)Connection_current_state(&d._sub_CONNECTING));
        return 51;
    }
    /* Fresh sub progresses again — proves re-init is a real fresh instance. */
    Device_dispatch(&d, &connect);
    Device_dispatch(&d, &ack);
    Device_dispatch(&d, &est);
    if (Device_current_state(&d) != DEVICE_STATE_ONLINE) {
        fprintf(stderr, "second cycle: expected ONLINE after re-run, got %d\n",
                (int)Device_current_state(&d));
        return 52;
    }

    /* (5) RECONNECT while IN Connecting — the ref-state's own
     * self-transition wins over delegation (transition-wins) and resets the
     * sub fresh (mirrors W2c's run_exit teardown→re-sync). */
    Device_dispatch(&d, &recon);     /* Online -> Connecting (fresh) */
    Device_dispatch(&d, &connect);   /* sub Idle -> Handshake */
    if (Connection_current_state(&d._sub_CONNECTING) != CONNECTION_STATE_HANDSHAKE) {
        fprintf(stderr, "pre-self-recon: sub expected HANDSHAKE, got %d\n",
                (int)Connection_current_state(&d._sub_CONNECTING));
        return 60;
    }
    Device_dispatch(&d, &recon);     /* Connecting --RECONNECT--> Connecting */
    if (Device_current_state(&d) != DEVICE_STATE_CONNECTING) {
        fprintf(stderr, "self-recon: expected CONNECTING, got %d\n",
                (int)Device_current_state(&d));
        return 61;
    }
    if (Connection_current_state(&d._sub_CONNECTING) != CONNECTION_STATE_IDLE) {
        fprintf(stderr, "self-recon: a ref-state self-transition must RESET the "
                "sub fresh to IDLE (transition-wins), got %d (HANDSHAKE => "
                "re-init-on-re-entry not applied)\n",
                (int)Connection_current_state(&d._sub_CONNECTING));
        return 62;
    }

    printf("OK\n");
    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write main.c");
}

fn run_submachine_acceptance(strategy: DispatchStrategy, label: &str) {
    if !gcc_available() {
        eprintln!("[submachine_codegen_runs:{label}] gcc not on PATH — skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(&device_connection_ir(), &cfg).expect("emit Device+Connection C");
    write_files(dir, &out);
    write_host_hal_c(dir);
    write_driver_main_c(dir);

    let exe = dir.join(format!("submachine_test_{label}"));
    let result = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "Device.c",
            "Connection.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");

    if !result.status.success() || !result.stderr.is_empty() {
        eprintln!(
            "=== generated Device.c ({label}) ===\n{}",
            out.find("Device.c").unwrap().content
        );
        eprintln!(
            "=== generated Connection.c ({label}) ===\n{}",
            out.find("Connection.c").unwrap().content
        );
        eprintln!(
            "=== gcc stdout ===\n{}",
            String::from_utf8_lossy(&result.stdout)
        );
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
        panic!("gcc failed [{label}]: status={:?}", result.status.code());
    }

    let run = Command::new(&exe).output().expect("run submachine_test");
    if !run.status.success() {
        eprintln!(
            "=== generated Device.c ({label}) ===\n{}",
            out.find("Device.c").unwrap().content
        );
        panic!(
            "submachine_test binary FAILED [{label}]: exit={:?} (see the matching \
             return code in main.c — this is the inert-leaf regression on `main`), \
             stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}

#[test]
fn submachine_subinstance_runs_and_completion_advances_parent_switch_strategy() {
    run_submachine_acceptance(DispatchStrategy::Switch, "switch");
}

#[test]
fn submachine_subinstance_runs_and_completion_advances_parent_table_strategy() {
    run_submachine_acceptance(DispatchStrategy::Table, "table");
}
