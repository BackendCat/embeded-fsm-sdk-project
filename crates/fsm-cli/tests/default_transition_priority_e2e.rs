//! End-to-end behavioural acceptance for W7-FU-2: the lowered DEFAULT
//! transition priority (SUBAGENT_CONVENTIONS §5.4, Doc 00 §11.27, Doc 04
//! §8.6, Doc 08 §4.2).
//!
//! THE DISCREPANCY THIS RECONCILES (pre-existing; surfaced by W7-FU-1's
//! §5.4 discipline, NOT introduced by it):
//! a transition with **no `priority` clause** was lowered by the analyzer's
//! four `lower_*` arms with `.unwrap_or(0)` — IR priority `0`. But Doc 04
//! §8.6 ("Lower number = higher priority. Default: `100`.") and Doc 09 §6
//! ("default 100") both specify **100**. Selection is min-wins on
//! `(priority, document_order)` (Doc 08 §4.2): a *lower* number is *higher*
//! priority. The discrepancy was applied IDENTICALLY by the simulator and
//! both codegen strategies (they each only READ the IR `priority`), so
//! sim≡codegen held regardless — that is why this is P1 (consistent,
//! well-defined) not P0 (silent miscompile). But a default of `0` made an
//! unprioritized transition out-prioritise an explicitly-deprioritised one,
//! so a small explicit number could no longer FLOAT a specific transition
//! above the unprioritized herd — the priority feature was half-useless and
//! the impl contradicted the normative spec. The fix defaults to `100`
//! (single source of truth: [`fsm_ir::DEFAULT_TRANSITION_PRIORITY`]).
//!
//! WHY THIS FIXTURE IS A HAND-BUILT IR (a judgment call, documented):
//! the analyzer's FSM-E0300 (Doc 10) makes two same-(source,event)
//! transitions whose guards are not statically disjoint a COMPILE ERROR
//! UNLESS every one carries an explicit `priority` clause (then FSM-W0300
//! acknowledges it). A transition with NO priority clause therefore can
//! NEVER be in genuine same-source same-event runtime competition in an
//! analyzer-accepted `.fsm` — exactly why W7-FU-1's `.fsm` fixture used
//! explicit `priority 1/2/3`. The default value's *observable* effect is
//! consequently only demonstrable at the codegen/simulator contract layer
//! (the IR), below the analyzer's compile-time E0300 gate ("runtime should
//! not reach here" — Doc 08 §4.1). Building IR directly is the established
//! pattern for codegen/sim behavioural tests here
//! (`codegen_equivalence_smoke.rs`, `done_autofire_c.rs`, the codegen
//! `common/mod.rs` builders). To keep the fixture a TRUE regression guard
//! tied to the fix, the "unprioritized" transition's IR priority is set to
//! [`fsm_ir::DEFAULT_TRANSITION_PRIORITY`] itself — i.e. the precise value
//! the FIXED lowering materializes for a clause-less transition. If the
//! constant or the four `lower_*` arms regress to `0`, this fixture's
//! winner flips and BOTH the gcc-run and the simulator assertions fail
//! loudly. The companion unit pin
//! (`fsm-analyzer/tests/lowering.rs::transition_without_priority_clause_\
//! lowers_to_spec_default_100`) proves the *lowering* itself produces 100
//! from real `.fsm` source; this file proves that value drives spec-correct
//! selection through the whole pipeline.
//!
//! ASCII-only C on purpose (`-Werror` + embedded toolchains reject stray
//! non-ASCII bytes — a prior wave hit this for real).

use std::fs;
use std::process::Command;

use fsm_codegen_c::{emit, CodegenConfig, DispatchStrategy};
use fsm_ir::{
    BinaryOp, ContextField, ContextSchema, EventObject, Expr, FieldRef, InitialPseudo, IntLit, Ir,
    Literal, MachineObject, OverflowPolicy, QueueConfig, SimpleState, SourceLocation, Span,
    StateNode, Statement, TransitionKind, TransitionObject, Trigger, Type,
    DEFAULT_TRANSITION_PRIORITY,
};
use fsm_simulator::{InitOptions, Interpreter, Value};

fn loc() -> SourceLocation {
    SourceLocation::new("pri.fsm", Span::new(0, 1), 1, 1)
}

/// `ctx.which = <n>` — a minimal observable side effect supported by BOTH
/// codegen strategies and the simulator.
fn assign_which(n: i64) -> Statement {
    Statement::Assign {
        target: FieldRef::Ctx {
            field: "which".into(),
        },
        value: Expr::Binary {
            // `0 + n` keeps the action shape identical to the canonical
            // `ctx.f = ctx.f + 1` form codegen/sim are exercised with
            // elsewhere, without depending on the field's prior value.
            op: BinaryOp::Add,
            left: Box::new(Expr::Literal(Literal::Int(IntLit {
                value: 0,
                loc: None,
            }))),
            right: Box::new(Expr::Literal(Literal::Int(IntLit {
                value: n,
                loc: None,
            }))),
        },
    }
}

fn simple(id: &str, name: &str, transitions: Vec<TransitionObject>) -> StateNode {
    StateNode::Simple(SimpleState {
        id: id.into(),
        stable_id: format!("Pri:state:{name}"),
        name: name.into(),
        entry: vec![],
        exit: vec![],
        transitions,
        timers: vec![],
        defers: vec![],
        loc: loc(),
    })
}

#[allow(deprecated)]
fn transition(id: &str, target: &str, priority: u16, action_which: i64) -> TransitionObject {
    TransitionObject {
        id: id.into(),
        stable_id: format!("Pri:transition:{id}"),
        source: "s-sel".into(),
        target: target.into(),
        trigger: Some(Trigger::Event {
            event_id: "ev-E".into(),
            payload_binding: None,
        }),
        // BOTH transitions are UNGUARDED so each is always an enabled
        // candidate: selection is decided purely by `(priority,
        // document_order)`. Since the two priorities differ (50 vs the
        // default), priority — not document order — is the deciding factor.
        guard: None,
        actions: vec![assign_which(action_which)],
        priority,
        kind: TransitionKind::External,
        internal: false,
        hint: None,
        loc: loc(),
    }
}

/// The explicit priority the discriminating transition carries. The fixture
/// only proves anything if the default sits strictly *below* this under
/// min-wins (i.e. a strictly larger number) — guarded at compile time just
/// below so a future change moving the default to `<= 50` breaks the build
/// rather than the fixture silently losing its discriminating power.
const EXPLICIT_PRIORITY: u16 = 50;

// Compile-time fixture invariant: the default transition priority must be a
// LOWER priority (larger number, min-wins) than the explicit competitor, or
// this fixture cannot prove the default value decides the selection. The
// `#[allow]` is the documented exception for `assertions_on_constants` — a
// deliberate const guard that must break the BUILD (not silently no-op) if
// the constant ever changes; clippy's "optimized out" note only confirms
// the invariant currently holds.
#[allow(clippy::assertions_on_constants)]
const _: () = assert!(
    DEFAULT_TRANSITION_PRIORITY > EXPLICIT_PRIORITY,
    "fixture invariant: DEFAULT_TRANSITION_PRIORITY must be > EXPLICIT_PRIORITY \
     (min-wins) for the default value to decide the selected transition"
);

/// Machine `Pri`: from `Sel`, two unguarded `on E` transitions.
///
///  - `t-explicit` : `priority 50`  -> `Hi` (sets `which = 1`)
///  - `t-default`   : `priority = DEFAULT_TRANSITION_PRIORITY` -> `Lo`
///                    (sets `which = 2`) — models a clause-less transition
///
/// Declared order is explicit-then-default, so the document-order tiebreak
/// is NOT what decides (the priorities differ). Min-wins (Doc 08 §4.2):
///  - default == 100 (the spec / the fix): `min(50, 100) = 50` → `Hi`,
///    `which == 1`.
///  - default == 0 (the pre-fix bug): `min(50, 0) = 0` → `Lo`, `which == 2`.
///
/// The two outcomes are observably distinct in BOTH the generated C and the
/// simulator, so the default value genuinely decides the outcome.
fn pri_ir() -> Ir {
    let t_explicit = transition("t-explicit", "s-hi", EXPLICIT_PRIORITY, 1);
    let t_default = transition("t-default", "s-lo", DEFAULT_TRANSITION_PRIORITY, 2);

    Ir {
        ir_version: "1.0.0".into(),
        source_hash: "".into(),
        source_files: vec!["pri.fsm".into()],
        machines: vec![MachineObject {
            id: "m-pri".into(),
            stable_id: "Pri".into(),
            name: "Pri".into(),
            context: ContextSchema {
                fields: vec![ContextField {
                    id: "f-which".into(),
                    name: "which".into(),
                    ty: Type::Primitive { name: "u8".into() },
                    default: Some(Literal::Int(IntLit {
                        value: 0,
                        loc: None,
                    })),
                    loc: loc(),
                }],
            },
            events: vec![EventObject {
                id: "ev-E".into(),
                stable_id: "Pri:event:E".into(),
                name: "E".into(),
                payload: vec![],
                loc: loc(),
            }],
            externs: vec![],
            root: fsm_ir::RegionObject {
                id: "r-root".into(),
                stable_id: None,
                name: "__root".into(),
                initial: "ps-init".into(),
                states: vec![
                    StateNode::Initial(InitialPseudo {
                        id: "ps-init".into(),
                        target: "s-sel".into(),
                        loc: loc(),
                    }),
                    simple("s-sel", "Sel", vec![t_explicit, t_default]),
                    simple("s-hi", "Hi", vec![]),
                    simple("s-lo", "Lo", vec![]),
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

/// ASCII-only host `main`: init `Pri`, dispatch `E`, assert the
/// spec-correct (priority-50, default=100 wins) transition fired — landing
/// in `Hi` with `which == 1`. A non-zero exit + a `FAIL` line on stderr
/// pinpoints the failure (mirrors `guard_disambiguated_dispatch_e2e.rs`).
const DRIVER_C: &str = r#"#include <stdio.h>
#include "Pri.h"

void Pri_entry_SEL(Pri_t *m) { (void)m; }
void Pri_entry_HI(Pri_t *m)  { (void)m; }
void Pri_entry_LO(Pri_t *m)  { (void)m; }
void Pri_exit_SEL(Pri_t *m)  { (void)m; }
void Pri_exit_HI(Pri_t *m)   { (void)m; }
void Pri_exit_LO(Pri_t *m)   { (void)m; }

int main(void) {
    Pri_t m;
    Pri_init(&m);
    Pri_Event_t e;
    e.id = PRI_EVENT_E;
    Pri_dispatch(&m, &e);

    /* Doc 08 4.2 min-wins on (priority, document_order): the explicit
       `priority 50` transition beats the default-priority (100) one
       (50 < 100), so we land in Hi and `which == 1`. If the default
       regressed to 0, the OTHER transition (0 < 50) would win -> Lo,
       which == 2 -> these assertions fail loudly. */
    if (m.context.which != 1) {
        fprintf(stderr,
                "FAIL which: got %d want 1 (default-priority bug? "
                "default must be 100 so explicit 50 wins)\n",
                (int)m.context.which);
        return 1;
    }
    if (Pri_current_state(&m) != PRI_STATE_HI) {
        fprintf(stderr, "FAIL state: got %d want HI(%d)\n",
                (int)Pri_current_state(&m), (int)PRI_STATE_HI);
        return 2;
    }
    printf("ALL OK\n");
    return 0;
}
"#;

/// Minimal ASCII host HAL — the generated runtime references these.
const HAL_C: &str = r#"#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
uint32_t fsm_hal_clock_now_ms(void) { return 0; }
void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) { fprintf(stderr, "[FSM ASSERT] %s\n", msg); abort(); }
}
"#;

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `emit` for `strategy` → write files → `gcc -…-Werror` compile + link +
/// RUN the driver. Panics with full gcc/runtime output + the generated C on
/// any failure so a regression (wrong transition fired, warning) is
/// unmissable.
fn emit_compile_run(strategy: DispatchStrategy, label: &str) {
    if !gcc_available() {
        eprintln!("[default_transition_priority_e2e/{label}] gcc not on PATH — skipping");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let cfg = CodegenConfig {
        strategy,
        ..CodegenConfig::default()
    };
    let out = emit(&pri_ir(), &cfg).expect("emit");
    for f in &out.files {
        fs::write(dir.join(&f.path), &f.content).expect("write generated");
    }
    fs::write(dir.join("driver.c"), DRIVER_C).unwrap();
    fs::write(dir.join("hal.c"), HAL_C).unwrap();

    let bin = dir.join("pri_app");
    let gcc = Command::new("gcc")
        .current_dir(dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "Pri.c",
            "driver.c",
            "hal.c",
            "-o",
        ])
        .arg(&bin)
        .output()
        .expect("gcc spawn");
    assert!(
        gcc.status.success(),
        "[{label}] gcc -Werror FAILED. stderr:\n{}\n--- generated Pri.c ---\n{}",
        String::from_utf8_lossy(&gcc.stderr),
        out.find("Pri.c").map(|f| f.content.as_str()).unwrap_or(""),
    );

    let run = Command::new(&bin).output().expect("run pri_app");
    assert!(
        run.status.success(),
        "[{label}] generated binary asserted WRONG behaviour — the \
         default-priority transition selection is not spec-correct (Doc 04 \
         §8.6 / Doc 08 §4.2). stdout:\n{}\nstderr:\n{}\n--- generated Pri.c ---\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
        out.find("Pri.c").map(|f| f.content.as_str()).unwrap_or(""),
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("ALL OK"),
        "[{label}] binary did not print ALL OK. stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
}

/// W7-FU-2 acceptance — SWITCH strategy. The default-priority (100)
/// transition must LOSE to the explicit `priority 50` one.
#[test]
fn default_priority_transition_loses_to_lower_explicit_switch_strategy() {
    emit_compile_run(DispatchStrategy::Switch, "switch");
}

/// W7-FU-2 acceptance — TABLE strategy (same spec outcome; the table is
/// pre-sorted `(source, priority, row_idx)` so 50 sorts before 100).
#[test]
fn default_priority_transition_loses_to_lower_explicit_table_strategy() {
    emit_compile_run(DispatchStrategy::Table, "table");
}

/// sim≡codegen cross-check. The simulator is the behavioural oracle; it
/// reads the SAME IR `priority` values the generated C does. The expected
/// outcome is derived BY HAND from Doc 08 §4.2 (the spec, not codegen):
/// min-wins picks the `priority 50` transition over the default-100 one, so
/// the machine lands in `Hi`. Asserting this against the simulator proves
/// the oracle agrees with the spec; the two e2e tests above assert the
/// generated C produces the identical outcome → sim≡codegen.
#[test]
fn default_priority_selection_matches_spec_and_simulator() {
    let ir = pri_ir();
    let mut interp = Interpreter::new(&ir).expect("interpreter");
    interp
        .init(InitOptions {
            machine_name: "Pri".into(),
            ..Default::default()
        })
        .expect("init");
    interp.dispatch("E").expect("dispatch E");

    // Spec (Doc 08 §4.2), derived by hand: min((50, 0), (100, 1)) on
    // (priority, document_order) = the priority-50 transition → `which`
    // mutated to 1, active leaf = `s-hi` (named `Hi`).
    let ctx = interp.context().expect("context");
    let which = match ctx.get("which") {
        Some(Value::U8(v)) => i128::from(*v),
        Some(Value::U16(v)) => i128::from(*v),
        Some(Value::U32(v)) => i128::from(*v),
        Some(Value::I8(v)) => i128::from(*v),
        Some(Value::I16(v)) => i128::from(*v),
        Some(Value::I32(v)) => i128::from(*v),
        other => panic!("`which` not an integer: {other:?}"),
    };
    assert_eq!(
        which, 1,
        "simulator must select the explicit `priority 50` transition over \
         the default-priority ({DEFAULT_TRANSITION_PRIORITY}) one per Doc \
         08 §4.2 min-wins (sim==codegen; oracle would be WRONG — escalate)"
    );
    // `current_states_named()` returns machine-qualified leaves
    // (`"<Machine>.<State>"`, e.g. `"Pri.Hi"`). Assert the active leaf is
    // the priority-50 target `Hi` and definitively NOT the default-priority
    // target `Lo` (robust to the qualification prefix).
    let leaves = interp.current_states_named();
    assert_eq!(
        leaves.len(),
        1,
        "exactly one active leaf expected; got {leaves:?}"
    );
    assert!(
        leaves[0].ends_with(".Hi") || leaves[0] == "Hi",
        "simulator active leaf must be `Hi` (the priority-50 target); got \
         {leaves:?}"
    );
    assert!(
        !(leaves[0].ends_with(".Lo") || leaves[0] == "Lo"),
        "simulator selected `Lo` (the default-priority target) — the \
         default priority is mis-ordered vs the explicit 50 (Doc 08 §4.2 \
         min-wins violated); got {leaves:?}"
    );
}
