//! §5.4 behavioural-acceptance — v1.1-W4 `likely`/`rare` branch hints.
//!
//! This is NOT a symbol-presence test (SUBAGENT_CONVENTIONS §5.2 / §5.4).
//! It builds an IR whose transitions carry every hint value
//! (`Some(Likely)`, `Some(Rare)`, `None`), runs `emit()`, then:
//!
//!   1. Asserts the generated C contains the portable
//!      `__builtin_expect`-backed macro for the hinted transitions and the
//!      *plain* condition for the unhinted one (structural check — cheap
//!      secondary guard, never the sole guard per §5.2).
//!   2. Compiles the generated C with
//!      `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, links a host HAL
//!      stub + a driver `main`, EXECUTES it, and asserts the machine still
//!      behaves IDENTICALLY regardless of the hints — a branch hint is a
//!      pure instruction-layout optimization with ZERO semantic effect, so
//!      the same event sequence must drive the same state path whether a
//!      transition is `likely`, `rare`, or unhinted. Both the switch and
//!      table dispatch strategies.
//!   3. Recompiles with `__GNUC__` / `__clang__` forced off (the macro's
//!      `#else` fallback) and asserts the non-GNU C *still builds and still
//!      behaves identically* — proving the portability discipline (the
//!      generated firmware does not hard-depend on a compiler extension).
//!
//! Regression contract (§5.1): on `main` the `likely`/`rare` prefix does
//! not parse (`FSM-E0010`) and `TransitionObject` has no `hint` field, so
//! this IR cannot even be constructed there — the test compiles+passes
//! only after the W4 vertical slice lands.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig, DispatchStrategy};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    BranchHint, ContextSchema, EventObject, ExternObject, GuardExpr, InitialPseudo, Ir,
    MachineObject, OverflowPolicy, QueueConfig, RegionObject, SimpleState, StateNode,
    TransitionKind, TransitionObject, Trigger, Type,
};

fn loc() -> SourceLocation {
    SourceLocation::new("hints.fsm", Span::new(0, 1), 1, 1)
}

/// A guarded transition (the hint wraps the guard condition). `guard_extern`
/// is a `pure extern` returning bool; the host stub makes it return `true`
/// so every transition fires when triggered (so the hint, which only biases
/// the *predicted* outcome, provably does not change the *actual* outcome).
#[allow(deprecated)]
fn guarded(
    id: &str,
    source: &str,
    target: &str,
    event_id: &str,
    guard_extern: &str,
    hint: Option<BranchHint>,
) -> TransitionObject {
    TransitionObject {
        id: id.into(),
        stable_id: format!("Hints:transition:{id}"),
        source: source.into(),
        target: target.into(),
        trigger: Some(Trigger::Event {
            event_id: event_id.into(),
            payload_binding: None,
        }),
        guard: Some(GuardExpr::ExternCall {
            callee: guard_extern.into(),
            args: vec![],
        }),
        actions: vec![],
        priority: 100,
        kind: TransitionKind::External,
        internal: false,
        hint,
        loc: loc(),
    }
}

fn event(id: &str, name: &str) -> EventObject {
    EventObject {
        id: id.into(),
        stable_id: format!("Hints:event:{name}"),
        name: name.into(),
        payload: vec![],
        loc: loc(),
    }
}

fn pure_extern(name: &str) -> ExternObject {
    ExternObject {
        id: format!("x-{name}"),
        stable_id: format!("Hints:extern:{name}"),
        name: name.into(),
        params: vec![],
        return_type: Some(Type::Primitive {
            name: "bool".into(),
        }),
        pure: true,
        loc: loc(),
    }
}

fn simple(id: &str, name: &str, transitions: Vec<TransitionObject>) -> StateNode {
    StateNode::Simple(SimpleState {
        id: id.into(),
        stable_id: format!("Hints:state:{name}"),
        name: name.into(),
        entry: vec![],
        exit: vec![],
        transitions,
        timers: vec![],
        defers: vec![],
        loc: loc(),
    })
}

/// `Hints`:
///   Idle  --GO[g_go]-->        Running   (likely — hot path)
///   Running --FAULT[g_fault]--> Faulted  (rare   — cold path)
///   Running --STOP[g_stop]-->   Idle     (unhinted — plain condition)
///   Faulted --RESET[g_reset]--> Idle     (unhinted)
fn hints_ir() -> Ir {
    let idle = simple(
        "s-idle",
        "Idle",
        vec![guarded(
            "t-go",
            "s-idle",
            "s-running",
            "e-go",
            "g_go",
            Some(BranchHint::Likely),
        )],
    );
    let running = simple(
        "s-running",
        "Running",
        vec![
            guarded(
                "t-fault",
                "s-running",
                "s-faulted",
                "e-fault",
                "g_fault",
                Some(BranchHint::Rare),
            ),
            guarded(
                "t-stop",
                "s-running",
                "s-idle",
                "e-stop",
                "g_stop",
                None, // unhinted
            ),
        ],
    );
    let faulted = simple(
        "s-faulted",
        "Faulted",
        vec![guarded(
            "t-reset",
            "s-faulted",
            "s-idle",
            "e-reset",
            "g_reset",
            None,
        )],
    );

    Ir {
        ir_version: "1.0.0".into(),
        source_hash: String::new(),
        source_files: vec!["hints.fsm".into()],
        machines: vec![MachineObject {
            id: "m-hints".into(),
            stable_id: "Hints".into(),
            name: "Hints".into(),
            context: ContextSchema { fields: vec![] },
            events: vec![
                event("e-go", "GO"),
                event("e-fault", "FAULT"),
                event("e-stop", "STOP"),
                event("e-reset", "RESET"),
            ],
            externs: vec![
                pure_extern("g_go"),
                pure_extern("g_fault"),
                pure_extern("g_stop"),
                pure_extern("g_reset"),
            ],
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
                    idle,
                    running,
                    faulted,
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

/// Driver: every guard extern returns true so each triggered transition
/// fires. We then drive GO→FAULT→RESET→GO→STOP and assert the exact state
/// path. If a branch hint had any semantic effect (it must NOT), the path
/// would diverge. Each non-zero return code is a distinct failed assertion.
fn write_driver_main_c(dir: &Path) {
    let main = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Hints.h"

/* All guards true → the hint biases prediction only, never the outcome. */
bool g_go(void)    { return true; }
bool g_fault(void) { return true; }
bool g_stop(void)  { return true; }
bool g_reset(void) { return true; }

void Hints_entry_IDLE(Hints_t *m)    { (void)m; }
void Hints_entry_RUNNING(Hints_t *m) { (void)m; }
void Hints_entry_FAULTED(Hints_t *m) { (void)m; }
void Hints_exit_IDLE(Hints_t *m)     { (void)m; }
void Hints_exit_RUNNING(Hints_t *m)  { (void)m; }
void Hints_exit_FAULTED(Hints_t *m)  { (void)m; }

int main(void) {
    Hints_t m;
    Hints_init(&m);
    if (Hints_current_state(&m) != HINTS_STATE_IDLE) return 10;

    /* GO (likely-hinted guard g_go): Idle -> Running. The _LIKELY wrapper
     * must not change that the guard (true) passes. */
    Hints_Event_t go = { .id = HINTS_EVENT_GO };
    Hints_dispatch(&m, &go);
    if (Hints_current_state(&m) != HINTS_STATE_RUNNING) return 11;

    /* FAULT (rare-hinted guard g_fault): Running -> Faulted. The _UNLIKELY
     * wrapper must STILL take the branch when the guard holds. */
    Hints_Event_t fault = { .id = HINTS_EVENT_FAULT };
    Hints_dispatch(&m, &fault);
    if (Hints_current_state(&m) != HINTS_STATE_FAULTED) return 12;

    /* RESET (unhinted): Faulted -> Idle. */
    Hints_Event_t reset = { .id = HINTS_EVENT_RESET };
    Hints_dispatch(&m, &reset);
    if (Hints_current_state(&m) != HINTS_STATE_IDLE) return 13;

    /* GO again then STOP (unhinted): Idle -> Running -> Idle. Proves the
     * unhinted plain-condition path and the hinted path interoperate and
     * the machine is fully reusable (no hint side-effect on state). */
    Hints_dispatch(&m, &go);
    if (Hints_current_state(&m) != HINTS_STATE_RUNNING) return 14;
    Hints_Event_t stop = { .id = HINTS_EVENT_STOP };
    Hints_dispatch(&m, &stop);
    if (Hints_current_state(&m) != HINTS_STATE_IDLE) return 15;

    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write main.c");
}

/// Structural check (§5.2 cheap secondary): the hinted transitions' guard
/// must be wrapped in the portable macro; the unhinted one must be the bare
/// condition (no wrapper). Plus the macro definition itself with both the
/// `__builtin_expect` arm and the `#else` fallback.
fn assert_macro_shape(c_src: &str, header: &str) {
    // Header defines both macros, GNU arm + portable fallback. (The
    // generated style uses indented preprocessor directives — `#  define`.)
    assert!(
        header.contains("#  define HINTS_LIKELY(x)   (__builtin_expect(!!(x), 1))"),
        "header must define the __builtin_expect-backed LIKELY macro; got:\n{header}"
    );
    assert!(
        header.contains("#  define HINTS_UNLIKELY(x) (__builtin_expect(!!(x), 0))"),
        "header must define the __builtin_expect-backed UNLIKELY macro"
    );
    assert!(
        header.contains("#  define HINTS_LIKELY(x)   (x)")
            && header.contains("#  define HINTS_UNLIKELY(x) (x)"),
        "header must define the non-GNU `(x)` fallback for BOTH macros"
    );
    assert!(
        header.contains(
            "#if !defined(HINTS_NO_BUILTIN_EXPECT) && (defined(__GNUC__) || defined(__clang__))"
        ),
        "fallback must be guarded by a __GNUC__/__clang__ #if with an \
         opt-out override macro"
    );

    // The likely-hinted guard (g_go) wrapped in _LIKELY.
    assert!(
        c_src.contains("HINTS_LIKELY(g_go())"),
        "likely-hinted transition's guard must be wrapped in HINTS_LIKELY; got:\n{c_src}"
    );
    // The rare-hinted guard (g_fault) wrapped in _UNLIKELY.
    assert!(
        c_src.contains("HINTS_UNLIKELY(g_fault())"),
        "rare-hinted transition's guard must be wrapped in HINTS_UNLIKELY"
    );
    // The unhinted guard (g_stop / g_reset) must be the BARE call — never
    // wrapped (zero codegen change for unhinted transitions).
    assert!(
        c_src.contains("!g_stop()") && !c_src.contains("LIKELY(g_stop())"),
        "unhinted transition's guard must be the plain condition, NOT wrapped"
    );
    assert!(
        !c_src.contains("LIKELY(g_reset())"),
        "unhinted transition's guard must be the plain condition"
    );
}

fn compile_run(dir: &Path, exe_name: &str, extra_gcc: &[&str]) {
    let exe = dir.join(exe_name);
    let mut args: Vec<&str> = vec![
        "-std=c99",
        "-Wall",
        "-Wextra",
        "-Wpedantic",
        "-Werror",
        "-I.",
    ];
    args.extend_from_slice(extra_gcc);
    args.extend_from_slice(&["Hints.c", "main.c", "host_hal.c", "-o"]);

    let result = Command::new("gcc")
        .current_dir(dir)
        .args(&args)
        .arg(&exe)
        .output()
        .expect("invoke gcc");
    if !result.status.success() || !result.stderr.is_empty() {
        let c = fs::read_to_string(dir.join("Hints.c")).unwrap_or_default();
        eprintln!("=== generated Hints.c ===\n{c}");
        eprintln!(
            "=== gcc stderr ({exe_name}) ===\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
        panic!("gcc failed [{exe_name}]: status={:?}", result.status.code());
    }
    let run = Command::new(&exe).output().expect("run hints test");
    if !run.status.success() {
        panic!(
            "hints binary FAILED [{exe_name}]: exit={:?} — a branch hint changed \
             observable behaviour (it must be a pure layout optimization). \
             stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}

fn run_acceptance(strategy: DispatchStrategy, label: &str) {
    if !gcc_available() {
        eprintln!("[branch_hints_codegen:{label}] gcc not on PATH — skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(&hints_ir(), &cfg).expect("emit Hints C");

    let c_src = out
        .find("Hints.c")
        .expect("Hints.c emitted")
        .content
        .clone();
    let header = out
        .find("Hints.h")
        .expect("Hints.h emitted")
        .content
        .clone();
    assert_macro_shape(&c_src, &header);

    write_files(dir, &out);
    write_host_hal_c(dir);
    write_driver_main_c(dir);

    // 1. GNU/clang path — __builtin_expect actually used.
    compile_run(dir, &format!("hints_gnu_{label}"), &[]);

    // 2. Non-GNU fallback path — force the macro `#else` arm via the
    //    generated header's own opt-out (`-DHINTS_NO_BUILTIN_EXPECT`) and
    //    prove the generated firmware STILL builds AND behaves identically
    //    with the plain `(x)` expansion (the portability discipline — a
    //    strictly-conforming non-GNU C99 compiler must accept it). Using
    //    the header's documented override (rather than `-U__GNUC__`, which
    //    would break glibc's own internal `__GNUC__` use) tests the exact
    //    `#else` arm the generated code emits for non-GCC toolchains.
    compile_run(
        dir,
        &format!("hints_nongnu_{label}"),
        &["-DHINTS_NO_BUILTIN_EXPECT"],
    );
}

#[test]
fn branch_hints_are_layout_only_switch_strategy() {
    run_acceptance(DispatchStrategy::Switch, "switch");
}

#[test]
fn branch_hints_are_layout_only_table_strategy() {
    run_acceptance(DispatchStrategy::Table, "table");
}
