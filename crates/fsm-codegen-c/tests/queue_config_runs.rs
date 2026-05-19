//! §5.4 behavioural-acceptance — Finding F-2: the in-source `queue {}`
//! config (capacity + overflow policy) is HONORED by codegen end-to-end.
//!
//! This is NOT a symbol-presence test (SUBAGENT_CONVENTIONS §5.2 / §5.4):
//! it builds an IR carrying an explicit, NON-DEFAULT `QueueConfig`
//! (`capacity = 4`, default codegen fallback is `8`), generates C,
//! compiles it with `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, links
//! a host HAL stub + a driver `main`, EXECUTES the binary, and asserts the
//! observable runtime behaviour the DSL `queue {}` *means*:
//!
//!   1. The ring buffer holds exactly the IN-SOURCE capacity (4), NOT the
//!      codegen default (8): `_post` 4 events all succeed; the buffer is
//!      then full. Pre-F-2 the lowerer dropped the in-source values and
//!      codegen emitted capacity 8, so a 5th post still fit — the silent
//!      miscompile this test exists to catch.
//!   2. The IN-SOURCE overflow policy is in effect on the 5th post:
//!      - `FSM_QUEUE_DROP_NEWEST` (from DSL `overflow = drop_newest`):
//!        the 5th post is silently dropped, count stays 4, and the four
//!        held events are the FIRST four (FIFO order preserved).
//!      - `FSM_QUEUE_ASSERT` (from DSL `overflow = assert`): the 5th post
//!        trips `fsm_hal_assert` and `abort()`s — proven by running a
//!        forked child and asserting it dies on the overflowing post but
//!        NOT on the four that fit.
//!
//! The C99 runtime ships exactly two overflow paths (Doc 11 §6 /
//! `OverflowPolicy::from_ir`): assert-or-drop. `DropOldest`/`Error` map
//! onto those two — that collapse is a documented design boundary, not a
//! defect, so this test exercises the two C-realizable policies. The
//! sim≡codegen differential for the *configured* queue lives in
//! `fsm-simulator/tests/codegen_equivalence_smoke.rs` (the `queue-overflow`
//! corpus fixture); together they prove the IR `QueueConfig` drives BOTH
//! backends off the SAME source of truth.
//!
//! Regression contract (§5.1): on `main` (pre-F-2) `lower_queue` parsed the
//! *key* `Ident` instead of the value, so `capacity = 4` silently became
//! the default 16 and `overflow = drop_newest` the default `Assert`; AND
//! codegen read `config.queue_capacity` (8), ignoring the IR entirely.
//! Either defect makes assertion (1) fail (a 5th post fits). This test
//! passes only after the F-2 lowerer + codegen wiring.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig, DispatchStrategy};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    ContextSchema, EventObject, InitialPseudo, Ir, MachineObject, OverflowPolicy, QueueConfig,
    RegionObject, SimpleState, StateNode, TransitionKind, TransitionObject, Trigger,
};

fn loc() -> SourceLocation {
    // A real `.fsm` path (NOT the `<default>` sentinel) so
    // `QueueConfig::is_explicit()` is true — i.e. this models an in-source
    // `queue {}` block, exactly the F-2 case.
    SourceLocation::new("queue.fsm", Span::new(0, 1), 1, 1)
}

#[allow(deprecated)]
fn transition(id: &str, source: &str, target: &str, event_id: &str) -> TransitionObject {
    TransitionObject {
        id: id.into(),
        stable_id: format!("Buf:transition:{id}"),
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
        hint: None,
        loc: loc(),
    }
}

fn event(id: &str, name: &str) -> EventObject {
    EventObject {
        id: id.into(),
        stable_id: format!("Buf:event:{name}"),
        name: name.into(),
        payload: vec![],
        loc: loc(),
    }
}

fn simple(id: &str, name: &str, transitions: Vec<TransitionObject>) -> StateNode {
    StateNode::Simple(SimpleState {
        id: id.into(),
        stable_id: format!("Buf:state:{name}"),
        name: name.into(),
        entry: vec![],
        exit: vec![],
        transitions,
        timers: vec![],
        defers: vec![],
        loc: loc(),
    })
}

/// `Buf`: a minimal two-state machine whose ONLY F-2-relevant content is an
/// explicit non-default `QueueConfig`. The state graph (S0 —TICK→ S1 —STOP→
/// S0) is incidental — the test never dispatches; it drives `_post` /
/// `_dequeue` directly to exercise the ring buffer at its declared bound.
fn buf_ir(capacity: u32, overflow: OverflowPolicy) -> Ir {
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: String::new(),
        source_files: vec!["queue.fsm".into()],
        machines: vec![MachineObject {
            id: "m-buf".into(),
            stable_id: "Buf".into(),
            name: "Buf".into(),
            context: ContextSchema { fields: vec![] },
            events: vec![event("e-tick", "TICK"), event("e-stop", "STOP")],
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-initial-0".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-initial-0".into(),
                        target: "s-0".into(),
                        loc: loc(),
                    }),
                    simple(
                        "s-0",
                        "S0",
                        vec![transition("t-0-1", "s-0", "s-1", "e-tick")],
                    ),
                    simple(
                        "s-1",
                        "S1",
                        vec![transition("t-1-0", "s-1", "s-0", "e-stop")],
                    ),
                ],
                priority: 0,
                loc: loc(),
            },
            submachines: vec![],
            consts: vec![],
            imports: vec![],
            features: vec![],
            // The F-2 payload: an EXPLICIT, non-default in-source queue
            // config. `is_explicit()` is true (real file in `loc`), so
            // `resolve_queue` (no integrator override here) MUST flow
            // these values to codegen.
            queue: QueueConfig {
                capacity,
                overflow_policy: overflow,
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
    // `fsm_hal_assert` aborts on a false condition (the Doc 16 host stub
    // contract) — that is exactly how the ASSERT overflow policy is
    // observed: an overflowing post under FSM_QUEUE_ASSERT must abort.
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

/// Driver for the DROP_NEWEST policy. Posts CAP+2 events; asserts the ring
/// held exactly CAP (the IN-SOURCE capacity, not the codegen default) and
/// that the survivors are the FIRST CAP in FIFO order (drop-newest drops
/// the overflowing tail, never the head). Each non-zero return is a
/// distinct failed sub-assertion.
fn write_drop_main_c(dir: &Path, capacity: u32) {
    let main = format!(
        r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Buf.h"

void Buf_entry_S0(Buf_t *m) {{ (void)m; }}
void Buf_entry_S1(Buf_t *m) {{ (void)m; }}
void Buf_exit_S0(Buf_t *m)  {{ (void)m; }}
void Buf_exit_S1(Buf_t *m)  {{ (void)m; }}

int main(void) {{
    Buf_t m;
    Buf_init(&m);

    /* The macro MUST equal the in-source capacity, not the codegen
     * default (8). A compile-time guard makes the F-2 silent-miscompile a
     * BUILD failure too (defense-in-depth atop the runtime checks). */
    #if BUF_QUEUE_CAPACITY != {cap}u
    #error "BUF_QUEUE_CAPACITY != in-source queue capacity — F-2 regression"
    #endif

    /* Post exactly CAP events: all must fit (count rises 1..CAP). We tag
     * each with the TICK id; payloadless events are indistinguishable, so
     * we instead verify ORDER via the dequeue id sequence below — here we
     * just prove capacity == CAP, not CAP-2 and not the default 8. */
    Buf_Event_t tick = {{ .id = BUF_EVENT_TICK }};
    Buf_Event_t stop = {{ .id = BUF_EVENT_STOP }};

    for (unsigned i = 0u; i < {cap}u; i++) {{
        /* Alternate ids so the FIFO survivor order is observable. */
        Buf_post(&m, (i % 2u == 0u) ? &tick : &stop);
    }}
    if (m._queue_count != {cap}u) {{
        fprintf(stderr, "after CAP posts: expected count=%u, got %u "
                "(8 => codegen ignored in-source queue{{}}, the F-2 bug)\n",
                {cap}u, (unsigned)m._queue_count);
        return 10;
    }}

    /* Two more posts must OVERFLOW. DROP_NEWEST: silently dropped, count
     * unchanged, the held set untouched. */
    Buf_post(&m, &tick);
    Buf_post(&m, &stop);
    if (m._queue_count != {cap}u) {{
        fprintf(stderr, "drop_newest: overflow must NOT grow the ring; "
                "expected count=%u, got %u\n", {cap}u, (unsigned)m._queue_count);
        return 11;
    }}

    /* Dequeue all CAP: must come out in the ORIGINAL post order
     * (tick, stop, tick, stop, ...) — drop-newest preserves the head, so
     * the overflowing posts never displaced the four that fit. */
    for (unsigned i = 0u; i < {cap}u; i++) {{
        Buf_Event_t out;
        if (!Buf_dequeue(&m, &out)) {{
            fprintf(stderr, "dequeue %u: ring unexpectedly empty\n", i);
            return 20;
        }}
        int want = (i % 2u == 0u) ? (int)BUF_EVENT_TICK : (int)BUF_EVENT_STOP;
        if ((int)out.id != want) {{
            fprintf(stderr, "dequeue %u: FIFO order broken — want id=%d got id=%d "
                    "(drop_newest must keep the head)\n", i, want, (int)out.id);
            return 21;
        }}
    }}
    if (m._queue_count != 0u) {{
        fprintf(stderr, "after draining CAP: expected empty, count=%u\n",
                (unsigned)m._queue_count);
        return 22;
    }}

    return 0;
}}
"#,
        cap = capacity
    );
    fs::write(dir.join("main.c"), main).expect("write drop main.c");
}

/// Driver for the ASSERT policy. The CAP posts that fit must NOT abort; the
/// (CAP+1)th post MUST abort (via `fsm_hal_assert`). The test harness runs
/// the binary and asserts a NON-zero exit (the abort) — and a separate
/// "only CAP posts" build that must exit 0.
fn write_assert_main_c(dir: &Path, capacity: u32, overflow_on_full: bool) {
    // `overflow_on_full == false`: post exactly CAP (must NOT assert →
    // exit 0).  `true`: post CAP+1 (the last MUST assert → abort).
    let posts = if overflow_on_full {
        capacity + 1
    } else {
        capacity
    };
    let main = format!(
        r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Buf.h"

void Buf_entry_S0(Buf_t *m) {{ (void)m; }}
void Buf_entry_S1(Buf_t *m) {{ (void)m; }}
void Buf_exit_S0(Buf_t *m)  {{ (void)m; }}
void Buf_exit_S1(Buf_t *m)  {{ (void)m; }}

int main(void) {{
    Buf_t m;
    Buf_init(&m);

    #if BUF_QUEUE_CAPACITY != {cap}u
    #error "BUF_QUEUE_CAPACITY != in-source queue capacity — F-2 regression"
    #endif

    Buf_Event_t tick = {{ .id = BUF_EVENT_TICK }};
    for (unsigned i = 0u; i < {posts}u; i++) {{
        Buf_post(&m, &tick);   /* the {posts}th post overflows iff posts>CAP */
    }}

    /* Reached only when NO assert fired (posts <= CAP). */
    if (m._queue_count != {cap}u) {{
        fprintf(stderr, "assert-policy: expected full ring count=%u, got %u\n",
                {cap}u, (unsigned)m._queue_count);
        return 30;
    }}
    return 0;
}}
"#,
        cap = capacity,
        posts = posts
    );
    fs::write(dir.join("main.c"), main).expect("write assert main.c");
}

fn compile(dir: &Path, machine_c: &str, exe: &Path, out: &fsm_codegen_c::EmittedFiles) {
    let result = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            machine_c,
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(exe)
        .output()
        .expect("invoke gcc");
    if !result.status.success() || !result.stderr.is_empty() {
        eprintln!(
            "=== generated {machine_c} ===\n{}",
            out.find(machine_c)
                .map(|f| f.content.as_str())
                .unwrap_or("")
        );
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
        panic!("gcc failed: status={:?}", result.status.code());
    }
}

const CAP: u32 = 4;

fn run_drop_newest(strategy: DispatchStrategy, label: &str) {
    if !gcc_available() {
        eprintln!("[queue_config_runs:drop:{label}] gcc not on PATH — skipping");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    // No integrator override → `resolve_queue` MUST honor the in-source
    // QueueConfig (capacity 4, drop_newest).
    let out = emit(&buf_ir(CAP, OverflowPolicy::DropNewest), &cfg).expect("emit Buf C");
    write_files(dir, &out);
    write_host_hal_c(dir);
    write_drop_main_c(dir, CAP);

    let exe = dir.join(format!("queue_drop_{label}"));
    compile(dir, "Buf.c", &exe, &out);

    let run = Command::new(&exe).output().expect("run queue_drop");
    if !run.status.success() {
        eprintln!(
            "=== generated Buf.c ({label}) ===\n{}",
            out.find("Buf.c").unwrap().content
        );
        panic!(
            "queue_drop binary FAILED [{label}]: exit={:?} (see main.c sub-assertion), stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}

fn run_assert_policy(strategy: DispatchStrategy, label: &str) {
    if !gcc_available() {
        eprintln!("[queue_config_runs:assert:{label}] gcc not on PATH — skipping");
        return;
    }
    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(&buf_ir(CAP, OverflowPolicy::Assert), &cfg).expect("emit Buf C");

    // (a) Exactly CAP posts → no assert → exit 0.
    {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        write_files(dir, &out);
        write_host_hal_c(dir);
        write_assert_main_c(dir, CAP, false);
        let exe = dir.join(format!("queue_assert_ok_{label}"));
        compile(dir, "Buf.c", &exe, &out);
        let run = Command::new(&exe).output().expect("run queue_assert_ok");
        assert!(
            run.status.success(),
            "assert-policy [{label}]: CAP posts must NOT abort, exit={:?} stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }

    // (b) CAP+1 posts → the overflowing post MUST trip fsm_hal_assert →
    // the process abort()s (non-zero / signal exit). This is the
    // observable ASSERT overflow behaviour.
    {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        write_files(dir, &out);
        write_host_hal_c(dir);
        write_assert_main_c(dir, CAP, true);
        let exe = dir.join(format!("queue_assert_overflow_{label}"));
        compile(dir, "Buf.c", &exe, &out);
        let run = Command::new(&exe)
            .output()
            .expect("run queue_assert_overflow");
        assert!(
            !run.status.success(),
            "assert-policy [{label}]: the (CAP+1)th post MUST abort under \
             FSM_QUEUE_ASSERT, but the process exited 0 (the in-source \
             overflow=assert was NOT honored — F-2 regression)"
        );
    }
}

#[test]
fn in_source_queue_capacity_and_drop_newest_honored_switch() {
    run_drop_newest(DispatchStrategy::Switch, "switch");
}

#[test]
fn in_source_queue_capacity_and_drop_newest_honored_table() {
    run_drop_newest(DispatchStrategy::Table, "table");
}

#[test]
fn in_source_queue_assert_policy_honored_switch() {
    run_assert_policy(DispatchStrategy::Switch, "switch");
}

#[test]
fn in_source_queue_assert_policy_honored_table() {
    run_assert_policy(DispatchStrategy::Table, "table");
}
