//! §5.4 behavioural-acceptance — OPAQUE-BUG-1: an `opaque "C_type"` extern
//! parameter and return type generate correct C and the opaque handle is
//! threaded verbatim through a guard and an action at runtime.
//!
//! This is NOT a symbol-presence test (SUBAGENT_CONVENTIONS §5.2 / §5.4).
//! It builds an IR with:
//!   * a `dev: opaque "struct dev *"` context field,
//!   * `pure extern is_ready(opaque "struct dev *" h) : bool`,
//!   * `extern set_handle(opaque "struct dev *" h)`,
//!   * a transition `S --GO[is_ready(ctx.dev)]--> S : set_handle(ctx.dev)`,
//! emits C for BOTH dispatch strategies, compiles with
//! `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, links a host impl whose
//! `is_ready`/`set_handle` take a real `struct dev *`, EXECUTES the binary,
//! and asserts the observable behaviour the DSL *means*: the opaque handle
//! stored in the context arrives byte-identical at both the guard and the
//! action, and the action runs exactly once.
//!
//! Codegen already mapped `Type::Opaque` verbatim, so this hand-built IR
//! exercises the emission path end-to-end (defence-in-depth for the
//! feature). The *root-cause* regression — the analyzer silently dropping
//! the opaque param/return/field before codegen ever sees it — is proven
//! FAIL-on-main by `fsm-analyzer/tests/opaque_extern_round_trips.rs`.
//! Before the lowering fix the analyzer produced `is_ready()`/
//! `set_handle()` with no params and no `dev` field, so the real pipeline
//! generated C that does not compile (wrong arity + dangling `ctx.dev`).

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig, DispatchStrategy};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    ContextField, ContextSchema, EventObject, Expr, ExternObject, FieldRef, GuardExpr,
    InitialPseudo, Ir, MachineObject, OverflowPolicy, Param, QueueConfig, RegionObject,
    SimpleState, StateNode, Statement, TransitionKind, TransitionObject, Trigger, Type,
};

fn loc() -> SourceLocation {
    SourceLocation::new("opaque.fsm", Span::new(0, 1), 1, 1)
}

fn opaque_ty() -> Type {
    Type::Opaque {
        c_type: "struct dev *".into(),
    }
}

#[allow(deprecated)]
fn opaque_ir() -> Ir {
    // S --GO [is_ready(ctx.dev)]--> S : set_handle(ctx.dev)
    let guarded = TransitionObject {
        id: "t-s-go".into(),
        stable_id: "M:transition:t-s-go".into(),
        source: "s-s".into(),
        target: "s-s".into(),
        trigger: Some(Trigger::Event {
            event_id: "e-go".into(),
            payload_binding: None,
        }),
        guard: Some(GuardExpr::ExternCall {
            callee: "is_ready".into(),
            args: vec![Expr::FieldRef {
                field_ref: FieldRef::Ctx {
                    field: "dev".into(),
                },
            }],
        }),
        actions: vec![Statement::Call {
            callee: "set_handle".into(),
            args: vec![Expr::FieldRef {
                field_ref: FieldRef::Ctx {
                    field: "dev".into(),
                },
            }],
        }],
        priority: 100,
        kind: TransitionKind::External,
        internal: false,
        hint: None,
        loc: loc(),
    };

    let s = StateNode::Simple(SimpleState {
        id: "s-s".into(),
        stable_id: "M:state:S".into(),
        name: "S".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![guarded],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    });

    Ir {
        ir_version: "1.0.0".into(),
        source_hash: String::new(),
        source_files: vec!["opaque.fsm".into()],
        machines: vec![MachineObject {
            id: "m-m".into(),
            stable_id: "M".into(),
            name: "M".into(),
            context: ContextSchema {
                fields: vec![ContextField {
                    id: "f-M-dev".into(),
                    name: "dev".into(),
                    ty: opaque_ty(),
                    // No default — a pointer field is initialised by the
                    // host before dispatch (NULL-init is the codegen
                    // default for opaque).
                    default: None,
                    loc: loc(),
                }],
            },
            events: vec![EventObject {
                id: "e-go".into(),
                stable_id: "M:event:GO".into(),
                name: "GO".into(),
                payload: vec![],
                loc: loc(),
            }],
            externs: vec![
                ExternObject {
                    id: "ex-M-is_ready".into(),
                    stable_id: "M:extern:is_ready".into(),
                    name: "is_ready".into(),
                    pure: true,
                    params: vec![Param {
                        name: "h".into(),
                        ty: opaque_ty(),
                        id: None,
                        loc: Some(loc()),
                    }],
                    return_type: Some(Type::Primitive {
                        name: "bool".into(),
                    }),
                    loc: loc(),
                },
                ExternObject {
                    id: "ex-M-set_handle".into(),
                    stable_id: "M:extern:set_handle".into(),
                    name: "set_handle".into(),
                    pure: false,
                    params: vec![Param {
                        name: "h".into(),
                        ty: opaque_ty(),
                        id: None,
                        loc: Some(loc()),
                    }],
                    return_type: None,
                    loc: loc(),
                },
            ],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-initial-0".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-initial-0".into(),
                        target: "s-s".into(),
                        loc: loc(),
                    }),
                    s,
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

/// Driver `main` + the opaque-typed HAL impls. Each non-zero return is a
/// distinct failed assertion about the *observable* behaviour: the opaque
/// `struct dev *` stored in the context must arrive byte-identical at the
/// guard (`is_ready`) and the action (`set_handle`), and the action must
/// run exactly once for one GO. `M_entry_S`/`M_exit_S` are no-op leaves.
fn write_driver_main_c(dir: &Path) {
    let main = r#"#include <stdio.h>
#include <assert.h>
#include "fsm_hal.h"
#include "M.h"

struct dev { int magic; int ready; int handle_calls; };

static struct dev g_dev = { 0xABCD, 1, 0 };

/* Opaque-typed externs: the generated prototypes are
 *   bool is_ready(struct dev * h);
 *   void set_handle(struct dev * h);
 * If the opaque type were dropped these would be (void) and the call
 * sites would not compile (wrong arity) — exactly the pre-fix breakage. */
bool is_ready(struct dev *h) {
    if (h != &g_dev) { fprintf(stderr, "is_ready: wrong handle\n"); return false; }
    if (h->magic != 0xABCD) { fprintf(stderr, "is_ready: corrupt handle\n"); return false; }
    return h->ready != 0;
}

void set_handle(struct dev *h) {
    if (h != &g_dev) { fprintf(stderr, "set_handle: wrong handle\n"); return; }
    h->handle_calls++;
}

void M_entry_S(M_t *m) { (void)m; }
void M_exit_S(M_t *m)  { (void)m; }

int main(void) {
    M_t m;
    M_init(&m);

    /* The opaque context field is a real `struct dev *`. */
    m.context.dev = &g_dev;

    M_Event_t go = { .id = M_EVENT_GO };
    M_dispatch(&m, &go);

    /* Guard is_ready(ctx.dev) was true (g_dev.ready==1), so the action
     * set_handle(ctx.dev) must have run exactly once with our handle. A
     * 0 here means the opaque arg never threaded through; a >1 means a
     * dispatch bug. Either way the opaque handle path is broken. */
    if (g_dev.handle_calls != 1) {
        fprintf(stderr,
                "FAIL: set_handle called %d times, want 1 "
                "(0 => opaque handle not threaded through guard+action)\n",
                g_dev.handle_calls);
        return 10;
    }
    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write main.c");
}

fn run_opaque_acceptance(strategy: DispatchStrategy, label: &str) {
    if !gcc_available() {
        eprintln!("[opaque_extern_runs:{label}] gcc not on PATH — skipping");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(&opaque_ir(), &cfg).expect("emit opaque M C");
    write_files(dir, &out);
    write_host_hal_c(dir);
    write_driver_main_c(dir);

    let exe = dir.join(format!("opaque_test_{label}"));
    let result = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "M.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");

    if !result.status.success() || !result.stderr.is_empty() {
        eprintln!(
            "=== generated M.c ({label}) ===\n{}",
            out.find("M.c").unwrap().content
        );
        eprintln!(
            "=== generated M_impl.h ({label}) ===\n{}",
            out.find("M_impl.h").unwrap().content
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

    let run = Command::new(&exe).output().expect("run opaque_test");
    if !run.status.success() {
        eprintln!(
            "=== generated M.c ({label}) ===\n{}",
            out.find("M.c").unwrap().content
        );
        panic!(
            "opaque_test binary FAILED [{label}]: exit={:?} (see assertion code in main.c), \
             stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}

#[test]
fn opaque_handle_threads_through_guard_and_action_switch_strategy() {
    run_opaque_acceptance(DispatchStrategy::Switch, "switch");
}

#[test]
fn opaque_handle_threads_through_guard_and_action_table_strategy() {
    run_opaque_acceptance(DispatchStrategy::Table, "table");
}
