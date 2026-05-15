//! TD-BUG-1 — a valid zero-transition machine must emit `-Werror`-clean C.
//!
//! ## The bug (per `docs/processes/TEST_DEBT.md` TD-BUG-1)
//!
//! A machine with ≥1 state and **zero transitions** is valid DSL (Doc 04
//! §3 — states may carry only entry/exit actions, or be a placeholder
//! skeleton). Pre-fix, codegen emitted
//! `M_try_transitions_in_state(M_t *m, M_StateId_t s, const M_Event_t *ev)`
//! whose body was an empty `switch (s)` that referenced neither `m` nor
//! `ev`, so the generated C failed the project's own MVP-gate-G4 compile
//! `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`
//! (`-Werror=unused-parameter`). The table strategy had the same class of
//! defect three ways over: an empty `M_trans_table[] = {}` (ISO-C empty
//! initializer + zero-size array), an unused `m` in
//! `M_execute_transition`, and a `unsigned i < TABLE_SIZE(==0)` always-
//! false loop bound (`-Werror=type-limits`).
//!
//! It was masked for the life of `context_defaults_emitted.rs` because
//! that file's only coverage was symbol-presence `.contains()` that never
//! invoked gcc — the exact P0-1-class hazard `SUBAGENT_CONVENTIONS.md`
//! §5.4 exists to kill. W0's behavioural-acceptance discipline surfaced
//! it (W0 worked around it with a one self-transition fixture).
//!
//! ## Test strategy (§5.4 behavioural acceptance — all four steps)
//!
//! For **both** dispatch strategies:
//! 1. Build the IR for a minimal valid machine: one state with an entry
//!    extern, **zero** `on`/`after`/`done` transitions.
//! 2. `emit()` → write the files → compile + link a host HAL stub with
//!    `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`.
//! 3. **Run** the binary: `M_init` then dispatch an arbitrary event.
//! 4. Assert it does not crash and stays in the initial state (a
//!    zero-transition machine can never leave its initial state — that is
//!    what "no transitions" *means*).
//!
//! These tests FAIL on `main` (gcc rejects the generated C, so the build
//! step panics) and PASS after the TD-BUG-1 fix.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig, DispatchStrategy};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    ContextSchema, EventObject, ExternObject, InitialPseudo, Ir, MachineObject, OverflowPolicy,
    QueueConfig, RegionObject, SimpleState, StateNode, Statement,
};

fn loc() -> SourceLocation {
    SourceLocation::new("beacon.fsm", Span::new(0, 1), 1, 1)
}

/// Minimal valid machine: a single state `Idle` whose only behaviour is an
/// entry action, and an event it never consumes. **Zero transitions** — no
/// `on`, no `after`, no `done`. This is the exact degenerate-but-valid
/// shape TD-BUG-1 is about; nothing here is a workaround (cf.
/// `context_defaults_emitted::vending_with_bool_default_ir`, which had to
/// add a self-transition precisely to dodge this bug).
fn zero_transition_ir() -> Ir {
    let idle = SimpleState {
        id: "s-idle".into(),
        stable_id: "Beacon:state:Idle".into(),
        name: "Idle".into(),
        // Entry action: one extern call. Note codegen renders entry/exit
        // *bodies* as user-implemented hook functions (`Beacon_entry_IDLE`
        // declared `extern` in the impl header) — the statement here makes
        // the fixture the canonical "state with only an entry action, no
        // transitions" shape TD-BUG-1 describes; the hook is stubbed in
        // the test's `main.c`.
        entry: vec![Statement::Call {
            callee: "blink".into(),
            args: vec![],
        }],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    };
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "".into(),
        source_files: vec!["beacon.fsm".into()],
        machines: vec![MachineObject {
            id: "m-beacon".into(),
            stable_id: "Beacon".into(),
            name: "Beacon".into(),
            context: ContextSchema { fields: vec![] },
            // One event the machine never consumes — exercises the event
            // enum + the dispatch path with a non-completion `ev->id`.
            events: vec![EventObject {
                id: "ev-poke".into(),
                stable_id: "Beacon:event:POKE".into(),
                name: "POKE".into(),
                payload: vec![],
                loc: loc(),
            }],
            externs: vec![ExternObject {
                id: "ext-blink".into(),
                stable_id: "Beacon:extern:blink".into(),
                name: "blink".into(),
                pure: false,
                params: vec![],
                return_type: None,
                loc: loc(),
            }],
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
                    StateNode::Simple(idle),
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

/// Shared driver: emit `zero_transition_ir()` under `strategy`, gcc-Werror
/// compile + link a host HAL stub, RUN it, assert it stays in `Idle`.
fn zero_transition_compiles_runs_and_stays_in_initial(strategy: DispatchStrategy, tag: &str) {
    if !gcc_available() {
        eprintln!("[zero_transition_werror/{tag}] gcc not on PATH — skipping");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir: &Path = tmp.path();

    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(&zero_transition_ir(), &cfg).expect("emit");
    for f in &out.files {
        fs::write(dir.join(&f.path), &f.content).expect("write generated");
    }
    fs::write(
        dir.join("host_hal.c"),
        "#include <stdint.h>\n#include <stdbool.h>\n#include <stdio.h>\n#include <stdlib.h>\n\
         uint32_t fsm_hal_clock_now_ms(void){return 0;}\n\
         void fsm_hal_assert(bool c,const char*m){if(!c){fprintf(stderr,\"%s\",m);abort();}}\n",
    )
    .expect("write hal");

    // Drive: init, then post an event the machine has no transition for.
    // A zero-transition machine MUST remain in its initial state (`Idle`)
    // — there is no edge to take. The `blink` entry extern must have run
    // exactly once (on entry into `Idle` at init).
    // Codegen renders entry/exit *bodies* as user-implemented hooks (it
    // emits the `Beacon_entry_IDLE(m)` call into `Beacon.c`, never the
    // statement body). So the observable here is the entry-HOOK firing,
    // not the extern inside it: it must run exactly once at init (entering
    // `Idle`) and NOT again when a no-op event is dispatched (a zero-
    // transition machine can never leave `Idle`).
    let main = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Beacon.h"

static int g_entry_idle = 0;
static int g_exit_idle  = 0;
void Beacon_entry_IDLE(Beacon_t *m) { (void)m; g_entry_idle++; }
void Beacon_exit_IDLE(Beacon_t *m)  { (void)m; g_exit_idle++;  }

int main(void) {
    Beacon_t m;
    Beacon_init(&m);

    if (Beacon_current_state(&m) != BEACON_STATE_IDLE) {
        fprintf(stderr, "post-init: expected to be in Idle, got %d\n",
                (int)Beacon_current_state(&m));
        return 1;
    }
    if (g_entry_idle != 1) {
        fprintf(stderr, "init: expected entry(Idle) once, got %d\n",
                g_entry_idle);
        return 2;
    }

    /* Dispatch an event the machine never consumes. A zero-transition
     * machine must not crash and must not move. */
    Beacon_Event_t ev;
    ev.id = BEACON_EVENT_POKE;
    Beacon_dispatch(&m, &ev);

    if (Beacon_current_state(&m) != BEACON_STATE_IDLE) {
        fprintf(stderr, "after dispatch: expected to still be in Idle, got %d\n",
                (int)Beacon_current_state(&m));
        return 3;
    }
    /* No transition exists, so neither exit(Idle) nor a second
     * entry(Idle) may have fired. */
    if (g_exit_idle != 0 || g_entry_idle != 1) {
        fprintf(stderr,
                "no-op dispatch must not run exit/re-entry: "
                "entry=%d (want 1) exit=%d (want 0)\n",
                g_entry_idle, g_exit_idle);
        return 4;
    }
    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write main.c");

    let exe = dir.join("zero_transition_test");
    let build = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "Beacon.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");
    if !build.status.success() || !build.stderr.is_empty() {
        eprintln!(
            "=== Beacon.c ({tag}) ===\n{}",
            out.find("Beacon.c").unwrap().content
        );
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&build.stderr)
        );
        panic!(
            "TD-BUG-1: zero-transition machine ({tag}) did not gcc -Werror \
             compile: status={:?}",
            build.status.code()
        );
    }
    let run = Command::new(&exe)
        .output()
        .expect("run zero_transition_test");
    if !run.status.success() {
        eprintln!(
            "=== Beacon.c ({tag}) ===\n{}",
            out.find("Beacon.c").unwrap().content
        );
        panic!(
            "TD-BUG-1: zero-transition behavioural test ({tag}) FAILED: \
             exit={:?} (see assertion codes in main.c), stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}

#[test]
#[cfg(not(target_os = "windows"))]
fn zero_transition_machine_gcc_werror_compiles_and_runs_switch_strategy() {
    zero_transition_compiles_runs_and_stays_in_initial(DispatchStrategy::Switch, "switch");
}

#[test]
#[cfg(not(target_os = "windows"))]
fn zero_transition_machine_gcc_werror_compiles_and_runs_table_strategy() {
    zero_transition_compiles_runs_and_stays_in_initial(DispatchStrategy::Table, "table");
}
