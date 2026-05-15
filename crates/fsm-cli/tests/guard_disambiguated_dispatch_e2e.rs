//! End-to-end behavioural acceptance for W7-FU-1: guard-disambiguated
//! same-event transition dispatch (SUBAGENT_CONVENTIONS §5.4, Doc 00
//! §11.26, Doc 08 §4.1/§4.2).
//!
//! THE DEFECT THIS PROVES FIXED (pre-existing P0-1 silent-data-loss class,
//! surfaced by W7's §5.4 discipline, NOT introduced by W7):
//! a state with two or more transitions on the SAME event disambiguated by
//! guards (`on E [g1] -> A` / `on E [g2] -> B`, plus an optional unguarded
//! `on E -> C` fallback) — bread-and-butter UML the DSL/analyzer/IR accept
//! and preserve — was mis-lowered by BOTH dispatch strategies independently:
//!   * SWITCH: one `case <EVENT>:` per transition => duplicate C `case`
//!     label => `gcc` HARD ERROR `duplicate case value` (even without
//!     `-Werror`); the 2nd+ guarded transition unreachable.
//!   * TABLE: `<M>_select_for_region` returned the FIRST `(source,trigger)`
//!     row IGNORING the guard; the executor re-checked the guard and
//!     silently no-op'd if false => the event was DROPPED and the later
//!     eligible transition never fired (silent data loss).
//!
//! THE SPEC RULE (Doc 08 §4.1 + §4.2, mirrored Doc 04 informational SELECT):
//! among same-(source,event) transitions the candidate set is the
//! transitions whose guard holds; the winner is
//! `min(candidates, key=(priority, document_order))` — i.e. first-enabled in
//! (priority asc, then declared source order). An unguarded / `[else]`
//! transition is an always-enabled candidate (the catch-all/fallback, taken
//! only once every earlier-in-order guard has failed). If NO candidate is
//! enabled the event is not consumed in that state and the leaf-to-root
//! walk continues (normal hierarchical bubbling) — here that means discard.
//!
//! THE ORACLE: the simulator (`fsm_simulator::Interpreter::select_transitions`)
//! already implements exactly this (guard filtered into `candidates` BEFORE
//! the stable priority/doc-order pick). It was independently verified
//! correct against the SPEC (not merely against codegen). So this wave
//! fixes ONLY codegen; both strategies must now match the simulator. This
//! test asserts that sim==codegen on the discriminating sequences.
//!
//! WHY EXPLICIT `priority 1/2/3`: the analyzer's FSM-E0300 makes two
//! same-(source,event) transitions whose guards are not statically disjoint
//! a COMPILE ERROR unless they carry explicit `priority` clauses (then
//! FSM-W0300 acknowledges it — Doc 10 FSM-E0300 fix option 3). Explicit
//! priorities are therefore the spec-legal way to express "guarded-first,
//! unguarded-fallback-last" AND the only construct in which two guards can
//! be simultaneously true so the declared/priority order genuinely decides
//! the winner (the subtle tiebreak case the brief mandates exercising).
//! (Aside: a `priority`-clause-less transition lowers to IR priority 0, not
//! the Doc 04 §8.6-stated 100 — a pre-existing doc/impl discrepancy applied
//! IDENTICALLY by sim and codegen, so sim==codegen holds regardless; flagged
//! in the W7-FU-1 completion report, out of this wave's scope.)
//!
//! ASCII-only C on purpose (`-Werror` + embedded toolchains reject stray
//! non-ASCII bytes — a prior wave hit this for real).

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::process::Command;

use assert_cmd::Command as Assert;
use fsm_analyzer::analyze_with_source;
use fsm_diagnostics::Severity;
use fsm_simulator::{InitOptions, Interpreter, Value};

/// The acceptance machine. `Run` declares three `on E` transitions
/// disambiguated by guards + explicit priority, plus an `on M` with a guard
/// and NO fallback (the no-match case). Action counters make every fired
/// transition observable from the host binary AND the simulator context.
const SEL_FSM: &str = r#"language fsm 2.0

machine Sel {
    context {
        x       : i16 = 0
        hi_ct   : u8  = 0
        lo_ct   : u8  = 0
        zero_ct : u8  = 0
        miss_ct : u8  = 0
    }
    events {
        E
        M
    }
    initial Run
    state Run {
        on E [ctx.x > 10] priority 1 -> Run : ctx.hi_ct = ctx.hi_ct + 1
        on E [ctx.x > 0]  priority 2 -> Run : ctx.lo_ct = ctx.lo_ct + 1
        on E              priority 3 -> Run : ctx.zero_ct = ctx.zero_ct + 1
        on M [ctx.x > 100]           -> Done : ctx.miss_ct = ctx.miss_ct + 1
    }
    state Done { }
}
"#;

/// ASCII-only host `main`. Drives the discriminating event sequences and
/// asserts observable context. Exit 0 == every assertion held; a non-zero
/// exit + a `FAIL ...` line on stderr pinpoints the exact failing case
/// (mirrors the import_header_e2e.rs / per_machine_strategy_e2e.rs
/// convention — proves the RIGHT transition fired, not just that it
/// compiled).
const DRIVER_C: &str = r#"#include <stdio.h>
#include "Sel.h"

void Sel_entry_RUN(Sel_t *m)  { (void)m; }
void Sel_entry_DONE(Sel_t *m) { (void)m; }
void Sel_exit_RUN(Sel_t *m)   { (void)m; }
void Sel_exit_DONE(Sel_t *m)  { (void)m; }

static int fails = 0;
static void chk(const char *what, int got, int want) {
    if (got != want) {
        fprintf(stderr, "FAIL %s: got %d want %d\n", what, got, want);
        fails++;
    }
}
static void send(Sel_t *m, Sel_EventId_t id) {
    Sel_Event_t e;
    e.id = id;
    Sel_dispatch(m, &e);
}

int main(void) {
    /* Case A (declared/priority-order tiebreak): x=50 makes BOTH
       [ctx.x > 10] and [ctx.x > 0] true simultaneously. The priority-1
       (declared-first) transition must win => hi only. */
    {
        Sel_t m; Sel_init(&m); m.context.x = 50;
        send(&m, SEL_EVENT_E);
        chk("A.hi", m.context.hi_ct, 1);
        chk("A.lo", m.context.lo_ct, 0);
        chk("A.zero", m.context.zero_ct, 0);
        chk("A.state", (int)Sel_current_state(&m), (int)SEL_STATE_RUN);
    }
    /* Case B (the silent-drop regression): x=5 => [ctx.x > 10] false,
       [ctx.x > 0] true. The 2nd guarded transition MUST fire (lo). The
       pre-fix table strategy dropped E here; the pre-fix switch strategy
       did not even compile. */
    {
        Sel_t m; Sel_init(&m); m.context.x = 5;
        send(&m, SEL_EVENT_E);
        chk("B.hi", m.context.hi_ct, 0);
        chk("B.lo", m.context.lo_ct, 1);
        chk("B.zero", m.context.zero_ct, 0);
    }
    /* Case C (unguarded fallback): x=0 => no guard true. The unguarded
       priority-3 transition is the catch-all and must fire (zero). */
    {
        Sel_t m; Sel_init(&m); m.context.x = 0;
        send(&m, SEL_EVENT_E);
        chk("C.hi", m.context.hi_ct, 0);
        chk("C.lo", m.context.lo_ct, 0);
        chk("C.zero", m.context.zero_ct, 1);
    }
    /* Case D (no match, NO fallback): M with x=7 => guard [ctx.x > 100]
       false and there is no other M transition => the event is NOT
       consumed, no transition fires, the machine stays in Run and miss_ct
       stays 0 (bubble-up then discard). */
    {
        Sel_t m; Sel_init(&m); m.context.x = 7;
        send(&m, SEL_EVENT_M);
        chk("D.miss", m.context.miss_ct, 0);
        chk("D.state", (int)Sel_current_state(&m), (int)SEL_STATE_RUN);
    }
    /* Case E (per-dispatch re-selection on one instance): the same machine,
       x mutated between dispatches, must re-evaluate the guard set every
       RTC step: 50 -> hi, 5 -> lo, 0 -> zero. */
    {
        Sel_t m; Sel_init(&m);
        m.context.x = 50; send(&m, SEL_EVENT_E);
        m.context.x = 5;  send(&m, SEL_EVENT_E);
        m.context.x = 0;  send(&m, SEL_EVENT_E);
        chk("E.hi", m.context.hi_ct, 1);
        chk("E.lo", m.context.lo_ct, 1);
        chk("E.zero", m.context.zero_ct, 1);
    }
    if (fails) {
        fprintf(stderr, "%d assertion(s) failed\n", fails);
        return 1;
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

/// `fsm generate --strategy <strategy>` then gcc -Werror compile + link +
/// RUN the driver. Panics with full gcc/runtime output on any failure so a
/// regression (duplicate-case, dropped event, warning) is unmissable.
fn generate_compile_run(strategy: &str) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    fs::write(dir.join("sel.fsm"), SEL_FSM).unwrap();

    // `fsm generate` itself MUST succeed — the analyzer accepts this valid
    // DSL (FSM-W0300 is a warning, not an error). The defect was only ever
    // at the emitted-C level.
    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--strategy", strategy])
        .arg(dir.join("sel.fsm"))
        .arg("--out")
        .arg(dir.join("out"))
        .assert()
        .success();

    let out = dir.join("out");
    fs::write(out.join("driver.c"), DRIVER_C).unwrap();
    fs::write(out.join("hal.c"), HAL_C).unwrap();

    let bin = out.join("sel_app");
    let gcc = Command::new("gcc")
        .current_dir(&out)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "Sel.c",
            "driver.c",
            "hal.c",
            "-o",
        ])
        .arg(&bin)
        .output()
        .expect("gcc spawn");
    assert!(
        gcc.status.success(),
        "[{strategy}] gcc -Werror FAILED (W7-FU-1 regression?). stderr:\n{}\n\
         --- generated Sel.c ---\n{}",
        String::from_utf8_lossy(&gcc.stderr),
        fs::read_to_string(out.join("Sel.c")).unwrap_or_default(),
    );

    let run = Command::new(&bin).output().expect("run sel_app");
    assert!(
        run.status.success(),
        "[{strategy}] generated binary asserted WRONG behaviour \
         (the right transition did not fire). stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("ALL OK"),
        "[{strategy}] binary did not print ALL OK. stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
}

/// The simulator is the behavioural oracle. Drive it with the SAME inputs
/// the C binary uses and assert the resulting context counters match the
/// generated-C behaviour exactly (sim==codegen). Cases A-D each use a fresh
/// `init` with `initial_context { x: <val> }` (the interpreter has no mid-
/// run context setter; Case E's mid-sequence mutate is covered C-side only).
fn sim_expectation(x: i16) -> (u8, u8, u8) {
    // Mirror Doc 08 §4.1/§4.2 by hand for the SPEC oracle cross-check.
    if x > 10 {
        (1, 0, 0) // priority-1 [x>10] wins (even though [x>0] also true)
    } else if x > 0 {
        (0, 1, 0) // priority-2 [x>0]
    } else {
        (0, 0, 1) // unguarded priority-3 fallback
    }
}

fn run_sim_event_e(x: i16) -> (u8, u8, u8) {
    let pr = fsm_parser::parse(SEL_FSM);
    let res = analyze_with_source(&pr, "sel.fsm", SEL_FSM);
    assert!(
        !res.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error),
        "analyzer rejected the acceptance fixture: {:?}",
        res.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.code.to_string())
            .collect::<Vec<_>>()
    );
    let ir = res.ir.expect("IR produced");
    let mut interp = Interpreter::new(&ir).expect("interpreter");
    let mut ctx0 = std::collections::BTreeMap::new();
    ctx0.insert("x".to_string(), Value::I16(x));
    interp
        .init(InitOptions {
            machine_name: "Sel".into(),
            initial_context: Some(ctx0),
            ..Default::default()
        })
        .expect("init");
    interp.dispatch("E").expect("dispatch E");
    let ctx = interp.context().expect("context");
    // The simulator may widen a `u8` context field to a wider integer
    // variant internally; accept any integer width and normalise to u8
    // (the values here are always 0/1).
    let get = |k: &str| -> u8 {
        let v = match ctx.get(k) {
            Some(Value::U8(v)) => *v as i128,
            Some(Value::U16(v)) => *v as i128,
            Some(Value::U32(v)) => *v as i128,
            Some(Value::U64(v)) => *v as i128,
            Some(Value::I8(v)) => *v as i128,
            Some(Value::I16(v)) => *v as i128,
            Some(Value::I32(v)) => *v as i128,
            Some(Value::I64(v)) => *v as i128,
            other => panic!("context field {k} not an integer: {other:?}"),
        };
        u8::try_from(v).unwrap_or_else(|_| panic!("context field {k} = {v} out of u8 range"))
    };
    (get("hi_ct"), get("lo_ct"), get("zero_ct"))
}

/// W7-FU-1 acceptance — SWITCH strategy. Was `gcc: duplicate case value`
/// (hard compile failure) on `main` before the fix.
#[test]
fn guard_disambiguated_same_event_switch_strategy_fires_correct_transition() {
    if common::should_skip_gcc("guard_disambiguated_dispatch_switch") {
        return;
    }
    generate_compile_run("switch");
}

/// W7-FU-1 acceptance — TABLE strategy. Silently DROPPED the event (P0-1
/// silent-data-loss class) on `main` before the fix when row-1's guard was
/// false.
#[test]
fn guard_disambiguated_same_event_table_strategy_fires_correct_transition() {
    if common::should_skip_gcc("guard_disambiguated_dispatch_table") {
        return;
    }
    generate_compile_run("table");
}

/// sim==codegen cross-check on the discriminating values. The expected
/// triples are derived BY HAND from Doc 08 §4.1/§4.2 (the spec, not
/// codegen), then asserted to equal BOTH the simulator's behaviour AND
/// (transitively, via the two e2e tests above which assert the identical
/// triples in C) the generated C. If the simulator disagreed with the spec
/// this fails independently of codegen.
#[test]
fn guard_disambiguated_selection_matches_spec_and_simulator() {
    // x = 50 : both guards true   -> priority-1 wins (declared-order tiebreak)
    // x = 5  : only [x>0] true    -> priority-2
    // x = 0  : no guard true      -> unguarded priority-3 fallback
    for x in [50i16, 5, 0, 11, 1, -3] {
        let spec = sim_expectation(x);
        let sim = run_sim_event_e(x);
        assert_eq!(
            sim, spec,
            "simulator disagrees with Doc 08 §4.1/§4.2 for x={x}: \
             sim={sim:?} spec={spec:?} (oracle would be WRONG — escalate)"
        );
    }
}
