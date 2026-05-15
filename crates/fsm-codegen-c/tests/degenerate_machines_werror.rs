//! TD-BUG-1 sibling sweep — every degenerate-but-valid machine shape must
//! emit `-Werror`-clean C and run.
//!
//! TD-BUG-1 is the zero-transition case (its own file,
//! `zero_transition_werror.rs`, covers both dispatch strategies). Per the
//! class-of-issues principle, a zero-transition machine is one point in a
//! family of *degenerate-but-valid* inputs whose generated C the MVP-G4
//! promise ("`-Werror`-clean for ALL valid inputs") must hold for:
//!
//! - **`absolute_minimal`** — a single state, **no** events, **no**
//!   context, **no** actions, **no** transitions. The maximally-stripped
//!   machine: every emitter's "nothing to do" path at once. (The single
//!   self-state's only edge is "stay put forever".)
//! - **`events_but_no_transitions`** — events are declared but nothing
//!   consumes them; no context. Exercises the event-enum emission +
//!   dispatch with a real (non-completion) `ev->id` against an empty
//!   transition set.
//! - **`nested_composite_no_transitions`** — a composite with an inner
//!   state, **zero** transitions anywhere. Exercises the hierarchical
//!   parent-table walk + per-state visitor recursion with nothing to
//!   emit at any depth.
//!
//! Each is emitted under **both** dispatch strategies, compiled with
//! `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, **run**, and asserted
//! to initialise without crashing and stay put when a no-op event is
//! dispatched (these machines have no edge to take — staying is the whole
//! observable semantics). On `main` the switch-strategy cases fail with
//! `-Werror=unused-parameter` and the table-strategy cases additionally
//! with the empty-`trans_table[]` / `type-limits` cluster; all pass after
//! the TD-BUG-1 fix.
//!
//! Bugs *not* of this class were probed and excluded: an empty `entry:`
//! block is an analyzer parse error (FSM-E0010), not codegen — correctly
//! out of scope.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig, DispatchStrategy};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    CompositeState, ContextField, ContextSchema, EventObject, InitialPseudo, Ir, Literal,
    MachineObject, OverflowPolicy, QueueConfig, RegionObject, SimpleState, StateNode, Type,
};

fn loc() -> SourceLocation {
    SourceLocation::new("deg.fsm", Span::new(0, 1), 1, 1)
}

fn simple(id: &str, name: &str) -> SimpleState {
    SimpleState {
        id: id.into(),
        stable_id: format!("M:state:{name}"),
        name: name.into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        loc: loc(),
    }
}

fn machine_shell(
    name: &str,
    context: ContextSchema,
    events: Vec<EventObject>,
    states: Vec<StateNode>,
    initial: &str,
) -> Ir {
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "".into(),
        source_files: vec!["deg.fsm".into()],
        machines: vec![MachineObject {
            id: format!("m-{name}"),
            stable_id: name.into(),
            name: name.into(),
            context,
            events,
            externs: vec![],
            root: RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: initial.into(),
                states,
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

/// Single state, no events, no context, no actions, no transitions.
fn absolute_minimal_ir() -> Ir {
    machine_shell(
        "Min",
        ContextSchema { fields: vec![] },
        vec![],
        vec![
            StateNode::Initial(InitialPseudo {
                id: "ps".into(),
                target: "s-only".into(),
                loc: loc(),
            }),
            StateNode::Simple(simple("s-only", "Only")),
        ],
        "ps",
    )
}

/// Events declared, nothing consumes them; no context, no transitions.
fn events_but_no_transitions_ir() -> Ir {
    let ev = |id: &str, n: &str| EventObject {
        id: id.into(),
        stable_id: format!("Evt:event:{n}"),
        name: n.into(),
        payload: vec![],
        loc: loc(),
    };
    machine_shell(
        "Evt",
        ContextSchema { fields: vec![] },
        vec![ev("ev-ping", "PING"), ev("ev-stop", "STOP")],
        vec![
            StateNode::Initial(InitialPseudo {
                id: "ps".into(),
                target: "s-idle".into(),
                loc: loc(),
            }),
            StateNode::Simple(simple("s-idle", "Idle")),
        ],
        "ps",
    )
}

/// Composite with an inner state; zero transitions anywhere; one context
/// field (so the context struct is non-empty too).
fn nested_composite_no_transitions_ir() -> Ir {
    let inner = StateNode::Simple(simple("s-inner", "Inner"));
    let outer = StateNode::Composite(CompositeState {
        id: "s-outer".into(),
        stable_id: "Nest:state:Outer".into(),
        name: "Outer".into(),
        entry: vec![],
        exit: vec![],
        transitions: vec![],
        timers: vec![],
        defers: vec![],
        regions: vec![RegionObject {
            id: "r-outer".into(),
            stable_id: None,
            name: "Outer".into(),
            initial: "ps-inner".into(),
            states: vec![
                StateNode::Initial(InitialPseudo {
                    id: "ps-inner".into(),
                    target: "s-inner".into(),
                    loc: loc(),
                }),
                inner,
            ],
            priority: 0,
            loc: loc(),
        }],
        history: None,
        loc: loc(),
    });
    machine_shell(
        "Nest",
        ContextSchema {
            fields: vec![ContextField {
                id: "f-n".into(),
                name: "n".into(),
                ty: Type::Primitive { name: "u16".into() },
                default: Some(Literal::Int(fsm_ir::IntLit {
                    value: 0,
                    loc: None,
                })),
                loc: loc(),
            }],
        },
        vec![],
        vec![
            StateNode::Initial(InitialPseudo {
                id: "ps".into(),
                target: "s-outer".into(),
                loc: loc(),
            }),
            outer,
        ],
        "ps",
    )
}

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Emit `ir` under `strategy`, gcc-Werror compile + link a host HAL +
/// generic entry/exit/HAL stub, RUN, assert init + a no-op dispatch keep
/// the machine in its initial state without crashing.
///
/// `prefix` is the machine name (== the C symbol prefix); `init_state` is
/// the `<PREFIX>_STATE_<X>` enumerator the machine must rest in.
fn degenerate_compiles_and_runs(
    ir: &Ir,
    strategy: DispatchStrategy,
    prefix: &str,
    init_state_enum: &str,
    entry_exit_decls: &str,
    tag: &str,
) {
    if !gcc_available() {
        eprintln!("[degenerate_machines_werror/{tag}] gcc not on PATH — skipping");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir: &Path = tmp.path();

    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(ir, &cfg).expect("emit");
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

    let src = format!(
        r#"#include <stdio.h>
#include "fsm_hal.h"
#include "{prefix}.h"

{entry_exit_decls}

int main(void) {{
    {prefix}_t m;
    {prefix}_init(&m);

    if ({prefix}_current_state(&m) != {init_state_enum}) {{
        fprintf(stderr, "post-init: not in initial state, got %d\n",
                (int){prefix}_current_state(&m));
        return 1;
    }}

    /* A degenerate machine with no transition has no edge to take.
     * Dispatch a synthetic event id and require it neither crashes nor
     * moves. Event id 1 is a valid enum slot regardless of declared
     * events (the reserved completion/internal ids occupy low slots). */
    {prefix}_Event_t ev;
    ev.id = ({prefix}_EventId_t)1;
    {prefix}_dispatch(&m, &ev);

    if ({prefix}_current_state(&m) != {init_state_enum}) {{
        fprintf(stderr, "after dispatch: moved off initial state, got %d\n",
                (int){prefix}_current_state(&m));
        return 2;
    }}
    return 0;
}}
"#,
    );
    fs::write(dir.join("main.c"), src).expect("write main.c");

    let c_file = format!("{prefix}.c");
    let exe = dir.join(format!("{tag}_test"));
    let build = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            &c_file,
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");
    if !build.status.success() || !build.stderr.is_empty() {
        eprintln!(
            "=== {c_file} ({tag}) ===\n{}",
            out.find(&c_file).unwrap().content
        );
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&build.stderr)
        );
        panic!(
            "TD-BUG-1 sibling: degenerate machine ({tag}) did not gcc \
             -Werror compile: status={:?}",
            build.status.code()
        );
    }
    let run = Command::new(&exe).output().expect("run degenerate test");
    if !run.status.success() {
        eprintln!(
            "=== {c_file} ({tag}) ===\n{}",
            out.find(&c_file).unwrap().content
        );
        panic!(
            "TD-BUG-1 sibling: degenerate machine ({tag}) behavioural test \
             FAILED: exit={:?}, stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}

// --- absolute_minimal -----------------------------------------------------

#[test]
#[cfg(not(target_os = "windows"))]
fn absolute_minimal_machine_gcc_werror_runs_switch() {
    degenerate_compiles_and_runs(
        &absolute_minimal_ir(),
        DispatchStrategy::Switch,
        "Min",
        "MIN_STATE_ONLY",
        "void Min_entry_ONLY(Min_t *m){(void)m;}\nvoid Min_exit_ONLY(Min_t *m){(void)m;}",
        "min_switch",
    );
}

#[test]
#[cfg(not(target_os = "windows"))]
fn absolute_minimal_machine_gcc_werror_runs_table() {
    degenerate_compiles_and_runs(
        &absolute_minimal_ir(),
        DispatchStrategy::Table,
        "Min",
        "MIN_STATE_ONLY",
        "void Min_entry_ONLY(Min_t *m){(void)m;}\nvoid Min_exit_ONLY(Min_t *m){(void)m;}",
        "min_table",
    );
}

// --- events_but_no_transitions -------------------------------------------

#[test]
#[cfg(not(target_os = "windows"))]
fn events_declared_but_unconsumed_gcc_werror_runs_switch() {
    degenerate_compiles_and_runs(
        &events_but_no_transitions_ir(),
        DispatchStrategy::Switch,
        "Evt",
        "EVT_STATE_IDLE",
        "void Evt_entry_IDLE(Evt_t *m){(void)m;}\nvoid Evt_exit_IDLE(Evt_t *m){(void)m;}",
        "evt_switch",
    );
}

#[test]
#[cfg(not(target_os = "windows"))]
fn events_declared_but_unconsumed_gcc_werror_runs_table() {
    degenerate_compiles_and_runs(
        &events_but_no_transitions_ir(),
        DispatchStrategy::Table,
        "Evt",
        "EVT_STATE_IDLE",
        "void Evt_entry_IDLE(Evt_t *m){(void)m;}\nvoid Evt_exit_IDLE(Evt_t *m){(void)m;}",
        "evt_table",
    );
}

// --- nested_composite_no_transitions -------------------------------------

#[test]
#[cfg(not(target_os = "windows"))]
fn nested_composite_no_transitions_gcc_werror_runs_switch() {
    degenerate_compiles_and_runs(
        &nested_composite_no_transitions_ir(),
        DispatchStrategy::Switch,
        "Nest",
        "NEST_STATE_INNER",
        "void Nest_entry_OUTER(Nest_t *m){(void)m;}\nvoid Nest_exit_OUTER(Nest_t *m){(void)m;}\n\
         void Nest_entry_INNER(Nest_t *m){(void)m;}\nvoid Nest_exit_INNER(Nest_t *m){(void)m;}",
        "nest_switch",
    );
}

#[test]
#[cfg(not(target_os = "windows"))]
fn nested_composite_no_transitions_gcc_werror_runs_table() {
    degenerate_compiles_and_runs(
        &nested_composite_no_transitions_ir(),
        DispatchStrategy::Table,
        "Nest",
        "NEST_STATE_INNER",
        "void Nest_entry_OUTER(Nest_t *m){(void)m;}\nvoid Nest_exit_OUTER(Nest_t *m){(void)m;}\n\
         void Nest_entry_INNER(Nest_t *m){(void)m;}\nvoid Nest_exit_INNER(Nest_t *m){(void)m;}",
        "nest_table",
    );
}
