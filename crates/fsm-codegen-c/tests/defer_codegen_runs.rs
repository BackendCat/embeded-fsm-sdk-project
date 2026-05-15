//! §5.4 behavioural-acceptance — `defer EVENT` codegen actually works.
//!
//! Replaces the retired `defer_codegen_unreachable.rs` (which asserted
//! codegen PANICS on a defer-bearing IR — the v1.0 audit P0-5 stopgap).
//! v1.1 ships a real deferred-event runtime, so the contract inverts: a
//! defer-bearing IR must emit C that, when COMPILED and RUN, defers and
//! replays correctly.
//!
//! This is NOT a symbol-presence test (SUBAGENT_CONVENTIONS §5.2 / §5.4).
//! It generates C for the Printer machine, compiles it with
//! `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, links a host HAL stub
//! + a driver `main`, EXECUTES the binary, and asserts the observable
//! behaviour the DSL `defer` *means*:
//!
//!   1. PRINT_JOB dispatched while in Maintenance is HELD (state stays
//!      Maintenance, `_deferred_count == 1`) — not dropped, not processed.
//!   2. On MAINT_DONE the machine returns to Idle, the held PRINT_JOB is
//!      released to the queue FRONT and reprocessed there, driving
//!      Idle → Printing. Final state = Printing, `_deferred_count == 0`.
//!   3. The deferred event is reprocessed EXACTLY ONCE (not dropped, not
//!      double-processed): a second JOB_DONE returns to Idle and there is
//!      no second phantom PRINT_JOB waiting.
//!   4. Transition-wins (UML 2.5.1 §14.2.3.9.1): from Idle (which consumes
//!      PRINT_JOB) the event transitions immediately and is never held,
//!      even though Maintenance declares `defer PRINT_JOB`.
//!
//! The Printer IR here is byte-for-byte the same machine the simulator
//! exercises in `fsm-simulator/tests/defer_replay.rs`, so this test plus
//! that one together prove sim ≡ codegen on the defer semantics.
//!
//! Regression contract (§5.1): on `main` the analyzer rejected `defer`
//! with FSM-E0903 and codegen's debug_assert refused a defer-bearing IR,
//! so `emit()` never produced runnable C — this test cannot pass there.
//! It passes only after the v1.1 defer-runtime wave.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig, DispatchStrategy};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    ContextSchema, DeferDecl, EventObject, InitialPseudo, Ir, MachineObject, OverflowPolicy,
    QueueConfig, RegionObject, SimpleState, StateNode, TransitionKind, TransitionObject, Trigger,
};

fn loc() -> SourceLocation {
    SourceLocation::new("deferred.fsm", Span::new(0, 1), 1, 1)
}

#[allow(deprecated)]
fn transition(id: &str, source: &str, target: &str, event_id: &str) -> TransitionObject {
    TransitionObject {
        id: id.into(),
        stable_id: format!("Printer:transition:{id}"),
        source: source.into(),
        target: target.into(),
        trigger: Some(Trigger::Event {
            event_id: event_id.into(),
            payload_binding: None,
        }),
        guard: None,
        actions: vec![],
        priority: 100,
        kind: TransitionKind::External,
        internal: false,
        loc: loc(),
    }
}

fn event(id: &str, name: &str) -> EventObject {
    EventObject {
        id: id.into(),
        stable_id: format!("Printer:event:{name}"),
        name: name.into(),
        payload: vec![],
        loc: loc(),
    }
}

fn simple(
    id: &str,
    name: &str,
    transitions: Vec<TransitionObject>,
    defers: Vec<DeferDecl>,
) -> StateNode {
    StateNode::Simple(SimpleState {
        id: id.into(),
        stable_id: format!("Printer:state:{name}"),
        name: name.into(),
        entry: vec![],
        exit: vec![],
        transitions,
        timers: vec![],
        defers,
        loc: loc(),
    })
}

/// The Printer: identical machine to `fsm-simulator/tests/defer_replay.rs`.
/// Idle —PRINT_JOB→ Printing, Idle —START_MAINT→ Maintenance,
/// Printing —JOB_DONE→ Idle, Maintenance { defer PRINT_JOB } —MAINT_DONE→ Idle.
fn printer_ir() -> Ir {
    let idle = simple(
        "s-idle",
        "Idle",
        vec![
            transition("t-idle-print", "s-idle", "s-printing", "e-print-job"),
            transition("t-idle-maint", "s-idle", "s-maintenance", "e-start-maint"),
        ],
        vec![],
    );
    let printing = simple(
        "s-printing",
        "Printing",
        vec![transition(
            "t-printing-done",
            "s-printing",
            "s-idle",
            "e-job-done",
        )],
        vec![],
    );
    let maintenance = simple(
        "s-maintenance",
        "Maintenance",
        vec![transition(
            "t-maint-done",
            "s-maintenance",
            "s-idle",
            "e-maint-done",
        )],
        // The deferring state.
        vec![DeferDecl {
            event_id: "e-print-job".into(),
            loc: loc(),
        }],
    );

    Ir {
        ir_version: "1.0.0".into(),
        source_hash: String::new(),
        source_files: vec!["deferred.fsm".into()],
        machines: vec![MachineObject {
            id: "m-printer".into(),
            stable_id: "Printer".into(),
            name: "Printer".into(),
            context: ContextSchema { fields: vec![] },
            events: vec![
                event("e-print-job", "PRINT_JOB"),
                event("e-start-maint", "START_MAINT"),
                event("e-maint-done", "MAINT_DONE"),
                event("e-job-done", "JOB_DONE"),
            ],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-initial-0".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-initial-0".into(),
                        target: "s-idle".into(),
                        loc: loc(),
                    }),
                    idle,
                    printing,
                    maintenance,
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

/// Driver `main`: each non-zero return code is a distinct failed assertion
/// about the *observable* defer/replay behaviour. The state-id checks read
/// the public `Printer_current_state`; `_deferred_count` is read directly
/// off the struct (it is part of the generated machine struct) to prove
/// the held event is actually in the buffer and then actually drained.
fn write_driver_main_c(dir: &Path) {
    let main = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Printer.h"

/* Action-less leaves — entry/exit are no-ops here; we test dispatch +
 * defer flow, not side effects. */
void Printer_entry_IDLE(Printer_t *m)        { (void)m; }
void Printer_entry_PRINTING(Printer_t *m)    { (void)m; }
void Printer_entry_MAINTENANCE(Printer_t *m) { (void)m; }
void Printer_exit_IDLE(Printer_t *m)         { (void)m; }
void Printer_exit_PRINTING(Printer_t *m)     { (void)m; }
void Printer_exit_MAINTENANCE(Printer_t *m)  { (void)m; }

int main(void) {
    Printer_t p;
    Printer_init(&p);

    if (Printer_current_state(&p) != PRINTER_STATE_IDLE) {
        fprintf(stderr, "init: expected IDLE, got %d\n", (int)Printer_current_state(&p));
        return 10;
    }

    /* ---- Transition-wins (UML 2.5.1 §14.2.3.9.1) ------------------------
     * From Idle, PRINT_JOB is consumed directly (Idle -> Printing). It
     * must NOT be deferred even though Maintenance declares
     * `defer PRINT_JOB` — Maintenance is not in the active config. */
    Printer_Event_t print_job = { .id = PRINTER_EVENT_PRINT_JOB };
    Printer_dispatch(&p, &print_job);
    if (Printer_current_state(&p) != PRINTER_STATE_PRINTING) {
        fprintf(stderr, "transition-wins: expected PRINTING, got %d\n",
                (int)Printer_current_state(&p));
        return 11;
    }
    if (p._deferred_count != 0u) {
        fprintf(stderr, "transition-wins: nothing should be deferred, count=%u\n",
                (unsigned)p._deferred_count);
        return 12;
    }

    /* Back to Idle for the deferral scenario. */
    Printer_Event_t job_done = { .id = PRINTER_EVENT_JOB_DONE };
    Printer_dispatch(&p, &job_done);
    if (Printer_current_state(&p) != PRINTER_STATE_IDLE) {
        fprintf(stderr, "JOB_DONE: expected IDLE, got %d\n",
                (int)Printer_current_state(&p));
        return 13;
    }

    /* ---- Defer + replay round trip ------------------------------------- */

    /* Enter Maintenance. */
    Printer_Event_t start_maint = { .id = PRINTER_EVENT_START_MAINT };
    Printer_dispatch(&p, &start_maint);
    if (Printer_current_state(&p) != PRINTER_STATE_MAINTENANCE) {
        fprintf(stderr, "START_MAINT: expected MAINTENANCE, got %d\n",
                (int)Printer_current_state(&p));
        return 20;
    }

    /* PRINT_JOB while in Maintenance: no transition consumes it there,
     * Maintenance defers it. It must be HELD — state unchanged, and the
     * event physically sitting in the defer buffer (NOT dropped). */
    Printer_dispatch(&p, &print_job);
    if (Printer_current_state(&p) != PRINTER_STATE_MAINTENANCE) {
        fprintf(stderr, "deferred PRINT_JOB moved the machine; state=%d\n",
                (int)Printer_current_state(&p));
        return 21;
    }
    if (p._deferred_count != 1u) {
        fprintf(stderr, "PRINT_JOB must be HELD: expected _deferred_count=1, got %u "
                "(0 => dropped, the v1.0 bug)\n", (unsigned)p._deferred_count);
        return 22;
    }

    /* MAINT_DONE: Maintenance -> Idle. On exit from the (last) deferring
     * state, the held PRINT_JOB is released to the FRONT of the queue and
     * reprocessed in Idle, where Idle -> Printing consumes it. So a single
     * MAINT_DONE dispatch must leave us in PRINTING with an empty buffer. */
    Printer_Event_t maint_done = { .id = PRINTER_EVENT_MAINT_DONE };
    Printer_dispatch(&p, &maint_done);
    if (Printer_current_state(&p) != PRINTER_STATE_PRINTING) {
        fprintf(stderr, "deferred PRINT_JOB was NOT replayed: expected PRINTING, "
                "got %d (IDLE => event silently lost on release)\n",
                (int)Printer_current_state(&p));
        return 23;
    }
    if (p._deferred_count != 0u) {
        fprintf(stderr, "defer buffer not drained after release: count=%u\n",
                (unsigned)p._deferred_count);
        return 24;
    }

    /* ---- Exactly-once: the replayed event is not double-processed ------
     * One JOB_DONE returns Printing -> Idle. If the deferred PRINT_JOB had
     * been reprocessed twice (or a phantom copy lingered) we would bounce
     * back into Printing here. We must come to rest in Idle. */
    Printer_dispatch(&p, &job_done);
    if (Printer_current_state(&p) != PRINTER_STATE_IDLE) {
        fprintf(stderr, "double-process: expected IDLE after one JOB_DONE, got %d\n",
                (int)Printer_current_state(&p));
        return 30;
    }
    if (p._deferred_count != 0u) {
        fprintf(stderr, "double-process: stray deferred events remain, count=%u\n",
                (unsigned)p._deferred_count);
        return 31;
    }

    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write main.c");
}

fn run_defer_acceptance(strategy: DispatchStrategy, label: &str) {
    if !gcc_available() {
        eprintln!("[defer_codegen_runs:{label}] gcc not on PATH — skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(&printer_ir(), &cfg).expect("emit Printer C");
    write_files(dir, &out);
    write_host_hal_c(dir);
    write_driver_main_c(dir);

    let exe = dir.join(format!("defer_test_{label}"));
    let result = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "Printer.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");

    if !result.status.success() || !result.stderr.is_empty() {
        eprintln!(
            "=== generated Printer.c ({label}) ===\n{}",
            out.find("Printer.c").unwrap().content
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

    let run = Command::new(&exe).output().expect("run defer_test");
    if !run.status.success() {
        eprintln!(
            "=== generated Printer.c ({label}) ===\n{}",
            out.find("Printer.c").unwrap().content
        );
        panic!(
            "defer_test binary FAILED [{label}]: exit={:?} (see assertion code in main.c), \
             stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}

#[test]
fn deferred_event_is_held_and_replayed_switch_strategy() {
    run_defer_acceptance(DispatchStrategy::Switch, "switch");
}

#[test]
fn deferred_event_is_held_and_replayed_table_strategy() {
    run_defer_acceptance(DispatchStrategy::Table, "table");
}

// ---------------------------------------------------------------------------
// Doc 08 §10.4 — recursion prevention. A composite `Outer` defers PING. An
// inner A→B transition changes the configuration but `Outer` stays active,
// so the held PING must NOT be released (it would be immediately re-deferred
// — a churn loop). Only exiting `Outer` releases it. This proves the
// generated `Motor_release_deferred` honours the "still covered by an active
// deferring ancestor → keep" partition, and that `Motor_active_config_defers`
// walks leaf-to-root through the parent table (not just the leaf).
// ---------------------------------------------------------------------------

#[allow(deprecated)]
fn composite(
    id: &str,
    name: &str,
    transitions: Vec<TransitionObject>,
    defers: Vec<DeferDecl>,
    region: RegionObject,
) -> StateNode {
    use fsm_ir::CompositeState;
    StateNode::Composite(CompositeState {
        id: id.into(),
        stable_id: format!("Nested:state:{name}"),
        name: name.into(),
        entry: vec![],
        exit: vec![],
        transitions,
        timers: vec![],
        defers,
        regions: vec![region],
        history: None,
        loc: loc(),
    })
}

/// `Nested`: Outer composite { defer PING } with inner A —GO→ B; Outer
/// —DONE→ Done. Outer remains active across the A→B step.
fn nested_defer_ir() -> Ir {
    let a = simple(
        "s-a",
        "A",
        vec![transition("t-a-go", "s-a", "s-b", "e-go")],
        vec![],
    );
    let b = simple("s-b", "B", vec![], vec![]);
    let inner = RegionObject {
        id: "r-inner".into(),
        stable_id: None,
        name: "Inner".into(),
        initial: "ps-inner-init".into(),
        states: vec![
            StateNode::Initial(InitialPseudo {
                id: "ps-inner-init".into(),
                target: "s-a".into(),
                loc: loc(),
            }),
            a,
            b,
        ],
        priority: 0,
        loc: loc(),
    };
    let outer = composite(
        "s-outer",
        "Outer",
        vec![transition("t-outer-done", "s-outer", "s-done", "e-done")],
        vec![DeferDecl {
            event_id: "e-ping".into(),
            loc: loc(),
        }],
        inner,
    );
    let done = simple("s-done", "Done", vec![], vec![]);

    Ir {
        ir_version: "1.0.0".into(),
        source_hash: String::new(),
        source_files: vec!["nested.fsm".into()],
        machines: vec![MachineObject {
            id: "m-nested".into(),
            stable_id: "Nested".into(),
            name: "Nested".into(),
            context: ContextSchema { fields: vec![] },
            events: vec![
                event("e-ping", "PING"),
                event("e-go", "GO"),
                event("e-done", "DONE"),
            ],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-root-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-root-init".into(),
                        target: "s-outer".into(),
                        loc: loc(),
                    }),
                    outer,
                    done,
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

fn write_nested_main_c(dir: &Path) {
    let main = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Nested.h"

void Nested_entry_OUTER(Nested_t *m) { (void)m; }
void Nested_entry_A(Nested_t *m)     { (void)m; }
void Nested_entry_B(Nested_t *m)     { (void)m; }
void Nested_entry_DONE(Nested_t *m)  { (void)m; }
void Nested_exit_OUTER(Nested_t *m)  { (void)m; }
void Nested_exit_A(Nested_t *m)      { (void)m; }
void Nested_exit_B(Nested_t *m)      { (void)m; }
void Nested_exit_DONE(Nested_t *m)   { (void)m; }

int main(void) {
    Nested_t m;
    Nested_init(&m);

    /* Initial config: Outer/A (composite entered, inner initial = A). */
    if (Nested_current_state(&m) != NESTED_STATE_A) {
        fprintf(stderr, "init: expected A, got %d\n", (int)Nested_current_state(&m));
        return 40;
    }

    /* PING: no transition consumes it; Outer (an active ANCESTOR of A)
     * defers it. Held — proves active_config_defers walks leaf-to-root. */
    Nested_Event_t ping = { .id = NESTED_EVENT_PING };
    Nested_dispatch(&m, &ping);
    if (m._deferred_count != 1u) {
        fprintf(stderr, "PING not held via ancestor defer: count=%u\n",
                (unsigned)m._deferred_count);
        return 41;
    }

    /* GO: A -> B. Configuration changes but Outer REMAINS active, so it
     * still defers PING. §10.4: the held PING must NOT be released (that
     * would churn — release then immediately re-defer). Buffer stays at 1. */
    Nested_Event_t go = { .id = NESTED_EVENT_GO };
    Nested_dispatch(&m, &go);
    if (Nested_current_state(&m) != NESTED_STATE_B) {
        fprintf(stderr, "GO: expected B, got %d\n", (int)Nested_current_state(&m));
        return 42;
    }
    if (m._deferred_count != 1u) {
        fprintf(stderr, "§10.4 violated: PING released while deferring ancestor "
                "still active (count=%u, expected 1)\n", (unsigned)m._deferred_count);
        return 43;
    }

    /* DONE: Outer -> Done. Outer (the only deferring state) is exited, so
     * PING is released to the queue front. Done has no PING transition, so
     * the released PING discards. Buffer drains to 0. */
    Nested_Event_t done = { .id = NESTED_EVENT_DONE };
    Nested_dispatch(&m, &done);
    if (Nested_current_state(&m) != NESTED_STATE_DONE) {
        fprintf(stderr, "DONE: expected DONE, got %d\n", (int)Nested_current_state(&m));
        return 44;
    }
    if (m._deferred_count != 0u) {
        fprintf(stderr, "PING not released on Outer exit: count=%u\n",
                (unsigned)m._deferred_count);
        return 45;
    }

    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write nested main.c");
}

fn run_nested_defer_acceptance(strategy: DispatchStrategy, label: &str) {
    if !gcc_available() {
        eprintln!("[defer_codegen_runs:nested:{label}] gcc not on PATH — skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(&nested_defer_ir(), &cfg).expect("emit Nested C");
    write_files(dir, &out);
    write_host_hal_c(dir);
    write_nested_main_c(dir);

    let exe = dir.join(format!("nested_defer_test_{label}"));
    let result = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "Nested.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");

    if !result.status.success() || !result.stderr.is_empty() {
        eprintln!(
            "=== generated Nested.c ({label}) ===\n{}",
            out.find("Nested.c").unwrap().content
        );
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
        panic!(
            "gcc failed [nested:{label}]: status={:?}",
            result.status.code()
        );
    }

    let run = Command::new(&exe).output().expect("run nested_defer_test");
    if !run.status.success() {
        eprintln!(
            "=== generated Nested.c ({label}) ===\n{}",
            out.find("Nested.c").unwrap().content
        );
        panic!(
            "nested_defer_test FAILED [{label}]: exit={:?} (see assertion code in main.c), \
             stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}

#[test]
fn ancestor_deferred_event_not_churned_on_inner_transition_switch_strategy() {
    run_nested_defer_acceptance(DispatchStrategy::Switch, "switch");
}

#[test]
fn ancestor_deferred_event_not_churned_on_inner_transition_table_strategy() {
    run_nested_defer_acceptance(DispatchStrategy::Table, "table");
}
