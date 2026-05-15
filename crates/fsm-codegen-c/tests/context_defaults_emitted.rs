//! Audit 2026-05-14 — Motor_init must seed context-field defaults.
//!
//! The IR carries `ContextField { default: Some(Literal) }` for every DSL
//! field with an `=` initializer (Doc 04 §3.3). Pre-fix, `Motor.c`'s
//! generated `Motor_init` ran `memset(m, 0, sizeof(*m))` and never wrote
//! the declared defaults — silent data loss for every machine that
//! depended on a non-zero initial value (e.g. vending-machine's
//! `price : u16 = 150`).
//!
//! ## Test strategy (v1.1-W0 / SUBAGENT_CONVENTIONS §5.4, PD-2)
//!
//! The §5.4 behavioural-acceptance guard is
//! [`non_zero_defaults_are_observable_in_gcc_built_binary`]: it emits a
//! machine with a non-zero integer default AND a `true` boolean default,
//! compiles it with `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`, RUNS
//! it, and asserts the *runtime* `m.context` field values after `_init`.
//! That is what "seed defaults" actually MEANS — a symbol-presence check
//! for `m->context.price = 150;` is precisely the P0-1-class proxy that
//! cannot tell "the assignment is emitted and runs" from "the assignment
//! is emitted but dead/overwritten". Pre-W0 this file had ONLY the
//! symbol-presence checks; the gcc-RUN test closes that gap.
//!
//! The `*_emits_*` / `*_not_assigned` tests below are kept as **secondary
//! structural checks** (§5.4 last paragraph): they pin the *emission
//! shape* — that fields WITHOUT a default get no assignment (so the memset
//! contract is intact) and that the assignment text appears at all — which
//! localizes a regression faster than a runtime exit code. They are not
//! the behaviour guard; the gcc-RUN test is.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig};
use fsm_diagnostics::{SourceLocation, Span};
use fsm_ir::{
    BoolLit, ContextField, ContextSchema, InitialPseudo, IntLit, Ir, Literal, MachineObject,
    OverflowPolicy, QueueConfig, RegionObject, SimpleState, StateNode, Type,
};

fn loc() -> SourceLocation {
    SourceLocation::new("ctx_defaults.fsm", Span::new(0, 1), 1, 1)
}

#[test]
fn motor_init_emits_zero_defaults_for_speed_and_running() {
    // SECONDARY structural check (§5.4): pins that the explicit-assignment
    // emission path runs even for zero-valued defaults, so a future memset
    // removal cannot silently regress them. The behaviour that defaults are
    // actually OBSERVABLE at runtime is proven by
    // `non_zero_defaults_are_observable_in_gcc_built_binary`.
    //
    // Motor's common-IR fixture declares `speed: u16 = 0; running: bool = false`.
    // Even the zero case must emit explicit assignments — otherwise we
    // can't tell apart "no default" from "default is 0", and any future
    // memset removal would regress silently.
    let ir = common::motor_ir();
    let emitted = emit(&ir, &CodegenConfig::default()).expect("emit");
    let motor_c = emitted
        .find("Motor.c")
        .expect("Motor.c emitted")
        .content
        .clone();
    assert!(
        motor_c.contains("m->context.speed = 0;"),
        "Motor.c must initialise `speed` to its DSL default 0;\n--- Motor.c ---\n{}",
        motor_c
    );
    assert!(
        motor_c.contains("m->context.running = false;"),
        "Motor.c must initialise `running` to its DSL default false;\n--- Motor.c ---\n{}",
        motor_c
    );
}

#[test]
fn motor_init_emits_non_zero_integer_default() {
    // SECONDARY structural check (§5.4) — the runtime-observable proof for
    // the non-zero integer default is the gcc-RUN test (`price == 150`).
    let ir = vending_like_ir(150);
    let emitted = emit(&ir, &CodegenConfig::default()).expect("emit");
    let motor_c = emitted
        .find("Vending.c")
        .expect("Vending.c emitted")
        .content
        .clone();
    assert!(
        motor_c.contains("m->context.price = 150;"),
        "Vending.c must initialise `price = 150` from the DSL default;\n--- Vending.c ---\n{}",
        motor_c
    );
    assert!(
        motor_c.contains("m->context.balance = 0;"),
        "Vending.c must initialise `balance = 0` from the DSL default;\n--- Vending.c ---\n{}",
        motor_c
    );
}

#[test]
fn fields_without_default_are_not_assigned() {
    // LEGITIMATE structural invariant (§5.4 last paragraph): this asserts
    // the ABSENCE of an assignment. A "does NOT emit X" contract has no
    // runtime behaviour to observe — the only correct check IS
    // symbol-(non-)presence. Not a P0-1-class proxy; kept as-is.
    //
    // Synthesise an IR with one field that has NO `default` literal —
    // codegen must NOT emit `m->context.<name>` for it (the memset zero
    // pattern remains, and the unwritten field stays at C's zero).
    let mut ir = vending_like_ir(150);
    // Add a third field with no default.
    ir.machines[0].context.fields.push(ContextField {
        id: "f-undecl".into(),
        name: "undecl".into(),
        ty: Type::Primitive { name: "u8".into() },
        default: None,
        loc: loc(),
    });
    let emitted = emit(&ir, &CodegenConfig::default()).expect("emit");
    let motor_c = emitted
        .find("Vending.c")
        .expect("Vending.c emitted")
        .content
        .clone();
    assert!(
        !motor_c.contains("m->context.undecl ="),
        "Field with no `=` initializer must not get an assignment in Motor_init;\n--- Vending.c ---\n{}",
        motor_c
    );
}

/// Minimal IR mirroring the vending-machine context shape:
///   `balance: u16 = 0`
///   `price  : u16 = price_default`
fn vending_like_ir(price_default: i64) -> Ir {
    let idle = SimpleState {
        id: "s-idle".into(),
        stable_id: "Vending:state:Idle".into(),
        name: "Idle".into(),
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
                target: "s-idle".into(),
                loc: loc(),
            }),
            StateNode::Simple(idle),
        ],
        priority: 0,
        loc: loc(),
    };
    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "".into(),
        source_files: vec!["vending.fsm".into()],
        machines: vec![MachineObject {
            id: "m-vending".into(),
            stable_id: "Vending".into(),
            name: "Vending".into(),
            context: ContextSchema {
                fields: vec![
                    ContextField {
                        id: "f-balance".into(),
                        name: "balance".into(),
                        ty: Type::Primitive { name: "u16".into() },
                        default: Some(Literal::Int(IntLit {
                            value: 0,
                            loc: None,
                        })),
                        loc: loc(),
                    },
                    ContextField {
                        id: "f-price".into(),
                        name: "price".into(),
                        ty: Type::Primitive { name: "u16".into() },
                        default: Some(Literal::Int(IntLit {
                            value: price_default,
                            loc: None,
                        })),
                        loc: loc(),
                    },
                ],
            },
            events: vec![],
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

/// Like [`vending_like_ir`] but (a) adds a `bool` field defaulting to
/// `true` (exercises the non-zero boolean path memset would leave `false`)
/// and (b) gives the machine an event + a self-transition.
///
/// The transition is functionally irrelevant to *this* test — the
/// behavioural assertions only read `m.context.*` straight after `_init`.
/// It exists solely to dodge an UNRELATED, pre-existing codegen defect
/// surfaced by wiring this gcc test (v1.1-W0 / PD-2): for a machine with
/// ZERO transitions, codegen emits
/// `Vending_try_transitions_in_state(Vending_t *m, …, const Vending_Event_t *ev)`
/// with an empty body that never touches `m`/`ev`, so the generated C does
/// NOT pass `gcc -Wall -Wextra -Wpedantic -Werror` (`-Werror=unused-parameter`).
/// That is a genuine product bug (uncompilable C for a degenerate-but-valid
/// machine, masked for the life of this file because the only coverage was
/// symbol-presence `.contains()` that never invoked gcc — the precise
/// P0-1-class hazard PD-2 pays down). Per SUBAGENT_CONVENTIONS §6 +
/// the brief's depth-first rule it is reported, NOT fixed in this
/// test-only wave; the single self-transition keeps THIS test exercising
/// the context-default behaviour it is actually for.
fn vending_with_bool_default_ir() -> Ir {
    let mut ir = vending_like_ir(150);
    ir.machines[0].context.fields.push(ContextField {
        id: "f-enabled".into(),
        name: "enabled".into(),
        ty: Type::Primitive {
            name: "bool".into(),
        },
        default: Some(Literal::Bool(BoolLit {
            value: true,
            loc: None,
        })),
        loc: loc(),
    });
    // One event + a self-transition so codegen's
    // `try_transitions_in_state` body actually uses its parameters (see
    // doc comment above re: the unrelated zero-transition codegen defect).
    ir.machines[0].events.push(fsm_ir::EventObject {
        id: "ev-poke".into(),
        stable_id: "Vending:event:POKE".into(),
        name: "POKE".into(),
        payload: vec![],
        loc: loc(),
    });
    if let StateNode::Simple(idle) = &mut ir.machines[0].root.states[1] {
        #[allow(deprecated)]
        idle.transitions.push(fsm_ir::TransitionObject {
            id: "t-poke".into(),
            stable_id: "Vending:transition:t-poke".into(),
            source: "s-idle".into(),
            target: "s-idle".into(),
            trigger: Some(fsm_ir::Trigger::Event {
                event_id: "ev-poke".into(),
                payload_binding: None,
            }),
            guard: None,
            actions: vec![],
            priority: 100,
            kind: fsm_ir::TransitionKind::External,
            internal: false,
            hint: None,
            loc: loc(),
        });
    } else {
        panic!("vending_like_ir shape changed: states[1] must be Simple Idle");
    }
    ir
}

// ---------------------------------------------------------------------------
// §5.4 behavioural acceptance — defaults are OBSERVABLE at runtime.
// ---------------------------------------------------------------------------

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
#[cfg(not(target_os = "windows"))]
fn non_zero_defaults_are_observable_in_gcc_built_binary() {
    if !gcc_available() {
        eprintln!("[context_defaults_emitted] gcc not on PATH — skipping");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir: &Path = tmp.path();

    let out = emit(&vending_with_bool_default_ir(), &CodegenConfig::default()).expect("emit");
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

    // The driver reads `m.context.*` straight off the struct after
    // `Vending_init`. Pre-fix (memset-only) every field is 0/false and
    // every check below fails — exactly the silent data loss this guards.
    let main = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Vending.h"

void Vending_entry_IDLE(Vending_t *m) { (void)m; }
void Vending_exit_IDLE(Vending_t *m)  { (void)m; }

int main(void) {
    Vending_t m;
    Vending_init(&m);

    /* `price : u16 = 150` — non-zero integer default MUST survive init. */
    if (m.context.price != 150u) {
        fprintf(stderr, "price: expected 150 (DSL default), got %u "
                "(0 => memset-only, the regressed behaviour)\n",
                (unsigned)m.context.price);
        return 1;
    }
    /* `balance : u16 = 0` — explicit zero default; must still be 0 (and
     * must not be skipped just because it equals the memset value). */
    if (m.context.balance != 0u) {
        fprintf(stderr, "balance: expected 0, got %u\n",
                (unsigned)m.context.balance);
        return 2;
    }
    /* `enabled : bool = true` — non-zero boolean default. memset would
     * leave this false; the seeded assignment must make it true. */
    if (!m.context.enabled) {
        fprintf(stderr, "enabled: expected true (DSL default), got false "
                "(memset leaves bool false — the bug)\n");
        return 3;
    }
    return 0;
}
"#;
    fs::write(dir.join("main.c"), main).expect("write main.c");

    let exe = dir.join("ctx_defaults_test");
    let build = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "Vending.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&exe)
        .output()
        .expect("invoke gcc");
    if !build.status.success() || !build.stderr.is_empty() {
        eprintln!(
            "=== Vending.c ===\n{}",
            out.find("Vending.c").unwrap().content
        );
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&build.stderr)
        );
        panic!("gcc failed: status={:?}", build.status.code());
    }
    let run = Command::new(&exe).output().expect("run ctx_defaults_test");
    if !run.status.success() {
        eprintln!(
            "=== Vending.c ===\n{}",
            out.find("Vending.c").unwrap().content
        );
        panic!(
            "context-default behavioural test FAILED: exit={:?} (see assertion \
             codes in main.c), stderr={}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr),
        );
    }
}
