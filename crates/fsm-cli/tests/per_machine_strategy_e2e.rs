//! End-to-end behavioural acceptance for v1.1-W7 per-machine dispatch
//! strategy override (Doc 00 §11.25, SUBAGENT_CONVENTIONS §5.4).
//!
//! Proves a SINGLE `fsm generate` over one source file with TWO machines
//! emits MIXED dispatch driven by `fsm.toml [machine.<Name>] strategy`:
//!   1. `fsm generate examples/per-machine-strategy/two_machines.fsm`
//!      (the sibling `fsm.toml` gives `Latch`=switch, `Counter`=table;
//!      both machines are tiny so the global `auto` heuristic would pick
//!      switch for BOTH — so a `Counter` table proves the *override*
//!      forced it, not coincidence).
//!   2. Structurally assert the emitted dispatch GENUINELY differs:
//!      `Latch.c` has the switch per-state helper and NO transition-table
//!      array; `Counter.c` has the `.rodata` `Counter_trans_table[]`
//!      array + `Counter_TransRow_t` and NO switch per-state helper. The
//!      two constructs are mutually exclusive (cheap secondary check).
//!   3. `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` compiles + links
//!      BOTH generated units with one ASCII-only host `main`.
//!   4. The binary is **executed**, driving each machine through its
//!      lifecycle and asserting real state + context transitions (NOT
//!      symbol presence) — this is what proves switch≡table behaviourally.
//!   5. A precedence test: CLI `--strategy switch` while fsm.toml sets
//!      `Counter` table ⇒ `Counter` stays table (fsm.toml wins); the
//!      same flag, against a machine with NO fsm.toml entry ⇒ follows the
//!      `--strategy` flag.
//!
//! FAIL-on-main: on `main` there is no per-machine override surface, so
//! both machines emit switch dispatch and step 2's
//! `Counter_trans_table[]` assertion fails (and the precedence test's
//! "Counter stays table despite --strategy switch" assertion fails).
//! PASS-after: the override forces Counter onto the table strategy.
//!
//! All C authored here is ASCII-only on purpose (-Werror + embedded
//! toolchains reject stray non-ASCII bytes — W6 hit this for real).

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::Command as Assert;

/// ASCII-only host `main` driving BOTH machines through their full
/// lifecycle and asserting observable state + context after each event.
/// Exit 0 == every assertion held; a distinct non-zero code pinpoints the
/// exact failing assertion (mirrors the import_header_e2e.rs convention).
const MAIN_C: &str = r#"#include <stdio.h>
#include "fsm_hal.h"
#include "Latch.h"
#include "Counter.h"

/* Entry/exit handlers the impl-header contract requires (no-ops here). */
void Latch_entry_OPEN(Latch_t *m)    { (void)m; }
void Latch_entry_ARMED(Latch_t *m)   { (void)m; }
void Latch_entry_LOCKED(Latch_t *m)  { (void)m; }
void Latch_exit_OPEN(Latch_t *m)     { (void)m; }
void Latch_exit_ARMED(Latch_t *m)    { (void)m; }
void Latch_exit_LOCKED(Latch_t *m)   { (void)m; }
void Counter_entry_COUNTING(Counter_t *m) { (void)m; }
void Counter_entry_TRIPPED(Counter_t *m)  { (void)m; }
void Counter_exit_COUNTING(Counter_t *m)  { (void)m; }
void Counter_exit_TRIPPED(Counter_t *m)   { (void)m; }

/* `latch_engaged()` is a DSL-declared extern shared by both impl headers
 * (the codegen emits the prototype into each). One definition with a
 * side-effect counter lets us prove the Latch action genuinely fired. */
static unsigned g_latch_engaged_calls = 0;
void latch_engaged(void) { g_latch_engaged_calls++; }

static int drive_latch(void) {
    /* Latch uses the SWITCH strategy (fsm.toml [machine.Latch]).
     * Open --ARM--> Armed (toggles+1)
     *      --LOCK[toggles>=1]--> Locked (locked=1, latch_engaged())
     *      --RELEASE--> Open (locked=0) */
    Latch_t lt;
    Latch_init(&lt);
    if (lt._active[0] != LATCH_STATE_OPEN) {
        fprintf(stderr, "Latch init: expected OPEN, got %d\n", lt._active[0]);
        return 11;
    }

    Latch_Event_t arm = { .id = LATCH_EVENT_ARM };
    Latch_dispatch(&lt, &arm);
    if (lt._active[0] != LATCH_STATE_ARMED) {
        fprintf(stderr, "Latch ARM: expected ARMED, got %d\n", lt._active[0]);
        return 12;
    }
    if (lt.context.toggles != 1) {
        fprintf(stderr, "Latch ARM: expected toggles=1, got %u\n",
                lt.context.toggles);
        return 13;
    }

    Latch_Event_t lock = { .id = LATCH_EVENT_LOCK };
    Latch_dispatch(&lt, &lock);
    if (lt._active[0] != LATCH_STATE_LOCKED) {
        fprintf(stderr, "Latch LOCK: expected LOCKED, got %d\n", lt._active[0]);
        return 14;
    }
    if (lt.context.locked != 1) {
        fprintf(stderr, "Latch LOCK: expected locked=1, got %u\n",
                lt.context.locked);
        return 15;
    }
    if (g_latch_engaged_calls != 1) {
        fprintf(stderr, "Latch LOCK: latch_engaged() not called (calls=%u)\n",
                g_latch_engaged_calls);
        return 16;
    }

    Latch_Event_t rel = { .id = LATCH_EVENT_RELEASE };
    Latch_dispatch(&lt, &rel);
    if (lt._active[0] != LATCH_STATE_OPEN) {
        fprintf(stderr, "Latch RELEASE: expected OPEN, got %d\n",
                lt._active[0]);
        return 17;
    }
    if (lt.context.locked != 0) {
        fprintf(stderr, "Latch RELEASE: expected locked=0, got %u\n",
                lt.context.locked);
        return 18;
    }
    return 0;
}

static int drive_counter(void) {
    /* Counter uses the TABLE strategy (fsm.toml [machine.Counter]).
     * Counting --INC--> Counting (value+1)   x2
     *          --TRIP--> Tripped  (value+100)
     * Tripped  --CLEAR--> Counting (value=0, cleared=1) */
    Counter_t cn;
    Counter_init(&cn);
    if (cn._active[0] != COUNTER_STATE_COUNTING) {
        fprintf(stderr, "Counter init: expected COUNTING, got %d\n",
                cn._active[0]);
        return 21;
    }
    if (cn.context.value != 0) {
        fprintf(stderr, "Counter init: expected value=0, got %u\n",
                cn.context.value);
        return 22;
    }

    Counter_Event_t inc = { .id = COUNTER_EVENT_INC };
    for (unsigned i = 1; i <= 2; i++) {
        Counter_dispatch(&cn, &inc);
        if (cn._active[0] != COUNTER_STATE_COUNTING) {
            fprintf(stderr, "Counter INC %u: expected COUNTING, got %d\n",
                    i, cn._active[0]);
            return 23;
        }
        if (cn.context.value != (uint16_t)i) {
            fprintf(stderr, "Counter INC %u: expected value=%u, got %u\n",
                    i, i, cn.context.value);
            return 24;
        }
    }

    /* TRIP: Counting -> Tripped, value += 100 (so 2 -> 102). */
    Counter_Event_t trip = { .id = COUNTER_EVENT_TRIP };
    Counter_dispatch(&cn, &trip);
    if (cn._active[0] != COUNTER_STATE_TRIPPED) {
        fprintf(stderr, "Counter TRIP: expected TRIPPED, got %d\n",
                cn._active[0]);
        return 25;
    }
    if (cn.context.value != 102) {
        fprintf(stderr, "Counter TRIP: expected value=102, got %u\n",
                cn.context.value);
        return 26;
    }

    Counter_Event_t clear = { .id = COUNTER_EVENT_CLEAR };
    Counter_dispatch(&cn, &clear);
    if (cn._active[0] != COUNTER_STATE_COUNTING) {
        fprintf(stderr, "Counter CLEAR: expected COUNTING, got %d\n",
                cn._active[0]);
        return 27;
    }
    if (cn.context.value != 0 || cn.context.cleared != 1) {
        fprintf(stderr, "Counter CLEAR: expected value=0 cleared=1, "
                "got value=%u cleared=%u\n",
                cn.context.value, cn.context.cleared);
        return 28;
    }
    return 0;
}

int main(void) {
    int rc = drive_latch();
    if (rc != 0) { return rc; }
    rc = drive_counter();
    if (rc != 0) { return rc; }
    printf("OK mixed-dispatch: Latch(switch) + Counter(table) "
           "both ran correctly\n");
    return 0;
}
"#;

/// Stand-alone host HAL (mirrors import_header_e2e.rs / vending_machine_gcc.rs):
/// supply the two HAL symbols directly so we don't depend on the POSIX
/// reference macros being plumbed into the right translation unit.
const HOST_HAL_C: &str = r#"#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
uint32_t fsm_hal_clock_now_ms(void) { return 0; }
void fsm_hal_assert(bool cond, const char *msg) {
    if (!cond) { fprintf(stderr, "[FSM ASSERT] %s\n", msg); abort(); }
}
"#;

/// Assert the two generated units use GENUINELY different dispatch
/// constructs. The switch strategy emits per-state
/// `<M>_try_transitions_in_state` helpers and NO transition-table array;
/// the table strategy emits a `<M>_TransRow_t` typedef + a `static const
/// <M>_trans_table[]` `.rodata` array and NO per-state switch helper.
/// Both strategies share `<M>_parent_table[]`, so we key on the
/// table-array / switch-helper pair, which is mutually exclusive.
fn assert_dispatch_differs(latch_c: &str, counter_c: &str) {
    // Latch == switch.
    assert!(
        latch_c.contains("Latch_try_transitions_in_state"),
        "Latch.c should use switch dispatch (per-state helper missing)\n{latch_c}"
    );
    assert!(
        !latch_c.contains("Latch_trans_table[]") && !latch_c.contains("Latch_TransRow_t"),
        "Latch.c should NOT carry a transition table (it must be switch)\n{latch_c}"
    );
    // Counter == table.
    assert!(
        counter_c.contains("Counter_trans_table[]") && counter_c.contains("Counter_TransRow_t"),
        "Counter.c should use table dispatch (.rodata trans table missing)\n{counter_c}"
    );
    assert!(
        !counter_c.contains("Counter_try_transitions_in_state"),
        "Counter.c should NOT carry the switch per-state helper (it must be table)\n{counter_c}"
    );
}

/// Compile + link both generated units with the ASCII host main and run
/// it. `label` is used in failure messages. Panics with gcc stderr on a
/// compile failure (so a -Werror regression is unmissable).
fn gcc_build_and_run(out_dir: &Path, label: &str) {
    fs::write(out_dir.join("main.c"), MAIN_C).unwrap();
    fs::write(out_dir.join("host_hal.c"), HOST_HAL_C).unwrap();

    let bin = out_dir.join("pms_app");
    let result = Command::new("gcc")
        .current_dir(out_dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "Latch.c",
            "Counter.c",
            "main.c",
            "host_hal.c",
            "-o",
        ])
        .arg(&bin)
        .output()
        .expect("gcc spawn");
    assert!(
        result.status.success(),
        "[{label}] gcc -Werror failed to compile/link the mixed-dispatch units\n\
         === gcc stderr ===\n{}\n=== gcc stdout ===\n{}",
        String::from_utf8_lossy(&result.stderr),
        String::from_utf8_lossy(&result.stdout),
    );

    let out = Command::new(&bin).output().expect("run pms_app");
    assert!(
        out.status.success(),
        "[{label}] mixed-dispatch run failed (code {:?}).\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("OK mixed-dispatch"),
        "[{label}] expected success banner, got: {stdout}"
    );
}

/// CORE behavioural acceptance: fsm.toml-driven mixed dispatch, compiled
/// and RUN with per-machine lifecycle assertions.
#[test]
fn fsm_toml_per_machine_strategy_mixes_switch_and_table_and_runs() {
    if common::should_skip_gcc("per_machine_strategy_e2e") {
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let out_dir = tmp.path();
    let root = common::workspace_root();
    let fsm = root.join("examples/per-machine-strategy/two_machines.fsm");
    let toml = root.join("examples/per-machine-strategy/fsm.toml");
    assert!(fsm.is_file(), "fixture missing: {}", fsm.display());
    assert!(toml.is_file(), "fixture missing: {}", toml.display());

    // No --strategy flag: the sibling fsm.toml [machine.*] tables drive
    // the per-machine override. fsm.toml is auto-discovered from the
    // input file's directory (Doc 18 §6.1 walk-upwards).
    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99"])
        .arg(&fsm)
        .arg("--out")
        .arg(out_dir)
        .assert()
        .success();

    let latch_c = fs::read_to_string(out_dir.join("Latch.c")).unwrap();
    let counter_c = fs::read_to_string(out_dir.join("Counter.c")).unwrap();
    assert_dispatch_differs(&latch_c, &counter_c);

    gcc_build_and_run(out_dir, "fsm.toml-driven");
}

/// PRECEDENCE: CLI `--strategy switch` while fsm.toml sets `Counter`
/// table. fsm.toml wins for `Counter` (stays table); `Latch` has its own
/// fsm.toml entry (switch) so also unaffected; the CLI flag only governs
/// machines WITHOUT a fsm.toml entry. We additionally prove the
/// "no-entry follows the flag" half with a second machine that has no
/// `[machine.*]` table (see `cli_flag_governs_machine_without_toml_entry`).
#[test]
fn fsm_toml_machine_override_beats_cli_strategy_flag() {
    if common::should_skip_gcc("per_machine_strategy_e2e") {
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let out_dir = tmp.path();
    let root = common::workspace_root();
    let fsm = root.join("examples/per-machine-strategy/two_machines.fsm");

    // CLI explicitly asks for `switch` globally. fsm.toml [machine.Counter]
    // = table MUST still win for Counter (highest precedence tier).
    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--strategy", "switch"])
        .arg(&fsm)
        .arg("--out")
        .arg(out_dir)
        .assert()
        .success();

    let latch_c = fs::read_to_string(out_dir.join("Latch.c")).unwrap();
    let counter_c = fs::read_to_string(out_dir.join("Counter.c")).unwrap();
    // Counter is STILL table despite `--strategy switch` (fsm.toml wins);
    // Latch is switch (its own fsm.toml entry, and the CLI flag agree).
    assert_dispatch_differs(&latch_c, &counter_c);
    // Compile+run to prove the override path is behaviourally correct too.
    gcc_build_and_run(out_dir, "precedence(--strategy switch)");
}

/// PRECEDENCE (second half): a machine with NO `[machine.*]` entry follows
/// the CLI `--strategy` flag. We generate `examples/motor/motor.fsm`
/// (single machine, no sibling fsm.toml carrying a [machine.Motor] table)
/// with `--strategy table` and assert it emits the table construct; then
/// with `--strategy switch` and assert the switch construct. This proves
/// the lower precedence tiers still function for un-overridden machines.
#[test]
fn cli_flag_governs_machine_without_toml_entry() {
    let root = common::workspace_root();
    let motor = root.join("examples/motor/motor.fsm");
    assert!(motor.is_file(), "example missing: {}", motor.display());

    for (flag, want_table) in [("table", true), ("switch", false)] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let out_dir = tmp.path();
        Assert::cargo_bin("fsm")
            .unwrap()
            .args(["generate", "--target", "c99", "--strategy", flag])
            .arg(&motor)
            .arg("--out")
            .arg(out_dir)
            .assert()
            .success();
        let motor_c = fs::read_to_string(out_dir.join("Motor.c")).unwrap();
        if want_table {
            assert!(
                motor_c.contains("Motor_trans_table[]")
                    && !motor_c.contains("Motor_try_transitions_in_state"),
                "--strategy table on an un-overridden machine must emit table dispatch"
            );
        } else {
            assert!(
                motor_c.contains("Motor_try_transitions_in_state")
                    && !motor_c.contains("Motor_trans_table[]"),
                "--strategy switch on an un-overridden machine must emit switch dispatch"
            );
        }
    }
}

/// CLEAN-REJECT: an invalid `[machine.X] strategy` value is a Doc 18 §3
/// exit-4 config error naming the offending machine, never a silent
/// fallback (zero-legacy doctrine).
#[test]
fn invalid_per_machine_strategy_value_is_exit4_not_silent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    fs::copy(
        common::workspace_root().join("examples/per-machine-strategy/two_machines.fsm"),
        dir.join("two_machines.fsm"),
    )
    .unwrap();
    fs::write(
        dir.join("fsm.toml"),
        "[generate]\ntarget = \"c99\"\n\n[machine.Latch]\nstrategy = \"bogus\"\n",
    )
    .unwrap();

    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99"])
        .arg(dir.join("two_machines.fsm"))
        .arg("--out")
        .arg(dir.join("out"))
        .assert()
        .failure()
        .code(4);
}

/// ABSENT-MACHINE: a `[machine.X]` naming a machine NOT generated this
/// invocation is a WARN (multi-file projects share one fsm.toml), NOT a
/// hard failure — generation still SUCCEEDS and the warning is emitted to
/// stderr (Doc 00 §11.25 decision).
#[test]
fn absent_machine_override_warns_but_still_succeeds() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    fs::copy(
        common::workspace_root().join("examples/per-machine-strategy/two_machines.fsm"),
        dir.join("two_machines.fsm"),
    )
    .unwrap();
    // `Phantom` is not in two_machines.fsm — should warn, not fail.
    fs::write(
        dir.join("fsm.toml"),
        "[generate]\ntarget = \"c99\"\n\n[machine.Phantom]\nstrategy = \"table\"\n",
    )
    .unwrap();

    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99"])
        .arg(dir.join("two_machines.fsm"))
        .arg("--out")
        .arg(dir.join("out"))
        .assert()
        .success()
        .stderr(predicates::str::contains("[machine.Phantom]"))
        .stderr(predicates::str::contains("no machine named `Phantom`"));
}

/// W7-FU-1 TRIPWIRE — documents a pre-existing core dispatch defect
/// surfaced (NOT introduced) by this wave's §5.4 behavioural acceptance.
///
/// The defect: a state with TWO `on EVENT` transitions distinguished
/// ONLY by guards (`on E [g1] -> A` / `on E [g2] -> B`) — a legal,
/// common UML statechart construct (the vending-machine
/// `on DISPENSE [balance >= price]` style) — is mis-lowered by BOTH
/// dispatch strategies, independently of W7:
///   * SWITCH (`dispatch_switch.rs`): emits two `case <EVENT>:` labels in
///     one C `switch` → `gcc` HARD ERROR `duplicate case value` (fails
///     even WITHOUT `-Werror`); the second guarded transition is
///     unreachable.
///   * TABLE (`dispatch_table.rs`): `<M>_select_for_region` returns the
///     first row matching `(source, trigger)` IGNORING the guard; the
///     guard is only re-checked at execute, so if the first row's guard
///     is false the event is dropped and the other eligible transition
///     never fires.
///
/// W7 only routes WHICH strategy a machine uses; fixing guard
/// disambiguation is in `dispatch_switch.rs`/`dispatch_table.rs` and is
/// explicitly OUT of W7 scope (the brief's DO-NOT list:
/// "the dispatch implementations themselves"). This test PINS the
/// current (defective) switch-strategy behaviour — a duplicate-`case`
/// gcc failure — so the defect cannot silently regress further AND so
/// this tripwire FAILS LOUDLY when W7-FU-1 is fixed, forcing whoever
/// fixes it to convert this into a positive behavioural test. Same
/// honest-surface pattern as SUB-FU-2 / OPAQUE-BUG-1 (Doc 00 §11.19/§11.23).
#[test]
fn w7_fu1_guard_disambiguated_same_event_is_currently_miscompiled() {
    if common::should_skip_gcc("per_machine_strategy_e2e") {
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    // Minimal repro: ONE state, TWO `on PING` guarded by complementary
    // conditions. ASCII-only.
    fs::write(
        dir.join("amb.fsm"),
        "language fsm 2.0\n\n\
         machine Amb {\n\
         \x20\x20\x20\x20context { n : u8 = 0 }\n\
         \x20\x20\x20\x20events { PING }\n\
         \x20\x20\x20\x20initial S0\n\
         \x20\x20\x20\x20state S0 {\n\
         \x20\x20\x20\x20\x20\x20\x20\x20on PING [ctx.n < 1] -> S0 : ctx.n = ctx.n + 1\n\
         \x20\x20\x20\x20\x20\x20\x20\x20on PING [ctx.n >= 1] -> S1\n\
         \x20\x20\x20\x20}\n\
         \x20\x20\x20\x20state S1 { }\n\
         }\n",
    )
    .unwrap();

    // Generate with the SWITCH strategy (the failure is deterministic and
    // self-evident there: a C compile error). `fsm generate` itself
    // SUCCEEDS — the analyzer accepts this valid DSL; the defect is at
    // the C level, the P0-1 silent-data-loss class.
    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--strategy", "switch"])
        .arg(dir.join("amb.fsm"))
        .arg("--out")
        .arg(dir.join("out"))
        .assert()
        .success();

    let amb_c = fs::read_to_string(dir.join("out/Amb.c")).unwrap();
    fs::write(
        dir.join("out/m.c"),
        "#include \"Amb.h\"\n\
         void Amb_entry_S0(Amb_t*m){(void)m;}\n\
         void Amb_entry_S1(Amb_t*m){(void)m;}\n\
         void Amb_exit_S0(Amb_t*m){(void)m;}\n\
         void Amb_exit_S1(Amb_t*m){(void)m;}\n\
         int main(void){Amb_t a;Amb_init(&a);return 0;}\n",
    )
    .unwrap();
    fs::write(dir.join("out/h.c"), HOST_HAL_C).unwrap();

    let res = Command::new("gcc")
        .current_dir(dir.join("out"))
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "Amb.c",
            "m.c",
            "h.c",
            "-o",
            "amb_app",
        ])
        .output()
        .expect("gcc spawn");

    // CURRENT (defective) expectation: the switch strategy emits a
    // duplicate `case` so gcc FAILS. When W7-FU-1 is fixed this assertion
    // flips and the test must be rewritten as a positive behavioural
    // check (drive PING twice -> S0 then S1).
    assert!(
        !res.status.success(),
        "W7-FU-1 APPEARS FIXED: the switch strategy now compiles \
         guard-disambiguated same-event transitions. Convert this \
         tripwire into a positive behavioural test (PING: S0 -> S0 -> S1) \
         and update Doc 00 §11.25 / the W7-FU-1 follow-up entry."
    );
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("duplicate case"),
        "expected the documented `duplicate case value` defect, got:\n{stderr}\
         \n--- generated Amb.c ---\n{amb_c}"
    );
}

/// Sanity that the fixture assets exist where docs/tests reference them.
#[test]
fn fixture_assets_present() {
    let root = common::workspace_root();
    for rel in &[
        "examples/per-machine-strategy/two_machines.fsm",
        "examples/per-machine-strategy/fsm.toml",
    ] {
        assert!(
            Path::new(&root).join(rel).is_file(),
            "missing fixture asset {rel}"
        );
    }
}
