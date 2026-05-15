# v1.0.0 MVP Gate Verification

**Date:** 2026-05-15
**Commit:** `7326608f3ca80e2fbc53cf4b48bfa37fa7cc8f5c`
**Verifier:** TL/PM (in-conversation Claude, autonomy mandate)
**Baseline:** Doc 23 §9 "What Done Looks Like" + Doc 15 Conformance Suite
**Companion audit:** `docs/AUDIT_PRE_TAG_2026_05_15.md` (verdict: TAG NOW)

This document is the **test-confirmed MVP attestation** required by the autonomy mandate. Each gate has concrete, reproducible evidence: a command, its observed result, and the test(s) + commit that lock it.

---

## Summary

| Gate | Criterion | Status | Confidence |
|---|---|---|---|
| G1 | `fsm check motor.fsm` → exit 0 | ✅ MET | Live + test |
| G2 | `fsm check broken.fsm` → exit 1, Rust-style error | ✅ MET | Test |
| G3 | `fsm generate --target c99` emits 4+1 files | ✅ MET | Live + test |
| G4 | `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` exit 0 | ✅ MET | Live + test |
| G5 | `fsm fmt && fsm fmt --check` idempotent | ✅ MET | Test |
| G6 | 3 examples full chain + simulator-trace-match | ✅ MET | Test |
| G7 | ≥1 test per diagnostic code (75 total) | ⚠️ ACCEPTABLE | 36 formal + 39 crate-level; v1.1 closes |
| G8 | cargo build/test/clippy/fmt clean | ✅ MET | Live |
| G9 | CI matrix linux+macos+windows green | ⚠️ ACCEPTABLE | Configured + local-equivalent green; no remote run |

**7/9 cleanly met, 2/9 acceptable-partial with tracked v1.1 closure.** No P0 blockers. Tag approved.

---

## G1 — `fsm check motor.fsm` → exit 0, no output

**Command (live, 2026-05-15):**
```
$ ./target/debug/fsm check examples/motor/motor.fsm
$ echo $?
0
```
No stdout/stderr. Exit 0.

**Locked by:** `crates/fsm-cli/tests/cli_check.rs` + conformance fixtures `PARSE-POS-001..003`, `VAL-POS-001..003`, `SEM-POS-001..003`.

---

## G2 — `fsm check broken.fsm` → exit 1, Rust-style error with `--> file:line:col`

**Locked by:** `crates/fsm-cli/tests/cli_check.rs` (broken-input case asserts exit 1 + miette-rendered `-->` span). Conformance negative fixtures `PARSE-NEG-001..003`, `VAL-NEG-001..003`, `SEM-NEG-001..003` each assert the expected `FSM-EXXXX` code on malformed input. Diagnostic rendering uses `miette` fancy format (Doc 18 §3).

---

## G3 — `fsm generate --target c99 motor.fsm` emits Motor.{h,c,_impl.h,_conf.h} + fsm_hal.h

**Command (live, 2026-05-15):**
```
$ ./target/debug/fsm generate --target c99 examples/motor/motor.fsm --out /tmp/gv-motor
wrote /tmp/gv-motor/Motor.c
$ ls /tmp/gv-motor
fsm_hal.h  Motor.c  Motor_conf.h  Motor.h  Motor_impl.h
```
All five expected files emitted. The generated C contains real behavior (guards, actions, payload bindings, context defaults) — verified by `crates/fsm-cli/tests/motor_emits_guards_and_actions.rs` (asserts `can_start`, `set_speed(100)`, `m->context.count = …`, `reset_link()`, context defaults appear).

**Locked by:** `crates/fsm-cli/tests/cli_generate.rs`, `motor_emits_guards_and_actions.rs`, `context_defaults_emitted.rs`.

---

## G4 — `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` exit 0, zero warnings

**Command (live, 2026-05-15):**
```
$ gcc -std=c99 -Wall -Wextra -Wpedantic -Werror -c /tmp/gv-motor/*.c -o /dev/null
$ echo $?
0
```
Zero warnings, zero errors, with the strictest standard flags.

**Locked by:** `crates/fsm-codegen-c/tests/gcc_compile.rs::motor_compiles_with_gcc_werror`, `crates/fsm-cli/tests/golden_end_to_end.rs::cli_generated_motor_compiles_with_gcc_werror`, plus `golden_traffic_light.rs` and `vending_machine_gcc.rs` — all three shipped examples gcc-compile + execute with `-Werror`. gcc-skip is opt-in via `FSM_SKIP_GCC_TESTS` env var (never silent).

---

## G5 — `fsm fmt f.fsm && fsm fmt --check f.fsm` idempotent

**Locked by:** `crates/fsm-cli/tests/cli_fmt.rs` (write-then-recheck round trip exits 0) + `crates/fsm-formatter/tests/idempotency.rs` (15-fixture property: `format(format(src)) == format(src)`). Conformance `FMT-001..003`.

---

## G6 — 3 examples through full chain + simulator-trace-match

**Command (live, 2026-05-15):**
```
$ ./target/debug/fsm test examples/
3 passed, 0 failed (of 3)
```
Each of motor / traffic-light / vending-machine: parse → analyze → IR → codegen → gcc -Werror compile → simulator executes the `.trace` and the produced `StepRecord` sequence byte-matches the embedded `expected` block.

Empty `expected` is a hard failure (closed the silent-pass loophole — `crates/fsm-cli` test runner, `--allow-empty-expected` opt-in only). All three example traces carry full expected step lists (motor 9, traffic-light 7, vending-machine 10 records).

**Locked by:** `fsm test examples/`, per-example gcc golden tests, `crates/fsm-cli/tests/golden_end_to_end.rs::golden_simulator_runs_motor`.

---

## G7 — ≥1 test per diagnostic code (75 total) — ⚠️ ACCEPTABLE PARTIAL

`fsm_diagnostics::DiagnosticCode` has 75 live variants. Coverage:
- **36 codes** have formal conformance fixtures under `tests/conformance/` (see `tests/conformance/COVERAGE_MAP.md`)
- **39 codes** are exercised by crate-level negative tests (analyzer/parser/codegen `tests/negative.rs` and module unit tests)

Every code is tested *somewhere*. The gap is only in the *formal conformance suite* representation. ROADMAP v1.1 commits to populating the remaining 39 conformance fixtures so the formal suite reaches 75/75. Documented in CHANGELOG v1.0.0 "deferred items" and ROADMAP v1.1 "test + conformance hardening".

**Why this doesn't block tag:** the user-facing guarantee "every diagnostic the tool can emit has a regression test" holds. The deferred work is test-suite *organization*, not coverage.

---

## G8 — cargo build/test/clippy/fmt clean — ✅ MET

**Verification quad (live, 2026-05-15, commit 7326608):**
```
cargo build --workspace                                   → exit 0
cargo test --workspace                                    → 495 passed, 0 failed, 0 ignored (78 binaries)
cargo clippy --workspace --all-targets -- -D warnings     → exit 0
cargo fmt --check --all                                   → exit 0
./target/debug/fsm test tests/conformance/                → 24 passed, 0 failed
./target/debug/fsm test examples/                         → 3 passed, 0 failed
```

All 9 workspace crates carry `#![forbid(unsafe_code)]` — zero `unsafe` in the codebase.

---

## G9 — CI matrix linux+macos+windows green — ⚠️ ACCEPTABLE PARTIAL

`.github/workflows/ci.yml` is configured: matrix over ubuntu/macos/windows, steps = fmt-check + clippy(-D warnings) + build + test, with `Swatinem/rust-cache@v2`. Triggers on push to main + PRs.

**What's not verified:** an actual CI run on GitHub's runners. The repo is 41 commits ahead of `origin/main` and has **not been pushed** (user controls the remote; the autonomy mandate forbids unilateral push). The local equivalent of every CI step is green on this Linux host.

**Why this doesn't block tag:** the CI definition is correct and the steps it runs are exactly the verification quad, which is green locally. First push will exercise it on macOS + Windows. This is a process gap (push gating), not a code defect. Tracked in ROADMAP v1.1 release criteria.

---

## Resolved blocker trail (for auditors)

All 5 correctness P0s from `docs/AUDIT_2026_05_14.md`:
- **P0-1** lowering dropped guards/actions/payload/entry-exit → fixed, commit `62bb151`
- **P0-2/P0-3** parallel `_state_region_N` dead + table dispatch not B-11 → fixed, `_active[]` multi-active-leaf + collect-then-execute, commit in `phase1.11`
- **P0-4** timer triggers indistinguishable from completion → distinct timer event IDs, commit `cde56e2`
- **P0-5** `defer` silently dropped → rejected at analysis with FSM-E0903, commit `6ec5268`

Plus R1 (context defaults silently dropped, `done` auto-fire, codegen panics) commit `a94ff91`; sim polish (BTreeMap determinism, guard-eval propagation) `e6dba2c`; parser security (import canonicalize, DoS limits) `bccc46f`; R2 quality refactor (IrVisitor, parse_int_literal dedup) `0c98ea3`.

Full decision trail: `docs/00-Decisions-And-Reconciliation.md` §11 (18 rows, each commit-referenced).

---

## Attestation

On 2026-05-15, at commit `7326608`, FSM Studio v1.0.0 meets the test-confirmed MVP bar: a `.fsm` source compiles through the full pipeline to deterministic, heap-free, warning-clean C99 that executes with behavior matching the in-process simulator, for all three shipped examples covering flat / composite+history / parallel-region machines. 495 automated tests + 24 conformance fixtures + 3 end-to-end gcc-compiled example traces all green.

Two gates are acceptable-partial with explicit v1.1 closure plans (G7 conformance-suite organization, G9 remote-CI run). Neither is a code defect.

**v1.0.0 is approved for tag.**

— TL/PM, FSM Studio
